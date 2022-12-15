pub use self::payload::*;
use {
	error_stack::{IntoReport as _, ResultExt as _},
	hass_mqtt_types::{DeviceClass, EntityCategory},
	once_cell::sync::Lazy,
	serde::{Deserialize, Serialize},
	std::{result::Result as StdResult, str::FromStr, sync::Arc},
	url::Url,
};

mod payload;

pub type Result<T> = error_stack::Result<T, Error>;

pub const ON: &'static str = "ON";
pub const OFF: &'static str = "OFF";
pub const PKG_NAME: &'static str = env!("CARGO_PKG_NAME");

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("built without support for an MQTT backend")]
	NoMqttBackend,
	#[error("built without a supported MQTT backend")]
	NoTls,
	#[error("MQTT url is missing the host to connect to")]
	UrlMissingHost,
	#[error("failed to parse unit specification")]
	InvalidUnitUrl(#[from] url::ParseError),
	#[error("failed to parse unit arguments")]
	InvalidUnitArgs(#[from] serde_urlencoded::de::Error),
	#[error("failed to parse unit specification {spec:?}")]
	InvalidUnitSpec { spec: String },
	#[error("Systemd connection lost")]
	ConnectionLostSystemd,
	#[error("MQTT connection lost")]
	ConnectionLostMqtt,
	#[error("MQTT connection error")]
	ConnectionError,
	#[error("HassMqttClient error")]
	HassMqtt,
	#[error("Systemd error")]
	Dbus,
	#[error("Systemd error")]
	Systemd,
	#[error("home-assistant entity configuration error")]
	Entity,
	#[error("internal serialization error, this is a bug!")]
	Serialization,
	#[error("internal consistency error, this is a bug!")]
	InternalConsistency { unit_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UnitConfig {
	#[serde(default)]
	pub unit: String,
	#[serde(alias = "readonly", default)]
	pub read_only: bool,
	#[serde(alias = "invert", default)]
	pub invert_state: bool,
	#[serde(alias = "enabled", default = "default_true")]
	pub enabled_by_default: bool,
	#[serde(default = "default_entity_category")]
	pub entity_category: EntityCategory,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub icon: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
	#[serde(alias = "entity_id", default, skip_serializing_if = "Option::is_none")]
	pub object_id: Option<String>,
	#[serde(default, skip_serializing_if = "DeviceClass::is_none")]
	pub device_class: DeviceClass,
}

pub trait Config {
	fn units<'c>(&'c self) -> Box<dyn Iterator<Item = &'c UnitConfig> + 'c>;

	fn hostname(&self) -> Option<&str>;
	fn mqtt_url(&self) -> Option<&Url>;
	fn mqtt_username(&self) -> Option<&str>;
	fn mqtt_password(&self) -> Option<&str>;
	fn discovery_prefix(&self) -> &str;
	fn clean_up(&self) -> bool;
}

impl UnitConfig {
	pub fn short_name(&self) -> &str {
		self.unit.split('.').next().unwrap()
	}

	pub fn name(&self) -> &str {
		self.name.as_ref().map(String::as_str).unwrap_or(self.short_name())
	}
}

impl FromStr for UnitConfig {
	type Err = Error;

	fn from_str(s: &str) -> StdResult<Self, Self::Err> {
		static BASE_URL: Lazy<Url> = Lazy::new(|| Url::parse("systemd:/unit/").expect("static url can't fail"));

		let url = Url::options().base_url(Some(&BASE_URL)).parse(s)?;

		let config = match url.query() {
			Some(query) => serde_urlencoded::de::from_str(&query),
			None => Ok(Self::default()),
		}?;

		Ok(Self {
			unit: url
				.path()
				.strip_prefix("/unit/")
				.ok_or_else(|| Error::InvalidUnitSpec { spec: s.into() })?
				.into(),
			..config
		})
	}
}

impl Default for UnitConfig {
	fn default() -> Self {
		Self {
			unit: Default::default(),
			icon: Default::default(),
			name: Default::default(),
			object_id: Default::default(),
			device_class: Default::default(),
			read_only: Default::default(),
			invert_state: Default::default(),
			enabled_by_default: true,
			entity_category: default_entity_category(),
		}
	}
}

fn default_true() -> bool {
	true
}

fn default_entity_category() -> EntityCategory {
	EntityCategory::Config
}

pub trait SerializeExt {
	fn try_encode(&self) -> crate::Result<Vec<u8>>;

	fn try_encode_payload(&self) -> crate::Result<Arc<[u8]>> {
		self.try_encode().map(Into::into)
	}

	fn encode_payload(&self) -> Arc<[u8]> {
		self.encode().into()
	}

	fn encode_str(&self) -> String {
		unsafe { String::from_utf8_unchecked(self.encode()) }
	}

	fn encode(&self) -> Vec<u8> {
		self.try_encode().expect("payloads should never fail to serialize")
	}
}

impl<T: Serialize> SerializeExt for T {
	fn try_encode(&self) -> crate::Result<Vec<u8>> {
		serde_json::to_vec(self)
			.into_report()
			.change_context(crate::Error::Serialization)
	}
}
