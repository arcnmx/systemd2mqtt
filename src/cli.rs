use {
	anyhow::Error,
	clap::Parser,
	hass_mqtt_types::{DeviceClass, EntityCategory},
	once_cell::sync::Lazy,
	paho_mqtt as mqtt,
	serde::{Deserialize, Serialize},
	std::{borrow::Cow, collections::HashMap, ops::Deref, str::FromStr},
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
	/// MQTT client ID
	#[arg(short, long)]
	pub client_id: Option<String>,
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
	pub fn hostname(&self) -> Cow<str> {
		self
			.hostname
			.as_ref()
			.map(|s| Cow::Borrowed(&s[..]))
			.unwrap_or_else(|| {
				hostname::get()
					.map(|h| Cow::Owned(h.to_string_lossy().into()))
					.unwrap_or(Cow::Borrowed("systemd"))
			})
	}

	pub fn units(&self) -> HashMap<&str, Unit> {
		self.units.iter().map(|u| (&u.unit[..], Unit::new(self, u))).collect()
	}

	pub fn use_mqtt(&self) -> bool {
		self.mqtt_url.is_some()
	}

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

	pub fn mqtt_create(&self) -> mqtt::CreateOptionsBuilder {
		mqtt::CreateOptionsBuilder::new()
			.server_uri(self.mqtt_url.as_ref().map(|s| &s[..]).unwrap_or_default())
			.client_id(self.client_id.as_ref().map(|s| &s[..]).unwrap_or("systemd"))
			.persist_qos0(false)
	}

	pub fn mqtt_connect(&self) -> mqtt::ConnectOptionsBuilder {
		let mut opts = mqtt::ConnectOptionsBuilder::new();
		if let Some(name) = self.mqtt_username() {
			opts.user_name(name);
		}
		if let Some(pw) = self.mqtt_password() {
			opts.password(pw);
		}
		opts.clean_session(true);
		opts
	}

	pub fn topic_root(&self) -> String {
		format!("systemd/{}", self.hostname())
	}

	pub fn mqtt_pub_topic(&self) -> String {
		format!("{}/status", self.topic_root())
	}

	pub fn mqtt_sub_topic(&self) -> String {
		format!("{}/control", self.topic_root())
	}
}

impl UnitConfig {
	pub fn short_name(&self) -> &str {
		self.unit.split('.').next().unwrap()
	}

	pub fn name(&self) -> &str {
		self.name.as_ref().map(String::as_str).unwrap_or(self.short_name())
	}

	pub fn unique_id(&self, cli: &Args) -> String {
		format!("{}_{}", cli.hass_device_id(), self.unit.replace(".", "_"))
	}

	pub fn default_object_id(&self, cli: &Args) -> String {
		format!("{}_{}", cli.hostname(), self.short_name())
	}

	pub fn object_id(&self, cli: &Args) -> Cow<str> {
		self
			.object_id
			.as_ref()
			.map(|id| Cow::Borrowed(&id[..]))
			.unwrap_or_else(|| self.default_object_id(cli).into())
	}

	pub fn mqtt_pub_topic(&self, cli: &Args) -> String {
		format!("{}/{}/status", cli.topic_root(), self.unit)
	}

	pub fn mqtt_sub_topic(&self, cli: &Args) -> String {
		format!("{}/{}/activate", cli.topic_root(), self.unit)
	}

	pub fn hass_platform(&self) -> &'static str {
		if self.read_only {
			"binary_sensor"
		} else {
			"switch"
		}
	}
}

#[derive(Debug)]
pub struct Unit<'a> {
	pub cli: &'a Args,
	pub unit: &'a UnitConfig,
	pub(crate) config: once_cell::unsync::OnceCell<Box<dyn crate::payload::Entity + 'a>>,
}

impl<'a> Unit<'a> {
	pub fn new(cli: &'a Args, unit: &'a UnitConfig) -> Self {
		Self {
			cli,
			unit,
			config: Default::default(),
		}
	}

	pub fn unit_name(&self) -> &'a String {
		&self.unit.unit
	}

	pub fn unique_id(&self) -> String {
		self.unit.unique_id(self.cli)
	}

	pub fn object_id(&self) -> Cow<'a, str> {
		self.unit.object_id(self.cli)
	}

	pub fn name(&self) -> &'a str {
		self.unit.name()
	}

	pub fn icon(&self) -> Option<&'a String> {
		self.unit.icon.as_ref()
	}

	pub fn mqtt_pub_topic(&self) -> String {
		self.unit.mqtt_pub_topic(self.cli)
	}

	pub fn mqtt_sub_topic(&self) -> String {
		self.unit.mqtt_sub_topic(self.cli)
	}
}

impl<'a> Deref for Unit<'a> {
	type Target = UnitConfig;

	fn deref(&self) -> &Self::Target {
		self.unit
	}
}

impl<'a> AsRef<(dyn crate::payload::Entity + 'a)> for Unit<'a> {
	fn as_ref(&self) -> &(dyn crate::payload::Entity + 'a) {
		self.hass_config()
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
				.ok_or_else(|| anyhow::format_err!("failed to parse unit specification: {}", s))?
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
