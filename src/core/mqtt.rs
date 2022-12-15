use {
	crate::{
		core::{Core, Unit},
		entities::{ConfiguredEntity as _, ConfiguredUnit, DiagButton, EntityContext},
		payload::{SerializeExt as _, ServiceCommand, ServiceStatus},
		Error, Result,
	},
	error_stack::{bail, IntoReport as _, ResultExt as _},
	futures::{
		future, pin_mut,
		stream::{self, FuturesUnordered},
		Future, StreamExt as _, TryStreamExt as _,
	},
	hass_mqtt_client::{EntityTopic, HassMqttClient, HassMqttOptions, Message, QosLevel, StateTopic},
	log::{error, warn},
	serde::Serialize,
	std::{
		borrow::Cow,
		collections::HashMap,
		mem::ManuallyDrop,
		sync::{atomic::Ordering, Arc, Weak},
	},
	zbus_systemd::systemd1::ManagerProxy,
};

pub(crate) const QOS: QosLevel = QosLevel::AtLeastOnce;
pub(crate) type MqttTopic = Arc<str>;

pub(crate) struct MqttConnection<'c> {
	diag_button: Arc<EntityTopics>,
	diag_button_entity: DiagButton<'c>,
	entity_context: EntityContext<'c>,
	// TODO: activate: CommandTopic,
	units: HashMap<&'c str, Arc<EntityTopics>>,
}

pub(crate) struct EntityTopics {
	pub config_topic: EntityTopic,
	pub state_topic: StateTopic,
	pub command_topic: Option<MqttTopic>,
}

