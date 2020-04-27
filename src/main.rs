use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use ssh2::Session;
use main_error::MainError;
use std::collections::HashMap;
use std::path::Path;
use warp::Filter;
use std::ffi::OsStr;
use std::sync::Arc;
use std::str::FromStr;

struct WifiLister {
    command: String,
    session: Session,
}

impl WifiLister {
    pub fn new<A: ToSocketAddrs, Priv: AsRef<OsStr>, Pub: AsRef<OsStr>>(addr: A, keyfile: Priv, pubkey: Pub, interfaces: &[&str]) -> Result<Self, MainError> {
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
            let commands: Vec<String> = interfaces.iter().map(|interface| format!("wl -a {} assoclist", interface)).collect();
            commands.join(" && ")
        };

        Ok(WifiLister {
            session,
            command,
        })
    }

    pub fn list_connected_devices(&self) -> Result<Vec<String>, MainError> {
        let mut channel = self.session.channel_session()?;
        channel.exec(&self.command)?;
        let mut s = String::new();
        channel.read_to_string(&mut s)?;
        channel.wait_close()?;

        Ok(s.lines().map(|s| s.trim_start_matches("assoclist ").to_string()).collect())
    }
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let mut env: HashMap<String, String> = dotenv::vars().collect();
    let addr = env.remove("ADDR").ok_or("No ADDR set")?;
    let keyfile = env.remove("KEYFILE").ok_or("No KEYFILE set")?;
    let pubfile = env.remove("PUBFILE").ok_or("No PUBFILE set")?;
    let port = env.get("PORT").and_then(|s| u16::from_str(s).ok()).unwrap_or(80);
    let interfaces: Vec<&str> = env.get("INTERFACES").map(|interfaces| interfaces.split(' ').collect()).unwrap_or_default();

    if interfaces.is_empty() {
        println!("Listening on default interface");
    } else {
        println!("Listening on interfaces: {}", interfaces.join(", "));
    }


    let wifi_listener = Arc::new(WifiLister::new(addr, &keyfile, &pubfile, &interfaces)?);

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let metrics = warp::path!("metrics")
        .map(move || {
            let mac_addresses = wifi_listener.list_connected_devices().unwrap_or_default();
            let lines: Vec<_> = mac_addresses.into_iter().map(|mac| format!("wifi_client{{mac=\"{}\"}} 1", mac)).collect();
            lines.join("\n")
        });

    ctrlc::set_handler(move || {
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");

    warp::serve(metrics)
        .run(([0, 0, 0, 0], port))
        .await;

    Ok(())
}