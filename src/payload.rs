use std::borrow::Cow;
use hass_mqtt_discovery::{
	availability::{Availability, AvailabilityMode},
	template::Template,
	topic::Topic,
	device::Device,
	device_class::DeviceClass,
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

#[derive(Serialize, Debug, Default)]
pub struct Switch<'a> {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub availability: Vec<Availability<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub availability_mode: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub availability_template: Option<Template<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub availability_topic: Option<Topic<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub command_topic: Option<Topic<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub device: Option<Device<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub device_class: Option<DeviceClass>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub enabled_by_default: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub encoding: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub entity_category: Option<EntityCategory>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub icon: Option<Cow<'a, str>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub json_attributes_template: Option<Template<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub json_attributes_topic: Option<Topic<'a>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<Cow<'a, str>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub object_id: Option<Cow<'a, str>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub optimistic: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payload_available: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payload_not_available: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payload_off: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub payload_on: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub qos: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub retain: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub state_off: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub state_on: Option<&'static str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub state_topic: Option<Topic<'a>>,
	pub unique_id: Cow<'a, str>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub value_template: Option<Template<'a>>,
}

impl<'a> Switch<'a> {
	pub fn new(cli: &'a Args, unit: &'a str) -> Self {
		Switch {
			unique_id: cli.hass_unique_id(unit).into(),
			entity_category: Some(cli.hass_entity_category(unit)),
			name: Some(cli.hass_entity_name(unit).into()),
			device: Some(cli.hass_device()),
			payload_off: Some(OFF.into()),
			payload_on: Some(ON.into()),
			state_topic: Some(cli.mqtt_pub_topic_unit(unit).into()),
			value_template: Some(format!(
				"{{% if {} %}}{}{{% else %}}{}{{% endif %}}",
				"value_json['active_state'] == 'active' or value_json['active_state'] == 'activating' or value_json['active_state'] == 'deactivating'",
				ON, OFF,
			).into()),
			command_topic: Some(cli.mqtt_sub_topic_unit(unit).into()),
			.. Default::default()
		}
	}

	pub fn announce(&self, cli: &Args) -> paho_mqtt::Message {
		let payload = serde_json::to_string(self).unwrap();
		paho_mqtt::Message::new(
			format!("{}/switch/{}/config", cli.discovery_prefix, self.unique_id),
			payload,
			paho_mqtt::QOS_0,
		)
	}
}

impl Args {
	pub fn hass_device(&self) -> Device {
		Device {
			connections: Default::default(),
			identifiers: self.hass_device_identifiers().into_iter()
				.map(|id| id.into())
				.collect(),
			manufacturer: Some(env!("CARGO_PKG_AUTHORS").into()),
			model: Some(env!("CARGO_PKG_NAME").into()),
			name: Some(self.hostname().to_string().into()),
			suggested_area: None,
			sw_version: Some(env!("CARGO_PKG_VERSION").into()),
			via_device: None,
			// configuration_url: env!("CARGO_PKG_HOMEPAGE"),
			// hw_version: Some(version),
		}
	}

	pub fn hass_global_switch(&self) -> Switch {
		Switch {
			unique_id: self.hass_device_id().into(),
			entity_category: Some(EntityCategory::None),
			name: Some(env!("CARGO_PKG_NAME").into()),
			device: Some(self.hass_device()),
			state_topic: Some(self.mqtt_pub_topic().into()),
			command_topic: Some(self.mqtt_pub_topic().into()),
			.. Default::default()
		}
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