impl<'c> Core<'c> {
	pub async fn handle_message(&self, manager: &ManagerProxy<'_>, message: &Message) -> Result<bool> {
		let segments = message.topic().split('/').collect::<Vec<_>>();
		match &segments[..] {
			["systemd", hostname, ..] if *hostname != self.hostname() => (), // not for us, ignore
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

	pub async fn announce(&self) -> Result<()> {
		let futures = if let Some(mqtt) = &*self.mqtt.lock().await {
			let payload = ServiceStatus {
				is_active: true,
				units: self.units.keys().map(|&s| Cow::Borrowed(s)).collect(),
			};

			mqtt
				.diag_button
				.clone()
				.publish_state(&payload, true, QOS)
				.await
				.change_context(Error::HassMqtt)?;

			let futures = self
				.mqtt_units()
				.map(|(unit, topics)| {
					let unit = ConfiguredUnit::new(&mqtt.entity_context, &topics, unit);
					Ok(topics.publish_config(unit.try_encode_payload()?, true, QOS))
				})
				.collect::<Result<FuturesUnordered<_>>>()?;

			let button = mqtt
				.diag_button
				.clone()
				.publish_config(mqtt.diag_button_entity.try_encode_payload()?, true, QOS);
			stream::select(futures, stream::once(button))
		} else {
			return Ok(())
		};
		pin_mut!(futures);

		while let Some(_) = futures.try_next().await.change_context(Error::HassMqtt)? {}

		Ok(())
	}

	pub(crate) async fn connect_mqtt(&self) -> Result<Option<stream::BoxStream<Message>>> {
		async fn try_build(_options: HassMqttOptions) -> Result<HassMqttClient> {
			match () {
				#[cfg(feature = "paho")]
				() => _options
					.build_paho()
					.await
					.into_report()
					.change_context(Error::HassMqtt),
				#[allow(unreachable_patterns)]
				_ => Err(Error::NoMqttBackend).into_report(),
			}
		}

		if let Some(options) = self.mqtt_create()? {
			let client = try_build(options).await?;
			let entity_context = EntityContext::with_hostname(self.hostname());

			let config_topic = DiagButton::entity_topic(&client, &entity_context, &()).await?;
			let state_topic = config_topic.state_topic("status");
			let set = config_topic.command_topic("set", QOS);
			/* TODO: let activate = config_topic.command_topic("+/activate", QOS);
			let (set_command, activate) = future::try_join(set, activate)
				.await.change_context(Error::Entity)?;*/
			let set_command = set.await.into_report().change_context(Error::Entity)?;
			let command_topic = Some(set_command.topic());
			let diag_button = Arc::new(EntityTopics {
				config_topic,
				state_topic,
				command_topic,
			});
			let entity_context = entity_context.with_status_topic(diag_button.state_topic());

			let mut unit_futures: FuturesUnordered<_> = self
				.units
				.values()
				.map(|unit| {
					let unit_name = unit.unit_name().as_str();
					let unit_context = entity_context.clone();
					let unit_client = &client;
					async move {
						let config_topic = ConfiguredUnit::entity_topic(unit_client, &unit_context, &unit.unit).await?;
						let state_topic = config_topic.state_topic("status");
						let command = match unit.read_only {
							true => None,
							false => Some(
								config_topic
									.command_topic("activate", QOS)
									.await
									.into_report()
									.change_context(Error::Entity)?,
							),
						};
						let command_topic = command.as_ref().map(|c| c.topic());
						let topics = EntityTopics {
							config_topic,
							state_topic,
							command_topic,
						};
						let res: Result<_> = Ok(((unit_name, topics), command));
						res
					}
				})
				.collect();

			let mut commands = stream::SelectAll::new();
			let mut units = HashMap::new();
			while let Some(((name, topics), cmd)) = unit_futures.try_next().await? {
				let topics = Arc::new(topics);
				match self.units.get(name) {
					Some(unit) => drop(unit.set_topics(&topics)),
					_ => bail!(Error::InternalConsistency { unit_name: name.into() }),
				}
				units.insert(name, topics);
				if let Some(cmd) = cmd {
					commands.push(cmd);
				}
			}

			let connection = MqttConnection {
				diag_button_entity: DiagButton::new(&entity_context, &diag_button, Default::default()),
				diag_button,
				entity_context,
				units,
			};
			*self.mqtt.lock().await = Some(connection);

			// TODO: opts.will_message(self.mqtt_will());

			commands.push(set_command);
			Ok(Some(commands.boxed()))
		} else {
			Ok(None)
		}
	}

	fn mqtt_create(&self) -> crate::Result<Option<HassMqttOptions>> {
		let url = match self.cli.mqtt_url() {
			Some(url) => url,
			None => return Ok(None),
		};
		let host = url.host_str().ok_or(Error::UrlMissingHost)?;

		let builder = HassMqttOptions::new(host, "systemd2mqtt")
			.port(url.port().unwrap_or(1883))
			.node_id(self.hostname())
			.discovery_prefix(self.cli.discovery_prefix())
			.private_prefix("systemd");

		let tls = url.scheme() == "mqtts" || url.scheme() == "ssl";
		#[allow(unreachable_patterns)]
		let builder = match builder {
			#[cfg(feature = "tls")]
			builder => builder.tls(tls),
			_builder if tls => bail!(Error::NoTls),
			builder => builder,
		};

		Ok(Some(if let Some(user) = self.cli.mqtt_username() {
			builder.auth(user, self.cli.mqtt_password().unwrap_or_default())
		} else {
			builder
		}))
	}

	pub async fn disconnect(&self) -> Result<()> {
		let mqtt = self.mqtt.lock().await.take();
		if let Some(mqtt) = mqtt {
			let configs = FuturesUnordered::new();
			if self.cli.clean_up() {
				for topics in mqtt.units.values() {
					configs.push(topics.clone().publish_config(Vec::new().into(), true, QOS));
				}
			} else {
				// unset retain flag on entity configs
				for (unit, topics) in self.mqtt_units() {
					let unit = ConfiguredUnit::new(&mqtt.entity_context, &topics, unit);
					configs.push(topics.publish_config(unit.try_encode_payload()?, false, QOS));
				}
				configs.push(
					mqtt
						.diag_button
						.clone()
						.publish_config(mqtt.diag_button_entity.try_encode_payload()?, false, QOS),
				)
			}
			let offline_status = mqtt
				.diag_button
				.clone()
				.publish_state(&self.mqtt_will_payload(), !self.cli.clean_up(), QOS);
			let futures = stream::select(configs, stream::once(offline_status))
				.filter_map(|res| future::ready(res.err()))
				.enumerate();
			pin_mut!(futures);

			for unit in self.units.values() {
				unit.drop_topics();
			}

			while let Some((i, e)) = futures.next().await {
				if i == 0 {
					warn!("Failed to clean up when disconnecting from MQTT:");
				}
				warn!("{e:?}");
			}

			// TODO: explicit disconnection?
		}

		Ok(())
	}

	pub(crate) fn mqtt_will_payload(&self) -> ServiceStatus {
		let payload = ServiceStatus {
			is_active: false,
			units: Default::default(),
		};
		payload
	}

	pub(crate) fn mqtt_units<'s>(&'s self) -> impl Iterator<Item = (&'s Unit<'c>, Arc<EntityTopics>)> + 's {
		self.units.values().filter_map(|unit| match unit.topics() {
			Some(topics) => Some((unit, topics)),
			None => {
				error!(
					"internal consistency error: mqtt unit data missing for {}",
					unit.unit_name()
				);
				None
			},
		})
	}
}

