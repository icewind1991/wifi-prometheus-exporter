use main_error::MainError;
use rumqttc::{AsyncClient, ClientError, MqttOptions, QoS};
use ssh2::{ErrorCode, Session};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Write;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{spawn, time::sleep};
use warp::Filter;

struct WifiLister {
    command: String,
    session: Session,
}

impl WifiLister {
    pub fn new<A: ToSocketAddrs, Priv: AsRef<OsStr>, Pub: AsRef<OsStr>>(
        addr: A,
        keyfile: Priv,
        pubkey: Pub,
        interfaces: &[&str],
    ) -> Result<Self, MainError> {
        let tcp = TcpStream::connect(addr)?;
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;
        let keyfile = Path::new(keyfile.as_ref());
        let pubkey = Path::new(pubkey.as_ref());
        session.userauth_pubkey_file("admin", Some(pubkey), keyfile, None)?;

        let command = if interfaces.is_empty() {
            "wl assoclist".to_string()
        } else {
            let commands: Vec<String> = interfaces
                .iter()
                .map(|interface| format!("wl -a {} assoclist", interface))
                .collect();
            commands.join(" && ")
        };

        Ok(WifiLister { session, command })
    }

    pub fn list_connected_devices(&self) -> Result<Vec<String>, ssh2::Error> {
        let mut channel = self.session.channel_session()?;
        channel.exec(&self.command)?;
        let mut s = String::new();
        channel.read_to_string(&mut s).map_err(|e| {
            ssh2::Error::new(
                ErrorCode::Session(e.raw_os_error().unwrap_or_default()),
                "error reading from ssh stream",
            )
        })?;
        channel.wait_close()?;

        Ok(s.lines()
            .map(|s| s.trim_start_matches("assoclist ").to_string())
            .collect())
    }
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let mut env: HashMap<String, String> = dotenv::vars().collect();
    let addr = env.remove("ADDR").ok_or("No ADDR set")?;
    let keyfile = env.remove("KEYFILE").ok_or("No KEYFILE set")?;
    let pubfile = env.remove("PUBFILE").ok_or("No PUBFILE set")?;
    let port = env
        .get("PORT")
        .and_then(|s| u16::from_str(s).ok())
        .unwrap_or(80);
    let mqtt_host = env.remove("MQTT_HOSTNAME");
    let mqtt_user = env.remove("MQTT_USERNAME");
    let mqtt_pass = env.remove("MQTT_PASSWORD");
    let interfaces: Vec<&str> = env
        .get("INTERFACES")
        .map(|interfaces| interfaces.split(' ').collect())
        .unwrap_or_default();

    let mqtt_options = match (mqtt_host, mqtt_user, mqtt_pass) {
        (Some(host), Some(user), Some(pass)) => {
            let mut mqtt_options = MqttOptions::new("wifi-exporter", host, 1883);
            mqtt_options.set_keep_alive(Duration::from_secs(5));
            mqtt_options.set_credentials(user, pass);
            Some(mqtt_options)
        }
        _ => None,
    };

    if interfaces.is_empty() {
        println!("Listening on default interface");
    } else {
        println!("Listening on interfaces: {}", interfaces.join(", "));
    }

    let connected: Arc<Mutex<DeviceStates>> = Default::default();
    let wifi_listener = WifiLister::new(addr, &keyfile, &pubfile, &interfaces)?;

    spawn(listener(wifi_listener, connected.clone(), mqtt_options));

    let metrics = warp::path!("metrics").map(move || connected.lock().unwrap().format());

    ctrlc::set_handler(move || {
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    warp::serve(metrics).run(([0, 0, 0, 0], port)).await;

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
) {
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
                let updates = connected.lock().unwrap().update(devices);
                if let Some(client) = client.as_mut() {
                    for (mac, update) in updates {
                        if let Err(e) = send_update(client, mac, update).await {
                            eprintln!("Error while sending mqtt update: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => eprintln!("Error while listing devices {:#?}", e),
        }
        sleep(Duration::from_secs(5)).await;
    }
}

async fn send_update(
    client: &mut AsyncClient,
    mac: String,
    update: Update,
) -> Result<(), ClientError> {
    let mac = mac.replace(":", "_");
    match update {
        Update::New => {
            client
                .publish(
                    format!("homeassistant/device_tracker/wifi-{}/config", mac),
                    QoS::AtLeastOnce,
                    false,
                    format!(
                        r#"{{
                            "state_topic": "wifi-exporter/{mac}/state",
                            "device": {{
                                "name": "Wifi device {mac}",
                                "manufacturer": "Icewind",
                                "model": "Wifi Tracker",
                                "identifiers": "{mac}"
                            }}
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
            client
                .publish(
                    format!("wifi-exporter/{}/state", mac),
                    QoS::AtLeastOnce,
                    false,
                    r#"connected"#,
                )
                .await?;
        }
        Update::Connected => {
            client
                .publish(
                    format!("wifi-exporter/{}/state", mac),
                    QoS::AtLeastOnce,
                    false,
                    r#"connected"#,
                )
                .await?;
        }
        Update::Disconnected => {
            client
                .publish(
                    format!("wifi-exporter/{}/state", mac),
                    QoS::AtLeastOnce,
                    false,
                    r#"disconnected"#,
                )
                .await?;
        }
    }
    Ok(())
}
