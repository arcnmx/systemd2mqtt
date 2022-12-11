use {
	crate::{
		core::{EntityTopics, MqttTopic},
		entities::{ConfiguredDevice, ConfiguredEntity, EntityContext, EntityIds, StaticEntity as _},
		payload::{SerializeExt as _, ServiceCommand, PKG_NAME},
	},
	hass_mqtt_types::{Availability, Button, EntityCategory},
	std::{borrow::Cow, slice},
};

#[derive(Debug, Clone)]
pub(crate) struct DiagButton<'i> {
	pub device: ConfiguredDevice<'i>,
	pub command_topic: MqttTopic,
	pub status_topic: MqttTopic,
	pub availability: Availability<'i>,
	pub unique_id: String,
	pub object_id: String,
	pub name: Cow<'i, str>,
}

impl<'i> DiagButton<'i> {
	pub fn as_button(&self) -> Button {
		let unique_id = self.unique_id.as_str();
		Button::new(&self.command_topic[..])
			.unique_id(unique_id)
			.object_id(&self.object_id)
			.name(&self.name[..])
			.device(&self.device)
			.availability(slice::from_ref(&self.availability))
			.json_attributes_topic(&self.status_topic[..])
			.payload_press(ServiceCommand::Set { active: false }.encode_str())
			.entity_category(EntityCategory::Diagnostic)
	}
}

impl<'i> ConfiguredEntity<'i> for DiagButton<'i> {
	type Args<'a> = () where Self: 'a;

	fn new_unique_id<'a>(context: &EntityContext<'i>, _args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		format!("{}_diag_reset", ConfiguredDevice::device_id(context.hostname())).into()
	}

	fn new_short_id<'a>(_context: &EntityContext<'i>, _args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		"diag_reset".into()
	}

	fn new_name<'a>(_context: &EntityContext<'i>, _args: &Self::Args<'a>) -> Cow<'i, str>
	where
		Self: 'a,
	{
		format!("{} reset", PKG_NAME).into()
	}

	fn new_domain<'a>(_context: &EntityContext<'i>, _args: &Self::Args<'a>) -> &'static str
	where
		Self: 'a,
	{
		Self::DOMAIN
	}

	fn new<'a>(context: &EntityContext<'i>, topics: &EntityTopics, args: Self::Args<'a>) -> Self
	where
		Self: 'a,
	{
		let (device, availability) = context.to_parts();
		let EntityIds {
			unique_id,
			object_id,
			name,
		} = Self::new_ids(context, &args);
		Self {
			unique_id: unique_id.into(),
			object_id: object_id.into(),
			name: name.into(),
			status_topic: availability.status_topic.clone(),
			availability: availability.into_availability(),
			command_topic: topics.expect_command_topic().clone(),
			device,
		}
	}
}

impl<'r: 'o, 'i: 'o, 'o> Into<Button<'o>> for &'r DiagButton<'i> {
	fn into(self) -> Button<'o> {
		self.as_button()
	}
}
