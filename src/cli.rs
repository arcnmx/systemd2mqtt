use {clap::Parser, systemd2mqtt::UnitConfig, url::Url};

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

impl systemd2mqtt::Config for Args {
	fn units<'c>(&'c self) -> Box<dyn Iterator<Item = &'c UnitConfig> + 'c> {
		Box::new(self.units.iter()) as Box<_>
	}

	fn hostname(&self) -> Option<&str> {
		self.hostname.as_ref().map(|s| s.as_str())
	}

	fn mqtt_url(&self) -> Option<&Url> {
		self.mqtt_url.as_ref()
	}

	fn mqtt_username(&self) -> Option<&str> {
		self
			.mqtt_username
			.as_ref()
			.map(String::as_str)
			.or_else(|| self.mqtt_url.as_ref().and_then(|u| opt_str(u.username())))
	}

	fn mqtt_password(&self) -> Option<&str> {
		self
			.mqtt_password
			.as_ref()
			.map(String::as_str)
			.or_else(|| self.mqtt_url.as_ref().and_then(|u| u.password()))
	}

	fn discovery_prefix(&self) -> &str {
		self.discovery_prefix.as_ref()
	}

	fn clean_up(&self) -> bool {
		self.clean_up
	}
}

fn opt_str(s: &str) -> Option<&str> {
	match s.is_empty() {
		false => Some(s),
		true => None,
	}
}
