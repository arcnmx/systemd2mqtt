pub(crate) use self::{
	diag::DiagButton,
	unit::{ConfiguredUnit, UnitSensor, UnitSwitch},
};
use {
	crate::{
		core::{EntityTopics, MqttTopic},
		payload::{OFF, ON, PKG_NAME},
	},
	error_stack::{FutureExt as _, IntoReport as _},
	futures::{Future, FutureExt as _},
	hass_mqtt_client::{EntityTopic, HassMqttClient},
	hass_mqtt_types::{Availability, Device, UniqueId},
	serde::{Serialize, Serializer},
	std::{borrow::Cow, fmt::Display, pin::Pin},
};

mod diag;
mod unit;

pub trait StaticEntity: EntityObject {
	const DOMAIN: &'static str;
}

pub trait EntityObject: Serialize {
	fn unique_id(&self) -> Option<UniqueId>;
	fn object_id(&self) -> Option<&str>;
	fn domain(&self) -> &str;
}

pub trait EntityDocument: EntityObject {
	type Document<'o>: Serialize + Sized
	where
		Self: 'o;

	fn to_document<'o>(&'o self) -> Self::Document<'o>;
}

pub(crate) trait ConfiguredEntity<'i>: EntityDocument + Sized {
	type Args<'a>: Clone
	where
		Self: 'a;

	fn new_unique_id<'a>(context: &EntityContext<'i>, args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a;
	fn new_short_id<'a>(context: &EntityContext<'i>, args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a;
	fn new_name<'a>(context: &EntityContext<'i>, args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a;
	fn new_domain<'a>(context: &EntityContext<'i>, args: &Self::Args<'a>) -> &'static str
	where
		Self: 'a;

	fn format_object_id<'a>(context: &EntityContext<'i>, short_id: &str) -> Cow<'i, str>
	where
		Self: 'a,
	{
		format!("{}_{short_id}", context.hostname()).into()
	}

	fn new_object_id<'a>(context: &EntityContext<'i>, args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		Self::format_object_id(context, &Self::new_short_id(context, args)).into()
	}

	fn new_ids<'a>(context: &EntityContext<'i>, args: &Self::Args<'a>) -> EntityIds<'i>
	where
		Self: 'a,
	{
		EntityIds {
			unique_id: Self::new_unique_id(context, args),
			object_id: Self::new_object_id(context, args),
			name: Self::new_name(context, args),
		}
	}

	fn new<'a>(context: &EntityContext<'i>, topics: &EntityTopics, args: Self::Args<'a>) -> Self
	where
		Self: 'a;

	fn entity_topic<'a>(
		client: &'a HassMqttClient,
		context: &EntityContext<'i>,
		args: &Self::Args<'a>,
	) -> Pin<Box<dyn Future<Output = crate::Result<EntityTopic>> + 'a>>
	where
		Self: 'a,
	{
		client
			.entity(
				MqttTopic::from(Self::new_domain(context, args)),
				MqttTopic::from(Self::new_short_id(context, args)),
			)
			.map(|res| res.into_report())
			.change_context(crate::Error::Entity)
			.boxed()
	}
}

impl_entity! { @document
	Button = "button",
	Switch = "switch",
	Light = "light",
	Sensor = "sensor",
	BinarySensor = "binary_sensor",
	Cover = "cover",
	DeviceTracker = "device_tracker",
}

impl_entity! { @wrapper
	DiagButton[as_button] = Button,
	UnitSwitch[as_switch] = Switch,
	UnitSensor[as_sensor] = BinarySensor,
}

#[derive(Debug, Clone)]
pub(crate) struct EntityContext<'i> {
	pub device: ConfiguredDevice<'i>,
	pub availability: ConfiguredAvailability<'i>,
}

impl<'i> EntityContext<'i> {
	pub fn new(hostname: &'i str, status_topic: MqttTopic) -> Self {
		Self {
			device: ConfiguredDevice::with_hostname(hostname),
			availability: ConfiguredAvailability::with_status_topic(status_topic),
		}
	}

	pub fn with_status_topic(self, status_topic: MqttTopic) -> Self {
		Self {
			device: self.device,
			availability: ConfiguredAvailability::with_status_topic(status_topic),
		}
	}

	pub fn with_hostname(hostname: &'i str) -> Self {
		Self::new(hostname.as_ref(), String::new().into())
	}

	pub fn hostname(&self) -> &str {
		&self.device.hostname
	}

