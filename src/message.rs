extern crate serde;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "ping")]
    Ping {},
    #[serde(rename = "pong")]
    Pong {},
    #[serde(rename = "clientRegister")]
    ClientRegister { data: RegistrationData },
    #[serde(rename = "appstreamRegister")]
    AppStreamRegister { data: RegistrationData },
    #[serde(rename = "message")]
    Default { data: String },
}

pub type RelayKey = String;

#[derive(Serialize, Deserialize)]
pub struct RegistrationData {
    pub key: RelayKey,
}