pub(crate) use self::mqtt::{EntityTopics, MqttConnection, MqttTopic, QOS};
use {
	crate::{
		cli::{Args, UnitConfig},
		payload::{UnitCommand, UnitStatus},
		Error, Result,
	},
	error_stack::IntoReport as _,
	futures::{lock::Mutex, stream::BoxStream, TryFutureExt as _},
	hass_mqtt_client::Message,
	log::{debug, error, warn},
	once_cell::sync::OnceCell,
	std::{
		collections::HashMap,
		ops::Deref,
		sync::{atomic::AtomicPtr, Weak},
	},
	zbus_systemd::{
		hostname1::HostnamedProxy,
		systemd1::{ManagerProxy, UnitProxy},
		zbus,
	},
};

mod mqtt;

static HOSTNAME: OnceCell<Option<String>> = OnceCell::new();

pub(crate) struct Core<'c> {
	cli: &'c Args,
	units: HashMap<&'c str, Unit<'c>>,
	mqtt: Mutex<Option<MqttConnection<'c>>>,
	sys: zbus::Connection,
}

impl<'c> Core<'c> {
	pub async fn new(cli: &'c Args) -> Result<Core<'c>> {
		Ok(Core {
			sys: zbus::Connection::system().await.map_err(Error::from)?,
			mqtt: Default::default(),
			units: cli.populate_core_units(),
			cli,
		})
	}

	pub async fn sys_manager(&self) -> Result<ManagerProxy> {
		let manager = ManagerProxy::new(&self.sys).await.map_err(Error::from).into_report()?;

		if self.cli.hostname.is_none() {
			let hostname = HostnamedProxy::new(&self.sys)
				.and_then(|hostnamed| async move { hostnamed.hostname().await })
				.await;
			let _ = HOSTNAME.set(match hostname {
				Ok(hostname) => Some(hostname),
				Err(e) => {
					debug!("failed to query hostnamed property: {e:?}");
					warn!("failed to determine system hostname, please specify manually with --hostname argument");
					None
				},
			});
		}

		Ok(manager)
	}

	pub fn hostname_(&self) -> &'c str {
		if let Some(hostname) = self.cli.hostname.as_ref() {
			return hostname
		}
		HOSTNAME.wait().as_ref().map(|s| &s[..]).unwrap_or("systemd")
	}

	pub fn hostname(&self) -> &'c str {
		let hostname = self.hostname_();
		self.hostname_().split('.').next().unwrap_or(hostname)
	}

	pub async fn connect(&self, manager: &ManagerProxy<'_>) -> Result<Option<BoxStream<Message>>> {
		manager.subscribe().await.map_err(Error::from)?;

		self.connect_mqtt().await
	}

	pub async fn unit_proxy(&self, manager: &ManagerProxy<'_>, unit: &Unit<'c>) -> Result<UnitProxy> {
		Ok(
			UnitProxy::builder(&self.sys)
				.path(manager.load_unit(unit.unit_name().into()).await.map_err(Error::from)?)
				.map_err(Error::from)?
				.build()
				.await
				.map_err(Error::from)?,
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
					error!("Failed to set up unit: {:?}", e);
					None
				},
				Ok((n, p)) => self.units.get(n).map(|u| (n, (u, p))),
			})
			.collect()
	}

	pub async fn inform_unit(&self, unit: &Unit<'c>, unit_proxy: &UnitProxy<'_>) -> Result<()> {
		let payload = UnitStatus {
			load_state: unit_proxy.load_state().await.map_err(Error::from)?,
			active_state: unit_proxy.active_state().await.map_err(Error::from)?,
			id: unit_proxy.id().await.map_err(Error::from)?,
			invocation_id: unit_proxy.invocation_id().await.map_err(Error::from)?,
			description: unit_proxy.description().await.map_err(Error::from)?,
			transient: unit_proxy.transient().await.map_err(Error::from)?,
		};

		let res = if let Some(_) = &*self.mqtt.lock().await {
			let topics = unit
				.topics()
				.ok_or_else(|| Error::InternalConsistency {
					unit_name: unit.unit_name().into(),
				})
				.into_report()?;
			topics.publish_state(&payload, true, QOS)
		} else {
			return Ok(())
		};

		res.await
	}

	pub async fn handle_activate(&self, manager: &ManagerProxy<'_>, unit: &str, payload: &[u8]) -> Result<()> {
		let mode = "fail".into();
		match serde_json::from_slice::<UnitCommand>(payload) {
			Ok(UnitCommand::Start) => {
				manager.start_unit(unit.into(), mode).await.map_err(Error::from)?;
			},
			Ok(UnitCommand::Stop) => {
				manager.stop_unit(unit.into(), mode).await.map_err(Error::from)?;
			},
			Ok(UnitCommand::Restart) => {
				manager.restart_unit(unit.into(), mode).await.map_err(Error::from)?;
			},
			Err(e) => {
				warn!("unsupported unit command: {:?}", e)
			},
		}
		Ok(())
	}
}

#[derive(Debug)]
pub(crate) struct Unit<'a> {
	pub unit: &'a UnitConfig,
	pub mqtt: AtomicPtr<EntityTopics>,
}

impl<'a> Unit<'a> {
	pub fn new(unit: &'a UnitConfig) -> Self {
		Self {
			unit,
			mqtt: AtomicPtr::new(Weak::<EntityTopics>::new().into_raw() as *mut _),
		}
	}

	pub fn unit_name(&self) -> &'a String {
		&self.unit.unit
	}
}

impl<'a> Drop for Unit<'a> {
	fn drop(&mut self) {
		self.drop_topics();
	}
}

impl<'a> Deref for Unit<'a> {
	type Target = UnitConfig;

	fn deref(&self) -> &Self::Target {
		self.unit
	}
}

impl Args {
	fn populate_core_units(&self) -> HashMap<&str, Unit> {
		self.units.iter().map(|u| (&u.unit[..], Unit::new(u))).collect()
	}
}
