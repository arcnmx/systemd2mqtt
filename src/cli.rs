use {
	crate::Error,
	clap::Parser,
	hass_mqtt_types::{DeviceClass, EntityCategory},
	once_cell::sync::Lazy,
	serde::{Deserialize, Serialize},
	std::str::FromStr,
	url::Url,
};

#[deny(missing_docs)]
/// Expose systemd services over MQTT
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
	/// such as `tcp://localhost:1883`
	#[arg(short = 'M', long)]
	pub mqtt_url: Option<Url>,
	/// units to pay attention to
	///
	/// A unit can be specified with additional settings,
	/// for example: `display-manager.service?read-only=true&icon=mdi:projector-screen`
	#[arg(short, long = "unit")]
	pub units: Vec<UnitConfig>,
	/// identify this host
	#[arg(short = 'H', long)]
	pub hostname: Option<String>,
	/// remove discoverable entities from home-assistant on exit
	#[arg(long)]
	pub clean_up: bool,
	/// home-assistant discovery prefix
	#[arg(short, long, default_value("homeassistant"))]
	pub discovery_prefix: String,
	/// authentication username
	#[arg(short = 'U', long, env("MQTT_USERNAME"))]
	pub mqtt_username: Option<String>,
	/// authentication password
	#[arg(short = 'P', long, env("MQTT_PASSWORD"))]
	pub mqtt_password: Option<String>,
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

impl Args {
	pub fn mqtt_username(&self) -> Option<&str> {
		self
			.mqtt_username
			.as_ref()
			.map(String::as_str)
			.or_else(|| self.mqtt_url.as_ref().and_then(|u| opt_str(u.username())))
	}

	pub fn mqtt_password(&self) -> Option<&str> {
		self
			.mqtt_password
			.as_ref()
			.map(String::as_str)
			.or_else(|| self.mqtt_url.as_ref().and_then(|u| u.password()))
	}
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

	fn from_str(s: &str) -> Result<Self, Self::Err> {
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

fn opt_str(s: &str) -> Option<&str> {
	match s.is_empty() {
		false => Some(s),
		true => None,
	}
}