	pub fn to_parts(&self) -> (ConfiguredDevice<'i>, ConfiguredAvailability<'i>) {
		(self.device.clone(), self.availability.clone())
	}
}

pub(crate) struct EntityIds<'i> {
	pub unique_id: Cow<'i, str>,
	pub object_id: Cow<'i, str>,
	pub name: Cow<'i, str>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredDevice<'i> {
	pub hostname: Cow<'i, str>,
	pub identifiers: [Cow<'static, str>; 1],
}

impl<'i> ConfiguredDevice<'i> {
	fn with_hostname(hostname: impl Into<Cow<'i, str>>) -> Self {
		let hostname = hostname.into();
		Self {
			identifiers: Self::identifiers(&hostname),
			hostname,
		}
	}

	pub fn as_device(&self) -> Device {
		Device {
			identifiers: self.identifiers[..].into(),
			manufacturer: Some(env!("CARGO_PKG_AUTHORS").into()),
			model: Some(PKG_NAME.into()),
			name: Some(self.hostname[..].into()),
			sw_version: Some(env!("CARGO_PKG_VERSION").into()),
			configuration_url: Some(env!("CARGO_PKG_HOMEPAGE").into()),
			// hw_version: Some(version),
			..Default::default()
		}
	}

	pub fn device_id_(hostname: &(impl Display + ?Sized), sep: char) -> String {
		format!("{PKG_NAME}{sep}{hostname}")
	}

	pub fn device_id(hostname: &(impl Display + ?Sized)) -> String {
		Self::device_id_(hostname, '_')
	}

	pub fn identifiers(hostname: &(impl Display + ?Sized)) -> [Cow<'static, str>; 1] {
		[Self::device_id_(hostname, '-').into()]
	}
}

impl<'r: 'o, 'i: 'o, 'o> Into<Device<'o>> for &'r ConfiguredDevice<'i> {
	fn into(self) -> Device<'o> {
		self.as_device()
	}
}

impl<'i, T: Into<Cow<'i, str>>> From<T> for ConfiguredDevice<'i> {
	fn from(hostname: T) -> Self {
		Self::with_hostname(hostname.into())
	}
}

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredAvailability<'i> {
	pub status_topic: MqttTopic,
	pub unit_name: Option<Cow<'i, str>>,
}

impl<'i> ConfiguredAvailability<'i> {
	pub fn new(unit_name: impl Into<Option<&'i str>>, status_topic: MqttTopic) -> Self {
		Self {
			status_topic,
			unit_name: unit_name.into().map(Cow::Borrowed),
		}
	}

	pub fn with_status_topic(status_topic: MqttTopic) -> Self {
		Self::new(None, status_topic)
	}

	pub fn with_unit_name(self, unit_name: impl Into<Cow<'i, str>>) -> Self {
		Self {
			unit_name: Some(unit_name.into()),
			status_topic: self.status_topic,
		}
	}

	pub fn value_template(&self) -> Cow<'static, str> {
		match &self.unit_name {
			Some(unit_name) => format!(
				"\
				{{% if value_json.is_active and '{unit_name}' in value_json.units %}}{ON}\
				{{% else %}}{OFF}\
				{{% endif %}}"
			)
			.into(),
			None => "\
				{% if value_json.is_active %}ON\
				{% else %}OFF\
				{% endif %}"
				.into(),
		}
	}

	pub fn as_availability(&self) -> Availability {
		Availability {
			topic: self.status_topic[..].into(),
			payload_available: Some(ON.into()),
			payload_not_available: Some(OFF.into()),
			value_template: Some(self.value_template().into()),
		}
	}

	pub fn into_availability(self) -> Availability<'i> {
		Availability {
			value_template: Some(self.value_template().into()),
			topic: self.status_topic[..].to_owned().into(), // XXX: ugh :<
			payload_available: Some(ON.into()),
			payload_not_available: Some(OFF.into()),
		}
	}
}

impl<'r: 'o, 'i: 'o, 'o> Into<Availability<'o>> for &'r ConfiguredAvailability<'i> {
	fn into(self) -> Availability<'o> {
		self.as_availability()
	}
}

impl<'i: 'o, 'o> Into<Availability<'o>> for ConfiguredAvailability<'i> {
	fn into(self) -> Availability<'o> {
		self.into_availability()
	}
}

impl<'i, T: Into<MqttTopic>> From<T> for ConfiguredAvailability<'i> {
	fn from(status_topic: T) -> Self {
		Self::with_status_topic(status_topic.into())
	}
}
