use {
	crate::cli::Args,
	hass_mqtt_discovery::{
		availability::Availability,
		device::Device,
		entity::{Entity, Switch},
		entity_category::EntityCategory,
	},
	serde::{Deserialize, Serialize},
	std::borrow::Cow,
};

#[derive(Serialize, Debug)]
pub struct ServiceStatus<'a> {
	pub is_active: bool,
	#[serde(borrow)]
	pub units: Vec<Cow<'a, str>>,
}

impl ServiceStatus<'_> {
	pub fn encode(&self) -> String {
		serde_json::to_string(self).unwrap()
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServiceCommand {
	Set { active: bool },
}

impl ServiceCommand {
	pub fn encode(&self) -> String {
		serde_json::to_string(self).unwrap()
	}
}

#[derive(Serialize, Debug, Default)]
pub struct UnitStatus {
	pub load_state: String,
	pub active_state: String,
	pub id: String,
	pub invocation_id: Vec<u8>,
	pub description: String,
	pub transient: bool,
}

impl UnitStatus {
	pub fn encode(&self) -> String {
		serde_json::to_string(self).unwrap()
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UnitCommand {
	Start,
	Stop,
	Restart,
}

impl UnitCommand {
	pub fn encode(&self) -> String {
		serde_json::to_string(self).unwrap()
	}
}

impl Args {
	pub fn hass_device(&self) -> Device {
		Device {
			identifiers: self.hass_device_identifiers().into_iter().map(|id| id.into()).collect(),
			manufacturer: Some(env!("CARGO_PKG_AUTHORS").into()),
			model: Some(env!("CARGO_PKG_NAME").into()),
			name: Some(self.hostname().to_string().into()),
			sw_version: Some(env!("CARGO_PKG_VERSION").into()),
			configuration_url: Some(env!("CARGO_PKG_HOMEPAGE").into()),
			// hw_version: Some(version),
			..Default::default()
		}
	}

	pub fn hass_availability(&self) -> Availability {
		Availability {
			topic: self.mqtt_pub_topic().into(),
			payload_available: Some("ON".into()),
			payload_not_available: Some("OFF".into()),
			value_template: Some("{% if value_json.is_active %}ON{% else %}OFF{% endif %}".into()),
		}
	}

	pub fn hass_availability_unit(&self, unit: &str) -> Availability {
		Availability {
			topic: self.mqtt_pub_topic().into(),
			payload_available: Some("ON".into()),
			payload_not_available: Some("OFF".into()),
			value_template: Some(
				format!(
					"{{% if value_json.is_active and '{}' in value_json.units %}}ON{{% else %}}OFF{{% endif %}}",
					unit
				)
				.into(),
			),
		}
	}

	pub fn hass_global_state(&self) -> Switch {
		Switch {
			entity: Entity {
				unique_id: Some(self.hass_device_id().into()),
				object_id: Some(self.hass_device_id().into()),
				name: Some(env!("CARGO_PKG_NAME").into()),
				device: Some(self.hass_device()),
				availability: vec![self.hass_availability()].into(),
				..Default::default()
			},
			command_topic: self.mqtt_sub_topic().into(),
			payload_on: Some(ServiceCommand::Set { active: true }.encode().into()),
			payload_off: Some(ServiceCommand::Set { active: false }.encode().into()),
			state_topic: Some(self.mqtt_pub_topic().into()),
			state_on: Some("ON".into()),
			state_off: Some("OFF".into()),
			value_template: Some("{% if value_json.is_active %}ON{% else %}OFF{% endif %}".into()),
			device_class: None,
			optimistic: None,
			retain: None,
		}
	}

	pub fn hass_unit_switch(&self, unit: &str) -> Switch {
		Switch {
			entity: Entity {
				unique_id: Some(self.hass_unique_id(unit).into()),
				object_id: Some(self.hass_unique_id(unit).into()),
				entity_category: self.hass_entity_category(unit),
				name: Some(self.hass_entity_name(unit).into()),
				device: Some(self.hass_device()),
				availability: vec![self.hass_availability_unit(unit)].into(),
				..Default::default()
			},
			command_topic: self.mqtt_sub_topic_unit(unit).into(),
			payload_on: Some(UnitCommand::Start.encode().into()),
			payload_off: Some(UnitCommand::Stop.encode().into()),
			state_topic: Some(self.mqtt_pub_topic_unit(unit).into()),
			state_off: Some("OFF".into()),
			state_on: Some("ON".into()),
			value_template: Some(
				format!(
					"{{% if {} %}}ON{{% else %}}OFF{{% endif %}}",
					"value_json.active_state in ['active', 'activating', 'deactivating']",
				)
				.into(),
			),
			device_class: None,
			optimistic: None,
			retain: None,
		}
	}

	pub fn hass_announce_entity<E: Serialize>(&self, retain: bool, config: &E, entity: &Entity) -> paho_mqtt::Message {
		let payload = serde_json::to_string(config).unwrap();
		let new = if retain {
			paho_mqtt::Message::new_retained
		} else {
			paho_mqtt::Message::new
		};
		new(
			self.hass_config_topic(entity.unique_id.as_ref().unwrap()),
			payload,
			paho_mqtt::QOS_0,
		)
	}

	pub fn hass_device_id(&self) -> String {
		format!("{}_{}", env!("CARGO_PKG_NAME"), self.hostname())
	}

	pub fn hass_device_identifiers(&self) -> impl IntoIterator<Item = String> {
		vec!["name".into(), format!("{}-{}", env!("CARGO_PKG_NAME"), self.hostname())]
	}

	pub fn hass_entity_category(&self, unit: &str) -> EntityCategory {
		let is_config = true;
		match is_config {
			true => EntityCategory::Config,
			false => EntityCategory::None,
		}
	}

	pub fn hass_unique_id(&self, unit: &str) -> String {
		format!("{}_{}", self.hostname(), Self::unit_short_name(unit))
	}

	pub fn hass_entity_name(&self, unit: &str) -> String {
		Self::unit_short_name(unit).into()
	}

	pub fn hass_config_topic(&self, unique_id: &str) -> String {
		format!("{}/switch/{}/config", self.discovery_prefix, unique_id)
	}

	pub fn hass_config_topic_unit(&self, unit: &str) -> String {
		self.hass_config_topic(&self.hass_unique_id(unit))
	}
}
