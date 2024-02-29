use crate::error::Error;
use ssh2::{ErrorCode, Session};
use std::fmt::Debug;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use tracing::{debug, info};

pub struct WifiLister {
    command: String,
    session: Session,
}

impl WifiLister {
    pub fn new<A: ToSocketAddrs + Debug>(
        addr: A,
        key: &str,
        pubkey: &str,
        interfaces: &[String],
    ) -> Result<Self, Error> {
        debug!(address = ?addr, "connecting to ssh");
        let tcp = TcpStream::connect(&addr).map_err(Error::SshConnect)?;
        let mut session = Session::new().map_err(Error::SshSession)?;
        session.set_tcp_stream(tcp);
        session.handshake().map_err(Error::SshSession)?;
        session
            .userauth_pubkey_memory("admin", Some(pubkey), key, None)
            .map_err(Error::SshAuth)?;

        let command = if interfaces.is_empty() {
            "wl assoclist".to_string()
        } else {
            let commands: Vec<String> = interfaces
                .iter()
                .map(|interface| format!("wl -a {} assoclist", interface))
                .collect();
            commands.join(" && ")
        };

        info!("ssh connected");

        Ok(WifiLister { session, command })
    }

    pub fn list_connected_devices(&self) -> Result<Vec<String>, ssh2::Error> {
        let mut channel = self.session.channel_session()?;
        debug!(command = self.command, "sending ssh command");
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
