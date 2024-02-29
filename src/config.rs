use crate::error::Error;
use secretfile::load;
use serde::Deserialize;
use std::fs::read_to_string;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub mqtt: Option<MqttConfig>,
    pub ssh: SshConfig,
    pub exporter: ExporterConfig,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let content = read_to_string(path).map_err(Error::ReadConfig)?;
        toml::from_str(&content).map_err(Error::ParseConfig)
    }
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    pub hostname: String,
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    pub username: String,
    password_file: String,
}

fn default_mqtt_port() -> u16 {
    1883
}

impl MqttConfig {
    pub fn password(&self) -> Result<String, Error> {
        Ok(load(&self.password_file)?)
    }
}

#[derive(Debug, Deserialize)]
pub struct SshConfig {
    pub address: String,
    pubkey_file: String,
    key_file: String,
}

impl SshConfig {
    pub fn key(&self) -> Result<String, Error> {
        Ok(load(&self.key_file)?)
    }

    pub fn pubkey(&self) -> Result<String, Error> {
        Ok(load(&self.pubkey_file)?)
    }
}

#[derive(Debug, Deserialize)]
pub struct ExporterConfig {
    #[serde(default = "default_address")]
    pub address: IpAddr,
    pub port: u16,
    pub interfaces: Vec<String>,
}

fn default_address() -> IpAddr {
    Ipv4Addr::new(127, 0, 0, 1).into()
}
