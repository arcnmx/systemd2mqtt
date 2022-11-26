use {
	crate::{
		cli::{Args, Unit},
		payload::{ServiceCommand, ServiceStatus, UnitCommand, UnitStatus},
	},
	anyhow::Result,
	futures::TryFutureExt,
	log::warn,
	paho_mqtt::{self as mqtt, Message, QOS_0 as QOS},
	std::{borrow::Cow, collections::HashMap, time::Duration},
	zbus_systemd::{
		systemd1::{ManagerProxy, UnitProxy},
		zbus,
	},
};

pub struct Core<'c> {
	pub cli: &'c Args,
	pub units: HashMap<&'c str, Unit<'c>>,
	pub mqtt: mqtt::AsyncClient,
	pub sys: zbus::Connection,
}

impl<'c> Core<'c> {
	pub async fn new(cli: &'c Args) -> Result<Core<'c>> {
		Ok(Core {
			sys: zbus::Connection::system().await?,
			mqtt: mqtt::AsyncClient::new(cli.mqtt_create().finalize())?,
			units: cli.units(),
			cli,
		})
	}

	pub async fn sys_manager(&self) -> Result<ManagerProxy> {
		ManagerProxy::new(&self.sys).await.map_err(Into::into)
	}

	pub fn mqtt_will(&self) -> Message {
		let payload = ServiceStatus {
			is_active: false,
			units: Default::default(),
		};
		Message::new_retained(self.cli.mqtt_pub_topic(), payload.encode(), mqtt::QOS_1)
	}

	pub async fn announce(&self) -> Result<()> {
		if self.cli.use_mqtt() {
			let mut futures = Vec::new();

			let payload = ServiceStatus {
				is_active: true,
				units: self.units.keys().map(|&s| Cow::Borrowed(s)).collect(),
			};
			self
				.mqtt
				.publish(Message::new_retained(
					self.cli.mqtt_pub_topic(),
					payload.encode(),
					mqtt::QOS_1,
				))
				.await?;

			for unit in self.units.values() {
				futures.push(self.mqtt.publish(unit.hass_announce(true)?));
			}
			let global = self.cli.hass_global_state();
			futures.push(self.mqtt.publish(self.cli.hass_announce(&global, true)?));

			futures::future::try_join_all(futures).await?;
		}

		Ok(())
	}

	pub async fn connect(&self, manager: &ManagerProxy<'_>) -> Result<()> {
		manager.subscribe().await?;

		if self.cli.use_mqtt() {
			let mut opts = self.cli.mqtt_connect();
			opts.will_message(self.mqtt_will());
			self.mqtt.connect(opts.finalize()).await?;
			self
				.mqtt
				.subscribe(format!("{}/+/activate", self.cli.topic_root()), QOS)
				.await?;
			self.mqtt.subscribe(self.cli.mqtt_sub_topic(), QOS).await?;
		}

		Ok(())
	}

	pub async fn disconnect(&self) -> Result<()> {
		if self.cli.use_mqtt() {
			let global = self.cli.hass_global_state();
			let mut futures = Vec::new();
			if self.cli.clean_up {
				for unit in self.units.values() {
					futures.push(
						self
							.mqtt
							.publish(Message::new_retained(unit.hass_config_topic(), "", QOS)),
					);
				}
			} else {
				// unset retain flag on entity configs
				for unit in self.units.values() {
					futures.push(self.mqtt.publish(unit.hass_announce(false)?));
				}
				futures.push(self.mqtt.publish(self.cli.hass_announce(&global, false)?));
			}
			futures.push(self.mqtt.publish(self.mqtt_will()));

			if let Err(e) = futures::future::try_join_all(futures).await {
				warn!("Failed to clean up after ourselves: {:?}", e);
			}

			let opts = mqtt::DisconnectOptionsBuilder::new()
				.timeout(Duration::from_secs(5))
				.reason_code(mqtt::ReasonCode::ServerShuttingDown)
				.publish_will_message()
				.finalize();
			self.mqtt.disconnect(Some(opts)).await?;
		}

		Ok(())
	}

	pub async fn unit_proxy(&self, manager: &ManagerProxy<'_>, unit: &Unit<'c>) -> Result<UnitProxy> {
		Ok(
			UnitProxy::builder(&self.sys)
				.path(manager.load_unit(unit.unit_name().into()).await?)?
				.build()
				.await?,
		)
	}

	pub async fn unit_proxies<'m, 's: 'm>(
		&'s self,
		manager: &ManagerProxy<'m>,
	) -> HashMap<&'c str, (&'s Unit<'c>, UnitProxy<'m>)> {
		let proxies = futures::future::join_all(
			self
				.units
				.iter()
				.map(|(&name, unit)| self.unit_proxy(manager, unit).map_ok(move |proxy| (name, proxy))),
		);

		proxies
			.await
			.into_iter()
			.filter_map(|r| match r {
				Err(e) => {
					log::error!("Failed to set up unit: {:?}", e);
					None
				},
				Ok((n, p)) => self.units.get(n).map(|u| (n, (u, p))),
			})
			.collect()
	}

	pub async fn inform_unit(&self, unit: &Unit<'c>, unit_proxy: &UnitProxy<'_>) -> Result<()> {
		let payload = UnitStatus {
			load_state: unit_proxy.load_state().await?,
			active_state: unit_proxy.active_state().await?,
			id: unit_proxy.id().await?,
			invocation_id: unit_proxy.invocation_id().await?,
			description: unit_proxy.description().await?,
			transient: unit_proxy.transient().await?,
		};

		if self.cli.use_mqtt() {
			self
				.mqtt
				.publish(Message::new_retained(unit.mqtt_pub_topic(), payload.encode(), QOS))
				.await?;
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
				warn!("unsupported unit command: {:?}", e)
			},
		}
		Ok(())
	}

	pub async fn handle_message(&self, manager: &ManagerProxy<'_>, message: &Message) -> Result<bool> {
		let segments = message.topic().split('/').collect::<Vec<_>>();
		match &segments[..] {
			["systemd", hostname, ..] if *hostname != self.cli.hostname() => (), // not for us, ignore
			[_, _, unit, "activate"] => match self.units.contains_key(unit) {
				true => self.handle_activate(manager, unit, &message.payload()).await?,
				false => {
					warn!("attempt to control untracked unit {}", unit);
				},
			},
			[_, _, "control"] => match serde_json::from_slice::<ServiceCommand>(message.payload()) {
				Ok(ServiceCommand::Set { active }) => match active {
					true => (), // ignore, already on
					false => return Ok(false),
				},
				Err(e) => warn!("unsupported systemd2mqtt command: {:?}", e),
			},
			_ => {
				warn!("unrecognized topic {}", message.topic());
			},
		}
		Ok(true)
	}
}
