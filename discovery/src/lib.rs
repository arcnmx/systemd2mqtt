pub use self::entities::*;
use {
	error_stack::{IntoReport as _, ResultExt as _},
	hass_mqtt_client::{EntityTopic, QosLevel, StateTopic},
	serde::Serialize,
	std::{future::Future, sync::Arc},
	systemd2mqtt_payload::{Error, Result, SerializeExt as _},
};

#[macro_use]
mod macros;
mod entities;

pub const QOS: QosLevel = QosLevel::AtLeastOnce;
pub type MqttTopic = Arc<str>;

pub struct EntityTopics {
	pub config_topic: EntityTopic,
	pub state_topic: StateTopic,
	pub command_topic: Option<MqttTopic>,
}

impl EntityTopics {
	pub fn state_topic(&self) -> MqttTopic {
		self.state_topic.topic()
	}

	pub fn expect_command_topic(&self) -> &MqttTopic {
		self.command_topic.as_ref().expect("expected command topic")
	}

	pub async fn publish_config(self: Arc<Self>, payload: Arc<[u8]>, retain: bool, qos: QosLevel) -> Result<()> {
		self
			.config_topic
			.publish(payload, retain, qos)
			.await
			.into_report()
			.change_context(Error::HassMqtt)
	}

	pub async fn publish_state_payload(self: Arc<Self>, payload: Arc<[u8]>, retain: bool, qos: QosLevel) -> Result<()> {
		self
			.state_topic
			.publish(payload, retain, qos)
			.await
			.into_report()
			.change_context(Error::HassMqtt)
	}

	pub fn publish_state(
		self: Arc<Self>,
		payload: &impl Serialize,
		retain: bool,
		qos: QosLevel,
	) -> impl Future<Output = Result<()>> {
		self.publish_state_payload(payload.encode_payload(), retain, qos)
	}
}
