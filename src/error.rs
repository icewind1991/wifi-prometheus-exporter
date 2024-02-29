use secretfile::SecretError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read config: {0}")]
    ReadConfig(std::io::Error),
    #[error("failed to parse config: {0}")]
    ParseConfig(toml::de::Error),
    #[error("failed to secret: {0:#}")]
    Secret(#[from] SecretError),
    #[error("failed to connect to ssh server: {0}")]
    SshConnect(std::io::Error),
    #[error("failed to start ssh session: {0}")]
    SshSession(ssh2::Error),
    #[error("failed to authenticate ssh session: {0}")]
    SshAuth(ssh2::Error),
}
