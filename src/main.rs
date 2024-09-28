mod config;
mod error;
mod listener;

use crate::config::Config;
use crate::listener::WifiLister;
use clap::Parser;
use main_error::MainError;
use rumqttc::{AsyncClient, ClientError, MqttOptions, QoS};
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{select, spawn, time::sleep};
use tracing::{error, info};
use warp::Filter;

#[derive(Parser, Debug)]
struct Args {
    /// Path to config file
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let config = Config::load(&args.config)?;
    let mqtt_options = match &config.mqtt {
        Some(mqtt_config) => {
            let mut mqtt_options =
                MqttOptions::new("wifi-exporter", &mqtt_config.hostname, mqtt_config.port);
            mqtt_options.set_keep_alive(Duration::from_secs(5));
            mqtt_options.set_credentials(&mqtt_config.username, mqtt_config.password()?);
            info!("mqtt enabled");
            Some(mqtt_options)
        }
        _ => {
            info!("mqtt disabled");
            None
        }
    };

    if config.exporter.interfaces.is_empty() {
        info!("Listening on default interface");
    } else {
        info!(
            "Listening on interfaces: {}",
            config.exporter.interfaces.join(", ")
        );
    }

    let connected: Arc<Mutex<DeviceStates>> = Default::default();
    let wifi_listener = WifiLister::new(
        &config.ssh.address,
        &config.ssh.key()?,
        &config.ssh.pubkey()?,
        &config.exporter.interfaces,
    )?;

    let listen = listener(wifi_listener, connected.clone(), mqtt_options);

    let metrics = warp::path!("metrics").map(move || connected.lock().unwrap().format());

    ctrlc::set_handler(move || {
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let serve = warp::serve(metrics).run((config.exporter.address, config.exporter.port));

    select! {
        _ = serve => (),
        res = listen =>  {
            return res.map_err(MainError::from);
        },
    }

    Ok(())
}

#[derive(Default)]
struct DeviceStates {
    devices: HashMap<String, bool>,
}

#[derive(Debug)]
enum Update {
    New,
    Disconnected,
    Connected,
}

impl Display for Update {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Update::New => "discovered",
            Update::Disconnected => "disconnected",
            Update::Connected => "connected",
        };
        write!(f, "{}", s)
    }
}

impl DeviceStates {
    fn update(&mut self, new: Vec<String>) -> Vec<(String, Update)> {
        let mut updated = Vec::with_capacity(4);

        for (mac, connected) in self.devices.iter_mut() {
            if *connected && !new.contains(mac) {
                *connected = false;
                updated.push((mac.clone(), Update::Disconnected));
            }
        }

        for mac in new {
            match self.devices.get_mut(&mac) {
                Some(connected) if !*connected => {
                    updated.push((mac, Update::Connected));
                    *connected = true;
                }
                None => {
                    self.devices.insert(mac.clone(), true);
                    updated.push((mac, Update::New));
                }
                _ => {}
            }
        }

        updated
    }

    fn format(&self) -> String {
        let mut out = String::with_capacity(self.devices.len() * 40);
        for (mac, connected) in self.devices.iter() {
            writeln!(
                &mut out,
                "wifi_client{{mac=\"{}\"}} {}",
                mac, *connected as u8
            )
            .unwrap();
        }
        out
    }
}

async fn listener(
    wifi_listener: WifiLister,
    connected: Arc<Mutex<DeviceStates>>,
    mqtt_options: Option<MqttOptions>,
) -> Result<(), ssh2::Error> {
    let mut error_count = 0;
    let mut client = match mqtt_options {
        Some(mqtt_options) => {
            let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
            spawn(async move {
                loop {
                    eventloop.poll().await.unwrap();
                }
            });
            Some(client)
        }
        None => None,
    };

    loop {
        match wifi_listener.list_connected_devices() {
            Ok(devices) => {
                error_count = 1;
                let updates = connected.lock().unwrap().update(devices);
                for (mac, update) in updates {
                    info!(mac, %update, "change detected");
                    if let Some(client) = client.as_mut() {
                        if let Err(e) = send_update(client, mac, update).await {
                            error!(e = ?e, "Error while sending mqtt update: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                error_count += 1;
                error!(e = ?e, "Error while listing devices {e}");
                if error_count >= 5 {
                    return Err(e);
                }
            }
        }
        sleep(Duration::from_secs(5)).await;
    }
}

async fn send_update(
    client: &mut AsyncClient,
    mac: String,
    update: Update,
) -> Result<(), ClientError> {
    let mac = mac.replace(':', "_");
    match update {
        Update::New => {
            client
                .publish(
                    format!("homeassistant/device_tracker/wifi-{}/config", mac),
                    QoS::AtLeastOnce,
                    true,
                    format!(
                        r#"{{
                            "state_topic": "wifi-exporter/{mac}/state",
                            "device": {{
                                "name": "Wifi device {mac}",
                                "manufacturer": "Icewind",
                                "model": "Wifi Tracker",
                                "identifiers": "{mac}"
                            }},
                            "name": "Wifi device {mac}",
                            "payload_home": "connected",
                            "payload_not_home": "disconnected",
                            "unique_id": "wifi-{mac}-connected",
                            "icon": "mdi:wifi",
                            "source_type": "router"
                         }}"#,
                        mac = mac
                    ),
                )
                .await?;
            let client = client.clone();
            spawn(async move {
                client
                    .publish(
                        format!("wifi-exporter/{}/state", mac),
                        QoS::AtLeastOnce,
                        true,
                        r#"connected"#,
                    )
                    .await
                    .ok();
            });
        }
        Update::Connected => {
            client
                .publish(
                    format!("wifi-exporter/{}/state", mac),
                    QoS::AtLeastOnce,
                    true,
                    r#"connected"#,
                )
                .await?;
        }
        Update::Disconnected => {
            client
                .publish(
                    format!("wifi-exporter/{}/state", mac),
                    QoS::AtLeastOnce,
                    true,
                    r#"disconnected"#,
                )
                .await?;
        }
    }
    Ok(())
}
