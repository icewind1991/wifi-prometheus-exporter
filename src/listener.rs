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
        user: &str,
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
            .userauth_pubkey_memory(user, Some(pubkey), key, None)
            .map_err(Error::SshAuth)?;

        let commands: Vec<String> = interfaces
            .iter()
            .map(|interface| format!("iw dev {} station dump", interface))
            .collect();
        let command = commands.join(" && ");

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
            .filter(|s| s.starts_with("Station"))
            .filter_map(|s| s.split(' ').nth(1))
            .map(str::to_ascii_uppercase)
            .collect())
    }
}
