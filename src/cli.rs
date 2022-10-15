use std::collections::HashSet;
use std::borrow::Cow;
use clap::Parser;
use paho_mqtt as mqtt;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
	/// such as `tcp://localhost:1883`
	#[arg(short = 'M', long)]
	pub mqtt_url: Option<String>,
	/// units to pay attention to
	#[arg(short, long = "unit")]
	pub units: Vec<String>,
	/// identify this host
	#[arg(short = 'H', long)]
	pub hostname: Option<String>,
	/// mqtt client ID
	#[arg(short, long)]
	pub client_id: Option<String>,
	/// home-assistant discovery prefix
	#[arg(short, long, default_value("homeassistant"))]
	pub discovery_prefix: String,
	#[arg(short = 'U', long, env("MQTT_USERNAME"))]
	pub mqtt_username: Option<String>,
	#[arg(short = 'P', long, env("MQTT_PASSWORD"))]
	pub mqtt_password: Option<String>,
}

impl Args {
	pub fn topic_root(&self) -> String {
		format!("systemd/{}", self.hostname())
	}

	pub fn hostname(&self) -> Cow<str> {
		self.hostname.as_ref().map(|s| Cow::Borrowed(&s[..]))
			.unwrap_or_else(|| hostname::get().map(|h| Cow::Owned(h.to_string_lossy().into())).unwrap_or(Cow::Borrowed("systemd")))
	}

	pub fn interesting_units(&self) -> HashSet<&str> {
		self.units.iter()
			.map(|s| &s[..])
			.collect()
	}

	pub fn use_mqtt(&self) -> bool {
		self.mqtt_url.is_some()
	}

	pub fn mqtt_create(&self) -> mqtt::CreateOptionsBuilder {
		mqtt::CreateOptionsBuilder::new()
			.server_uri(self.mqtt_url.as_ref().map(|s| &s[..]).unwrap_or_default())
			.client_id(self.client_id.as_ref().map(|s| &s[..]).unwrap_or("systemd"))
			.persist_qos0(false)
	}

	pub fn mqtt_connect(&self) -> mqtt::ConnectOptionsBuilder {
		let mut opts = mqtt::ConnectOptionsBuilder::new();
		if let Some(name) = &self.mqtt_username {
			opts.user_name(name);
		}
		if let Some(pw) = &self.mqtt_password {
			opts.password(pw);
		}
		opts.clean_session(true);
		opts
	}

	pub fn mqtt_pub_topic_unit(&self, unit: &str) -> String {
		format!("{}/{}/status", self.topic_root(), unit)
	}

	pub fn mqtt_sub_topic_unit(&self, unit: &str) -> String {
		format!("{}/{}/activate", self.topic_root(), unit)
	}

	pub fn mqtt_pub_topic(&self) -> String {
		format!("{}/status", self.topic_root())
	}

	pub fn unit_short_name(unit: &str) -> &str {
		unit.split('.').next().unwrap()
	}
}
