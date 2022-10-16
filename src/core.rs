use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Duration;
use anyhow::Result;
use zbus_systemd::{
	self as systemd,
	zbus,
	systemd1::{ManagerProxy, UnitProxy},
};
use paho_mqtt::{
	self as mqtt,
	Message,
	QOS_0 as QOS,
};
use crate::{
	cli::Args,
	payload::{ServiceStatus, UnitStatus, UnitCommand},
};

pub struct Core<'c> {
	pub cli: &'c Args,
	pub interesting_units: HashSet<&'c str>,
	pub mqtt: mqtt::AsyncClient,
	pub sys: zbus::Connection,
}

impl<'c> Core<'c> {
	pub async fn new(cli: &'c Args) -> Result<Core<'c>> {
		Ok(Core {
			sys: systemd::connect_system_dbus().await?,
			mqtt: mqtt::AsyncClient::new(cli.mqtt_create().finalize())?,
			interesting_units: cli.interesting_units(),
			cli,
		})
	}

	pub async fn sys_manager(&self) -> Result<ManagerProxy> {
		ManagerProxy::new(&self.sys).await
			.map_err(Into::into)
	}

	pub fn mqtt_will(&self) -> Message {
		let payload = ServiceStatus {
			is_active: false,
			units: Default::default(),
		};
		Message::new_retained(self.cli.mqtt_pub_topic(), payload.encode(), QOS)
	}

	pub async fn announce(&self) -> Result<()> {
		if self.cli.use_mqtt() {
			let mut futures = Vec::new();
			for unit in self.cli.interesting_units() {
				let switch = self.cli.hass_unit_switch(unit);
				futures.push(self.mqtt.publish(self.cli.hass_announce_switch(&switch)));
			}
			futures.push(self.mqtt.publish(self.cli.hass_announce_switch(&self.cli.hass_global_switch())));
			futures::future::try_join_all(futures).await?;

			let payload = ServiceStatus {
				is_active: true,
				units: self.cli.interesting_units().iter()
					.map(|&k| Cow::Borrowed(k))
					.collect(),
			};
			self.mqtt.publish(Message::new_retained(self.cli.mqtt_pub_topic(), payload.encode(), QOS)).await?;
		}

		Ok(())
	}

	pub async fn connect(&self, manager: &ManagerProxy<'_>) -> Result<()> {
		manager.subscribe().await?;

		if self.cli.use_mqtt() {
			let mut opts = self.cli.mqtt_connect();
			opts.will_message(self.mqtt_will());
			self.mqtt.connect(opts.finalize()).await?;
			self.mqtt.subscribe(format!("{}/+/activate", self.cli.topic_root()), QOS).await?;
		}

		Ok(())
	}

	pub async fn disconnect(&self) -> Result<()> {
		let opts = mqtt::DisconnectOptionsBuilder::new()
			.timeout(Duration::from_secs(5))
			.reason_code(mqtt::ReasonCode::ServerShuttingDown)
			.publish_will_message()
			.finalize();
		self.mqtt.disconnect(Some(opts)).await?;

		Ok(())
	}

	pub async fn inform_job(&self, manager: &ManagerProxy<'_>, _job_id: &u32, unit_name: &str) -> Result<()> {
		if self.interesting_units.contains(unit_name) {
			self.inform_unit(manager, unit_name).await?;
		}
		Ok(())

	}

	pub async fn inform_unit(&self, manager: &ManagerProxy<'_>, unit_name: &str) -> Result<()> {
		let unit = UnitProxy::builder(&self.sys)
			.path(manager.load_unit(unit_name.into()).await?)?
			.build().await?;

		let payload = UnitStatus {
			load_state: unit.load_state().await?,
			active_state: unit.active_state().await?,
			id: unit.id().await?,
			invocation_id: unit.invocation_id().await?,
			description: unit.description().await?,
			transient: unit.transient().await?,
		};

		if self.cli.use_mqtt() {
			self.mqtt.publish(Message::new_retained(
				self.cli.mqtt_pub_topic_unit(unit_name),
				payload.encode(), QOS,
			)).await?;
		}

		Ok(())
	}

	pub async fn handle_activate(&self, manager: &ManagerProxy<'_>, unit: &str, payload: &[u8]) -> Result<()> {
		let mode = "fail".into();
		match serde_json::from_slice::<UnitCommand>(payload) {
			Ok(UnitCommand::Start) => {
				manager.start_unit(unit.into(), mode).await?;
			},
			Ok(UnitCommand::Stop) => {
				manager.stop_unit(unit.into(), mode).await?;
			},
			Ok(UnitCommand::Restart) => {
				manager.restart_unit(unit.into(), mode).await?;
			},
			Err(e) => {
				log::warn!("unsupported unit command: {:?}", e)
			}
		}
		Ok(())
	}

	pub async fn handle_message(&self, manager: &ManagerProxy<'_>, message: &Message) -> Result<bool> {
		let segments = message.topic().split('/').collect::<Vec<_>>();
		match &segments[..] {
			[ "systemd", hostname, .. ] if *hostname != self.cli.hostname() =>
				(), // not for us, ignore
			[ _, _, unit, "activate" ] => match self.interesting_units.contains(unit) {
				true => self.handle_activate(manager, unit, &message.payload()).await?,
				false => {
					log::warn!("attempt to control untracked unit {}", unit);
				},
			},
			_ => {
				log::warn!("unrecognized topic {}", message.topic());
			},
		}
		Ok(true)
	}
}