impl<'c> MqttConnection<'c> {
	pub fn status_topic(&self) -> MqttTopic {
		self.diag_button.state_topic()
	}

	pub fn set_topic(&self) -> &MqttTopic {
		self.diag_button.expect_command_topic()
	}
}

impl EntityTopics {
	pub fn state_topic(&self) -> MqttTopic {
		self.state_topic.topic()
	}

	pub fn expect_command_topic(&self) -> &MqttTopic {
		self.command_topic.as_ref().expect("expected command topic")
	}

	pub async fn publish_config(self: Arc<Self>, payload: Arc<[u8]>, retain: bool, qos: QosLevel) -> Result<()> {
		self
			.config_topic
			.publish(payload, retain, qos)
			.await
			.into_report()
			.change_context(Error::HassMqtt)
	}

	pub async fn publish_state_payload(self: Arc<Self>, payload: Arc<[u8]>, retain: bool, qos: QosLevel) -> Result<()> {
		self
			.state_topic
			.publish(payload, retain, qos)
			.await
			.into_report()
			.change_context(Error::HassMqtt)
	}

	pub fn publish_state(
		self: Arc<Self>,
		payload: &impl Serialize,
		retain: bool,
		qos: QosLevel,
	) -> impl Future<Output = Result<()>> {
		self.publish_state_payload(payload.encode_payload(), retain, qos)
	}
}

impl<'a> Unit<'a> {
	fn mqtt_weak(&self) -> ManuallyDrop<Weak<EntityTopics>> {
		let ptr = self.mqtt.load(Ordering::SeqCst) as *const _;
		let weak = unsafe { Weak::from_raw(ptr) };
		ManuallyDrop::new(weak)
	}

	pub(crate) fn topics(&self) -> Option<Arc<EntityTopics>> {
		self.mqtt_weak().upgrade()
	}

	fn set_topics_weak(&self, mqtt: Weak<EntityTopics>) -> Weak<EntityTopics> {
		let prev = self.mqtt.swap(mqtt.into_raw() as *mut _, Ordering::SeqCst) as *const _;
		unsafe { Weak::from_raw(prev) }
	}

	pub(crate) fn set_topics(&self, mqtt: &Arc<EntityTopics>) -> Weak<EntityTopics> {
		self.set_topics_weak(Arc::downgrade(mqtt))
	}

	pub(crate) fn drop_topics(&self) {
		drop(self.set_topics_weak(Weak::new()));
	}
}
