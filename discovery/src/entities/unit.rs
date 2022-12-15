use {
	crate::{
		entities::{
			ConfiguredDevice, ConfiguredEntity, EntityContext, EntityDocument, EntityIds, EntityObject, StaticEntity as _,
		},
		EntityTopics, MqttTopic,
	},
	hass_mqtt_types::{Availability, BinarySensor, DeviceClass, EntityCategory, Switch, UniqueId},
	serde::{Serialize, Serializer},
	std::{borrow::Cow, slice},
	systemd2mqtt_payload::{SerializeExt as _, UnitCommand, UnitConfig, OFF, ON},
};

#[derive(Debug, Clone)]
pub struct UnitSwitch<'i> {
	pub device: ConfiguredDevice<'i>,
	pub command_topic: MqttTopic,
	pub state_topic: MqttTopic,
	pub availability: Availability<'i>,
	pub unique_id: String,
	pub object_id: String,
	pub name: String,
	pub payloads: (String, String),
	pub states: (&'static str, &'static str),
	pub entity_category: EntityCategory,
	pub device_class: DeviceClass,
	pub enabled_by_default: bool,
	pub icon: Option<&'i str>,
}

impl<'i> ConfiguredEntity<'i> for UnitSwitch<'i> {
	type Args<'a> = &'i UnitConfig where Self: 'a;

	fn new_unique_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		format!(
			"{}_{}",
			ConfiguredDevice::device_id(context.hostname()),
			unit.unit.replace(".", "_")
		)
		.into()
	}

	fn new_short_id<'a>(_context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		unit.short_name().into()
	}

	fn new_object_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		let object_id = unit.object_id.as_ref();
		object_id
			.map(|id| Cow::Borrowed(&id[..]))
			.unwrap_or_else(|| Self::format_object_id(context, &Self::new_short_id(context, unit)))
	}

	fn new_name<'a>(_context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		unit.name().into()
	}

	fn new_domain<'a>(_context: &EntityContext<'i>, _unit: &Self::Args<'a>) -> &'static str
	where
		Self: 'a,
	{
		Self::DOMAIN
	}

	fn new<'a>(context: &EntityContext<'i>, topics: &EntityTopics, unit: Self::Args<'a>) -> Self
	where
		Self: 'a,
	{
		let (device, availability) = context.to_parts();
		let EntityIds {
			unique_id,
			object_id,
			name,
		} = Self::new_ids(context, &unit);
		let (pon, poff, son, soff) = match unit.invert_state {
			false => (UnitCommand::Start, UnitCommand::Stop, ON, OFF),
			true => (UnitCommand::Stop, UnitCommand::Start, OFF, ON),
		};

		Self {
			unique_id: unique_id.into(),
			object_id: object_id.into(),
			name: name.into(),
			availability: availability.with_unit_name(unit.unit.as_str()).into_availability(),
			command_topic: topics.expect_command_topic().clone(),
			device,
			state_topic: topics.state_topic(),
			states: (son, soff),
			payloads: (pon.encode_str(), poff.encode_str()),
			entity_category: unit.entity_category,
			device_class: unit.device_class,
			enabled_by_default: unit.enabled_by_default,
			icon: unit.icon.as_ref().map(|s| s.as_str()),
		}
	}
}

impl<'i> UnitSwitch<'i> {
	pub fn as_switch(&self) -> Switch {
		let (pon, poff) = &self.payloads;
		let (son, soff) = self.states;
		let switch = Switch::new(&self.command_topic[..])
			.unique_id(&self.unique_id[..])
			.object_id(&self.object_id[..])
			.name(&self.name[..])
			.entity_category(self.entity_category)
			.device_class(self.device_class)
			.enabled_by_default(self.enabled_by_default)
			.availability(slice::from_ref(&self.availability))
			.device(&self.device)
			.json_attributes_topic(&self.state_topic[..])
			.state_topic(&self.state_topic[..])
			.payload_on(pon.as_str())
			.payload_off(poff.as_str())
			.state_on(son)
			.state_off(soff)
			.value_template(
				"\
				{% if value_json.active_state in ['active', 'activating', 'deactivating'] %}ON\
				{% else %}OFF\
				{% endif %}",
			);
		if let Some(icon) = self.icon {
			switch.icon(icon)
		} else {
			switch
		}
	}
}

#[derive(Debug, Clone)]
pub struct UnitSensor<'i> {
	pub device: ConfiguredDevice<'i>,
	pub state_topic: MqttTopic,
	pub availability: Availability<'i>,
	pub unique_id: String,
	pub object_id: String,
	pub name: String,
	pub states: (&'static str, &'static str),
	pub entity_category: EntityCategory,
	pub device_class: DeviceClass,
	pub enabled_by_default: bool,
	pub icon: Option<&'i str>,
}

