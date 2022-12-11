use {
	error_stack::{IntoReport as _, ResultExt as _},
	serde::{Deserialize, Serialize},
	std::{borrow::Cow, sync::Arc},
};

pub(crate) const ON: &'static str = "ON";
pub(crate) const OFF: &'static str = "OFF";
pub(crate) const PKG_NAME: &'static str = env!("CARGO_PKG_NAME");

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

#[derive(Serialize, Debug)]
pub struct ServiceStatus<'a> {
	pub is_active: bool,
	#[serde(borrow)]
	pub units: Vec<Cow<'a, str>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServiceCommand {
	Set { active: bool },
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

#[derive(Serialize, Deserialize, Debug)]
pub enum UnitCommand {
	Start,
	Stop,
	Restart,
}
