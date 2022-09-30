use hass_mqtt_discovery::{
	device::Device,
	entity::{Entity, Switch},
	entity_category::EntityCategory,
};
use serde::Serialize;
use crate::cli::Args;

pub const ON: &'static str = "active";
pub const OFF: &'static str = "inactive";
pub const RESTART: &'static str = "restart";

#[derive(Serialize, Debug, Default)]
pub struct UnitStatus {
	pub load_state: String,
	pub active_state: String,
	pub id: String,
	pub invocation_id: Vec<u8>,
	pub description: String,
	pub transient: bool,
}

impl Args {
	pub fn hass_device(&self) -> Device {
		Device {
			identifiers: self.hass_device_identifiers().into_iter()
				.map(|id| id.into())
				.collect(),
			manufacturer: Some(env!("CARGO_PKG_AUTHORS").into()),
			model: Some(env!("CARGO_PKG_NAME").into()),
			name: Some(self.hostname().to_string().into()),
			sw_version: Some(env!("CARGO_PKG_VERSION").into()),
			configuration_url: Some(env!("CARGO_PKG_HOMEPAGE").into()),
			// hw_version: Some(version),
			.. Default::default()
		}
	}

	pub fn hass_global_switch(&self) -> Switch {
		Switch {
			entity: Entity {
				unique_id: Some(self.hass_device_id().into()),
				name: Some(env!("CARGO_PKG_NAME").into()),
				device: Some(self.hass_device()),
				.. Default::default()
			},
			state_topic: Some(self.mqtt_pub_topic().into()),
			command_topic: Some(self.mqtt_pub_topic().into()),
			.. Default::default()
		}
	}

	pub fn hass_unit_switch(&self, unit: &str) -> Switch {
		Switch {
			entity: Entity {
				unique_id: Some(self.hass_unique_id(unit).into()),
				entity_category: self.hass_entity_category(unit),
				name: Some(self.hass_entity_name(unit).into()),
				device: Some(self.hass_device()),
				.. Default::default()
			},
			payload_off: Some(OFF.into()),
			payload_on: Some(ON.into()),
			state_topic: Some(self.mqtt_pub_topic_unit(unit).into()),
			value_template: Some(format!(
				"{{% if {} %}}{}{{% else %}}{}{{% endif %}}",
				"value_json['active_state'] == 'active' or value_json['active_state'] == 'activating' or value_json['active_state'] == 'deactivating'",
				ON, OFF,
			).into()),
			command_topic: Some(self.mqtt_sub_topic_unit(unit).into()),
			.. Default::default()
		}
	}

	pub fn hass_announce_switch(&self, switch: &Switch) -> paho_mqtt::Message {
		let payload = serde_json::to_string(switch).unwrap();
		paho_mqtt::Message::new(
			format!("{}/switch/{}/config", self.discovery_prefix, switch.entity.unique_id.as_ref().unwrap()),
			payload,
			paho_mqtt::QOS_0,
		)
	}


	pub fn hass_device_id(&self) -> String {
		format!("{}-{}", env!("CARGO_PKG_NAME"), self.hostname())
	}

	pub fn hass_device_identifiers(&self) -> impl IntoIterator<Item=String> {
		vec!["name".into(), self.hass_device_id()]
	}

	pub fn hass_entity_category(&self, unit: &str) -> EntityCategory {
		let is_config = true;
		match is_config {
			true => EntityCategory::Config,
			false => EntityCategory::None,
		}
	}

	pub fn hass_unique_id(&self, unit: &str) -> String {
		format!("{}-{}", self.hass_device_id(), Self::unit_short_name(unit))
	}

	pub fn hass_entity_name(&self, unit: &str) -> String {
		Self::unit_short_name(unit).into()
	}
}