impl<'i> ConfiguredEntity<'i> for UnitSensor<'i> {
	type Args<'a> = &'i UnitConfig where Self: 'a;

	fn new_unique_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_unique_id(context, unit)
	}

	fn new_short_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_short_id(context, unit)
	}

	fn new_object_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_object_id(context, unit)
	}

	fn new_name<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_name(context, unit)
	}

	fn new_domain<'a>(_context: &EntityContext<'i>, _unit: &Self::Args<'a>) -> &'static str
	where
		Self: 'a,
	{
		Self::DOMAIN
	}

	fn new<'a>(context: &EntityContext<'i>, topics: &EntityTopics, unit: Self::Args<'a>) -> Self
	where
		Self: 'a,
	{
		let (device, availability) = context.to_parts();
		let EntityIds {
			unique_id,
			object_id,
			name,
		} = Self::new_ids(context, &unit);
		let (on, off) = match unit.invert_state {
			false => (ON, OFF),
			true => (OFF, ON),
		};

		Self {
			unique_id: unique_id.into(),
			object_id: object_id.into(),
			name: name.into(),
			availability: availability.with_unit_name(unit.unit.as_str()).into_availability(),
			device,
			state_topic: topics.state_topic(),
			states: (on, off),
			entity_category: unit.entity_category,
			device_class: unit.device_class,
			enabled_by_default: unit.enabled_by_default,
			icon: unit.icon.as_ref().map(|s| s.as_str()),
		}
	}
}

impl<'i> UnitSensor<'i> {
	pub fn as_sensor(&self) -> BinarySensor {
		let (son, soff) = self.states;
		let sensor = BinarySensor::new(&self.state_topic[..])
			.unique_id(self.unique_id.as_str())
			.object_id(&self.object_id[..])
			.name(&self.name[..])
			.entity_category(self.entity_category)
			.device_class(self.device_class)
			.enabled_by_default(self.enabled_by_default)
			.availability(slice::from_ref(&self.availability))
			.device(&self.device)
			.json_attributes_topic(&self.state_topic[..])
			.payload_on(son)
			.payload_off(soff)
			.value_template(
				"\
				{% if value_json.active_state in ['active', 'activating', 'deactivating'] %}ON\
				{% else %}OFF\
				{% endif %}",
			);
		if let Some(icon) = self.icon {
			sensor.icon(icon)
		} else {
			sensor
		}
	}
}

#[derive(Debug, Clone)]
pub enum ConfiguredUnit<'i> {
	Switch(UnitSwitch<'i>),
	Sensor(UnitSensor<'i>),
}

impl<'i> EntityObject for ConfiguredUnit<'i> {
	fn unique_id(&self) -> Option<UniqueId> {
		match self {
			Self::Switch(entity) => entity.unique_id(),
			Self::Sensor(entity) => entity.unique_id(),
		}
	}

	fn object_id(&self) -> Option<&str> {
		match self {
			Self::Switch(entity) => entity.object_id(),
			Self::Sensor(entity) => entity.object_id(),
		}
	}

	fn domain(&self) -> &str {
		match self {
			Self::Switch(entity) => entity.domain(),
			Self::Sensor(entity) => entity.domain(),
		}
	}
}

impl<'i> EntityDocument for ConfiguredUnit<'i> {
	type Document<'o> = &'o Self where Self: 'o;

	fn to_document<'o>(&'o self) -> Self::Document<'o> {
		self
	}
}

impl<'i> ConfiguredEntity<'i> for ConfiguredUnit<'i> {
	type Args<'a> = &'i UnitConfig where Self: 'a;

	fn new_unique_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_unique_id(context, unit)
	}

	fn new_short_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_short_id(context, unit)
	}

	fn new_object_id<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_object_id(context, unit)
	}

	fn new_name<'a>(context: &EntityContext<'i>, unit: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		UnitSwitch::new_name(context, unit)
	}

	fn new_domain<'a>(_context: &EntityContext<'i>, unit: &Self::Args<'a>) -> &'static str
	where
		Self: 'a,
	{
		match unit.read_only {
			true => UnitSensor::DOMAIN,
			false => UnitSwitch::DOMAIN,
		}
	}

	fn new<'a>(context: &EntityContext<'i>, topics: &EntityTopics, unit: Self::Args<'a>) -> Self
	where
		Self: 'a,
	{
		match Self::new_domain(&context, &unit) {
			UnitSwitch::DOMAIN => Self::Switch(UnitSwitch::new(context, topics, unit)),
			UnitSensor::DOMAIN => Self::Sensor(UnitSensor::new(context, topics, unit)),
			domain => unreachable!("unsupported domain {}", domain),
		}
	}
}

impl<'i> Serialize for ConfiguredUnit<'i> {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		match self {
			Self::Switch(entity) => entity.serialize(s),
			Self::Sensor(entity) => entity.serialize(s),
		}
	}
}
