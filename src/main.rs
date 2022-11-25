use {
	self::{cli::Args, core::Core},
	anyhow::{format_err, Result},
	clap::Parser,
	futures::{pin_mut, select, FutureExt, StreamExt},
};

mod cli;
mod core;
mod payload;

#[tokio::main]
async fn main() -> Result<()> {
	env_logger::init();

	let cli = Args::parse();

	let mut core = Core::new(&cli).await?;

	let mut messages = core.mqtt.get_stream(25);

	let manager = core.sys_manager().await?;
	let mut new_jobs = manager.receive_job_new().await?;
	let mut done_jobs = manager.receive_job_removed().await?;

	core.connect(&manager).await?;

	let initial_setup = async {
		core.announce().await?;
		let mut futures = Vec::new();
		for unit in &core.interesting_units {
			futures.push(core.inform_unit(&manager, unit));
		}
		futures::future::try_join_all(futures).await?;
		Ok::<(), anyhow::Error>(())
	}
	.fuse();
	pin_mut!(initial_setup);

	let ctrlc = StreamExt::fuse(async_ctrlc::CtrlC::new().expect("ctrl+c"));
	pin_mut!(ctrlc);

	loop {
		select! {
			res = initial_setup => if let Err(e) = res {
				log::error!("Failed to perform initial setup: {:?}", e);
			},
			_ = ctrlc.next() => {
				break
			},
			job_new = new_jobs.next() => {
				let job_new = job_new
					.ok_or_else(|| format_err!("lost systemd connection"))?;
				let job_new = job_new.args()?;
				core.inform_job(&manager, job_new.id(), job_new.unit()).await?;
			},
			job_removed = done_jobs.next() => {
				let job_removed = job_removed
					.ok_or_else(|| format_err!("lost systemd connection"))?;
				let job_removed = job_removed.args()?;
				core.inform_job(&manager, job_removed.id(), job_removed.unit()).await?;
			},
			message = messages.next() => {
				let message = match message {
					Some(Some(m)) => m,
					_ => return Err(format_err!("lost mqtt connection")),
				};
				log::debug!("received MQTT msg: {:#?}", message.topic());
				if !core.handle_message(&manager, &message).await? {
					break
				}
			},
		}
	}

	core.disconnect().await
}
