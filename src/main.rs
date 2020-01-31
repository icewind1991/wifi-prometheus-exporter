use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use ssh2::Session;
use main_error::MainError;
use std::collections::HashMap;
use std::path::Path;
use warp::Filter;
use std::ffi::OsStr;
use std::sync::Arc;

struct WifiLister {
    session: Session
}

impl WifiLister {
    pub fn new<A: ToSocketAddrs, S: AsRef<OsStr> + ?Sized>(addr: A, keyfile: &S) -> Result<Self, MainError> {
        let tcp = TcpStream::connect(addr)?;
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;
        let key_file = Path::new(keyfile);
        session.userauth_pubkey_file("admin", None, &key_file, None)?;

        Ok(WifiLister {
            session
        })
    }

    pub fn list_connected_devices(&self) -> Result<Vec<String>, MainError> {
        let mut channel = self.session.channel_session()?;
        channel.exec("wl assoclist")?;
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

    let wifi_listener = Arc::new(WifiLister::new(addr, &keyfile)?);

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let metrics = warp::path!("metrics")
        .map(move || {
            let mac_addresses = wifi_listener.list_connected_devices().unwrap_or_default();
            let lines: Vec<_> = mac_addresses.into_iter().map(|mac| format!("wifi_client{{mac=\"{}\"}} 1", mac)).collect();
            lines.join("\n")
        });

    warp::serve(metrics)
        .run(([127, 0, 0, 1], 3030u16))
        .await;

    Ok(())
}