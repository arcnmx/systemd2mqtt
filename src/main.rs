use {
	self::cli::Args,
	clap::Parser,
	error_stack::{bail, IntoReport as _, ResultExt as _},
	futures::{future, pin_mut, select, stream, FutureExt as _, StreamExt},
	log::{debug, error, info, trace},
	sd_notify::NotifyState,
	systemd2mqtt_core::Core,
	systemd2mqtt_payload::{Error, Result},
};

mod cli;

fn log_init() {
	use {
		env_logger::{Builder, Env},
		log::LevelFilter,
	};

	Builder::new()
		.filter_level(LevelFilter::Warn)
		.parse_env(Env::default())
		.init()
}

fn notify(state: NotifyState) {
	trace!("sd_notify({state:?})");
	if let Err(e) = sd_notify::notify(false, &[state]) {
		debug!("sd_notify failed: {e:?}");
	}
}

fn info_notify(msg: &str) {
	info!("{msg}");
	notify(NotifyState::Status(msg));
}

#[tokio::main]
async fn main() -> crate::Result<()> {
	log_init();

	let cli = Args::parse();

	info_notify("Connecting to system bus…");
	let core = Core::new(&cli).await?;

	info_notify("Communicating with org.freedesktop.systemd1…");
	let manager = core.sys_manager().await?;

	let ctrlc = StreamExt::fuse(async_ctrlc::CtrlC::new().expect("ctrl+c"));
	pin_mut!(ctrlc);

	let units = core.unit_proxies(&manager).fuse();
	pin_mut!(units);

	let units = select! {
		units = units => units,
		_ = ctrlc.next() => return Ok(()),
	};

	let systemd_changes = future::join_all(units.iter().map(|(_, (unit, proxy))| {
		proxy
			.receive_active_state_changed()
			.map(move |c| c.map(move |c| (unit, proxy, c)))
	}))
	.fuse();
	pin_mut!(systemd_changes);

	let systemd_changes = select! {
		res = systemd_changes => res,
		_ = ctrlc.next() => return Ok(()),
	};

	let systemd_changes = stream::select_all(systemd_changes);
	pin_mut!(systemd_changes);

	info_notify("Connecting to MQTT broker…");
	core.connect(&manager).await?;
	let messages = core.connect(&manager).await?;
	let mut messages = messages.unwrap_or(stream::empty().boxed()).fuse();

	info_notify("Broadcasting unit entities and state…");
	let initial_setup = async {
		core.announce().await.change_context(Error::ConnectionError)?;
		let mut futures = Vec::new();
		for (unit, proxy) in units.values() {
			futures.push(core.inform_unit(unit, proxy));
		}
		future::try_join_all(futures).await?;
		Ok::<(), error_stack::Report<Error>>(())
	}
	.fuse();
	pin_mut!(initial_setup);

	let mut new_jobs = manager
		.receive_job_new()
		.await
		.into_report()
		.change_context(Error::Systemd)?;
	let mut done_jobs = manager
		.receive_job_removed()
		.await
		.into_report()
		.change_context(Error::Systemd)?;

	loop {
		select! {
			res = initial_setup => match res {
				Ok(()) => {
					info_notify("Started");
					notify(NotifyState::Ready);
				},
				Err(e) =>
					error!("Failed to perform initial setup: {:?}", e),
			},
			_ = ctrlc.next() => {
				break
			},
			job_new = new_jobs.next() => {
				let job_new = job_new
					.ok_or_else(|| Error::ConnectionLostSystemd)?;
				let job_new = job_new.args().into_report().change_context(Error::Dbus)?;
				let unit_name = job_new.unit();
				match units.get(&unit_name[..]) {
					Some((unit, proxy)) => core.inform_unit(unit, proxy).await?,
					None => info!("uninterested in new {} job", unit_name),
				}
			},
			job_removed = done_jobs.next() => {
				let job_removed = job_removed
					.ok_or_else(|| Error::ConnectionLostSystemd)?;
				let job_removed = job_removed.args().into_report().change_context(Error::Dbus)?;
				let unit_name = job_removed.unit();
				match units.get(&unit_name[..]) {
					Some((unit, proxy)) => core.inform_unit(unit, proxy).await?,
					None => info!("uninterested in removed {} job", unit_name),
				}
			},
			res = systemd_changes.next() => if let Some((unit, proxy, _active_changed)) = res {
				core.inform_unit(unit, proxy).await?;
			},
			message = messages.next() => {
				let message = match message {
					Some(m) => m,
					_ => bail!(Error::ConnectionLostMqtt),
				};
				debug!("received MQTT msg: {:#?}", message.topic());
				if !core.handle_message(&manager, &message).await? {
					info!("shutdown requested via MQTT");
					break
				}
			},
		}
	}

	info_notify("Cleaning up and disconnecting…");
	notify(NotifyState::Stopping);
	drop(messages);
	core.disconnect().await
}
