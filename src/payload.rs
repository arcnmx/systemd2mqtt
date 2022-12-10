use {
	crate::cli::{Args, Unit},
	hass_mqtt_types::{Availability, BinarySensor, Button, Device, Document, EntityCategory, Switch},
	serde::{Deserialize, Serialize},
	std::{borrow::Cow, fmt::Debug},
};

const ON: &'static str = "ON";
const OFF: &'static str = "OFF";

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
			payload_available: Some(ON.into()),
			payload_not_available: Some(OFF.into()),
			value_template: Some(
				"\
				{% if value_json.is_active %}ON\
				{% else %}OFF\
				{% endif %}"
					.into(),
			),
		}
	}

	pub fn hass_diag_button(&self) -> Button {
		Button::new(self.mqtt_sub_topic())
			.unique_id(self.hass_device_id())
			.object_id(self.hass_device_id())
			.name(format!("{} reset", env!("CARGO_PKG_NAME")))
			.device(self.hass_device())
			.availability(vec![self.hass_availability()])
			.json_attributes_topic(self.mqtt_pub_topic())
			.payload_press(ServiceCommand::Set { active: false }.encode())
			.entity_category(EntityCategory::Diagnostic)
	}

	pub fn hass_announce(&self, config: &dyn Entity, retain: bool) -> Result<paho_mqtt::Message, serde_json::Error> {
		let payload = config.to_json()?;
		let new = if retain {
			paho_mqtt::Message::new_retained
		} else {
			paho_mqtt::Message::new
		};
		Ok(new(self.hass_config_topic(config), payload, paho_mqtt::QOS_0))
	}

	pub fn hass_config_topic(&self, config: &dyn Entity) -> String {
		self.hass_config_topic_with(config.platform(), config.unique_id())
	}

	pub fn hass_config_topic_with(&self, platform: &'static str, unique_id: &str) -> String {
		format!("{}/{}/{}/config", self.discovery_prefix, platform, unique_id)
	}

	pub fn hass_device_id(&self) -> String {
		format!("{}_{}", env!("CARGO_PKG_NAME"), self.hostname())
	}

	pub fn hass_device_identifiers(&self) -> impl IntoIterator<Item = String> {
		vec!["name".into(), format!("{}-{}", env!("CARGO_PKG_NAME"), self.hostname())]
	}
}

impl<'a> Unit<'a> {
	pub fn to_hass_config(&self) -> Box<dyn Entity + 'a> {
		match self.hass_platform() {
			"switch" => Box::new(self.hass_config_switch()) as Box<_>,
			"binary_sensor" => Box::new(self.hass_config_sensor()) as Box<_>,
			p => unimplemented!("{p} platform for {}", self.unit_name()),
		}
	}

	pub fn hass_config_switch<'s>(&'s self) -> Switch<'a> {
		let (pon, poff, son, soff) = match self.unit.invert_state {
			false => (UnitCommand::Start, UnitCommand::Stop, ON, OFF),
			true => (UnitCommand::Stop, UnitCommand::Start, OFF, ON),
		};
		let mut switch = Switch::new(self.mqtt_sub_topic())
			.unique_id(self.unique_id())
			.object_id(self.object_id())
			.entity_category(self.entity_category)
			.device_class(self.device_class)
			.enabled_by_default(self.enabled_by_default)
			.name(self.name())
			.device(self.cli.hass_device())
			.availability(vec![self.hass_availability()])
			.json_attributes_topic(self.mqtt_pub_topic())
			.state_topic(self.mqtt_pub_topic())
			.payload_on(pon.encode())
			.payload_off(poff.encode())
			.state_on(son)
			.state_off(soff)
			.value_template(
				"\
				{% if value_json.active_state in ['active', 'activating', 'deactivating'] %}ON\
				{% else %}OFF\
				{% endif %}",
			);
		switch.icon = self.icon().map(|s| s[..].into());
		switch
	}

	pub fn hass_config_sensor<'s>(&'s self) -> BinarySensor<'a> {
		let (on, off) = match self.unit.invert_state {
			false => (ON, OFF),
			true => (OFF, ON),
		};
		let mut sensor = BinarySensor::new(self.mqtt_pub_topic())
			.unique_id(self.unique_id())
			.object_id(self.object_id())
			.entity_category(self.entity_category)
			.device_class(self.device_class)
			.enabled_by_default(self.enabled_by_default)
			.name(self.name())
			.device(self.cli.hass_device())
			.availability(vec![self.hass_availability()])
			.json_attributes_topic(self.mqtt_pub_topic())
			.payload_on(on)
			.payload_off(off)
			.value_template(
				"\
				{% if value_json.active_state in ['active', 'activating', 'deactivating'] %}ON\
				{% else %}OFF\
				{% endif %}",
			);
		sensor.icon = self.icon().map(|s| s[..].into());
		sensor
	}

	pub fn hass_availability(&self) -> Availability<'static> {
		Availability {
			topic: self.cli.mqtt_pub_topic().into(),
			payload_available: Some(ON.into()),
			payload_not_available: Some(OFF.into()),
			value_template: Some(
				format!(
					"\
					{{% if value_json.is_active and '{}' in value_json.units %}}ON\
					{{% else %}}OFF\
					{{% endif %}}",
					self.unit_name(),
				)
				.into(),
			),
		}
	}
}

type JsonSerializer<'w> = serde_json::Serializer<&'w mut Vec<u8>>;

pub trait Entity: Debug {
	fn unique_id(&self) -> &str;
	fn platform(&self) -> &'static str;
	fn serialize_json(&self, serializer: &mut JsonSerializer) -> Result<(), serde_json::Error>;

	fn to_json(&self) -> serde_json::Result<Vec<u8>> {
		let mut data = Vec::new();
		let mut ser = JsonSerializer::new(&mut data);
		self.serialize_json(&mut ser)?;
		Ok(data)
	}
}

impl<'a> Unit<'a> {
	pub fn hass_config<'u>(&'u self) -> &'u (dyn Entity + 'a) {
		self.config.get_or_init(|| self.to_hass_config()).as_ref()
	}

	pub fn hass_announce(&self, retain: bool) -> serde_json::Result<paho_mqtt::Message> {
		self.cli.hass_announce(self.hass_config(), retain)
	}

	pub fn hass_config_topic(&self) -> String {
		self.cli.hass_config_topic_with(self.platform(), &self.unique_id())
	}
}

impl<'a> Entity for Unit<'a> {
	fn unique_id(&self) -> &str {
		self.hass_config().unique_id()
	}

	fn platform(&self) -> &'static str {
		self.hass_config().platform()
	}

	fn serialize_json(&self, serializer: &mut JsonSerializer) -> Result<(), serde_json::Error> {
		self.hass_config().serialize_json(serializer)
	}
}

macro_rules! impl_entity {
	($($ty:ident = $platform:literal,)*) => {
		$(
			impl<'a> Entity for hass_mqtt_types::$ty<'a> {
				fn platform(&self) -> &'static str {
					$platform
				}

				fn unique_id(&self) -> &str {
					self.unique_id.as_ref()
						.expect("valid unique_id")
				}

				fn serialize_json(&self, serializer: &mut JsonSerializer) -> Result<(), serde_json::Error> {
					Document::serialize(self, serializer)
				}
			}
		)*
	};
}

impl_entity! {
	Switch = "switch", Button = "button",
	Sensor = "sensor", BinarySensor = "binary_sensor",
}
