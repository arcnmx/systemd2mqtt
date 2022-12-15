use {
	serde::{Deserialize, Serialize},
	std::borrow::Cow,
};

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
