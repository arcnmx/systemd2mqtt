use {
	self::{cli::Args, core::Core},
	anyhow::{format_err, Result},
	clap::Parser,
	futures::{pin_mut, select, FutureExt, StreamExt},
	log::{debug, error, info},
};

mod cli;
mod core;
mod payload;

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

#[tokio::main]
async fn main() -> Result<()> {
	log_init();

	let cli = Args::parse();

	let mut core = Core::new(&cli).await?;

	let mut messages = core.mqtt.get_stream(25);

	let manager = core.sys_manager().await?;

	let ctrlc = StreamExt::fuse(async_ctrlc::CtrlC::new().expect("ctrl+c"));
	pin_mut!(ctrlc);

	let units = core.unit_proxies(&manager).fuse();
	pin_mut!(units);

	let units = select! {
		units = units => units,
		_ = ctrlc.next() => return Ok(()),
	};

	let systemd_changes = futures::future::join_all(units.iter().map(|(_, (unit, proxy))| {
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

	let systemd_changes = futures::stream::select_all(systemd_changes);
	pin_mut!(systemd_changes);

	core.connect(&manager).await?;

	let initial_setup = async {
		core.announce().await?;
		let mut futures = Vec::new();
		for (unit, proxy) in units.values() {
			futures.push(core.inform_unit(unit, proxy));
		}
		futures::future::try_join_all(futures).await?;
		Ok::<(), anyhow::Error>(())
	}
	.fuse();
	pin_mut!(initial_setup);

	let mut new_jobs = manager.receive_job_new().await?;
	let mut done_jobs = manager.receive_job_removed().await?;

	loop {
		select! {
			res = initial_setup => if let Err(e) = res {
				error!("Failed to perform initial setup: {:?}", e);
			},
			_ = ctrlc.next() => {
				break
			},
			job_new = new_jobs.next() => {
				let job_new = job_new
					.ok_or_else(|| format_err!("lost systemd connection"))?;
				let job_new = job_new.args()?;
				let unit_name = job_new.unit();
				match units.get(&unit_name[..]) {
					Some((unit, proxy)) => core.inform_unit(unit, proxy).await?,
					None => info!("uninterested in new {} job", unit_name),
				}
			},
			job_removed = done_jobs.next() => {
				let job_removed = job_removed
					.ok_or_else(|| format_err!("lost systemd connection"))?;
				let job_removed = job_removed.args()?;
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
					Some(Some(m)) => m,
					_ => return Err(format_err!("lost mqtt connection")),
				};
				debug!("received MQTT msg: {:#?}", message.topic());
				if !core.handle_message(&manager, &message).await? {
					break
				}
			},
		}
	}

	core.disconnect().await
}
