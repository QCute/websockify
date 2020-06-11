//! Argument Options

use structopt::StructOpt;
use std::str::FromStr;

#[derive(StructOpt, Clone, Debug)]
#[structopt()]
pub struct Opt {
    /// Listen Address
    #[structopt(short, long, default_value = "0.0.0.0")]
    pub listen: String,

    /// Destination Address or Unix Domain Socket Path
    #[structopt(short, long, default_value = "127.0.0.1")]
    pub dst: String,

    /// TCP Port
    #[structopt(short, long, default_value = "0")]
    pub tcp_port: u16,

    /// SSL Port
    #[structopt(short, long, default_value = "0")]
    pub ssl_port: u16,

    /// SSL Cert File Path
    #[structopt(short = "c", long, default_value = "", parse(from_os_str))]
    pub ssl_cert: std::path::PathBuf,

    /// SSL Key File Path
    #[structopt(short = "k", long, default_value = "", parse(from_os_str))]
    pub ssl_key: std::path::PathBuf,

    /// Proxy Type
    #[structopt(short = "m", long, default_value = "prefix", possible_values = &PortMapType::variants(), case_insensitive = true)]
    pub port_map_type: PortMapType,

    /// Proxy Port Restrict Min [min, max)
    #[structopt(short = "i", long, default_value = "0")]
    pub port_restrict_min: u16,

    /// Proxy Port Restrict Max [min, max)
    #[structopt(short = "x", long, default_value = "0")]
    pub port_restrict_max: u16,

    /// Client Request Timeout
    #[structopt(short = "o", long, default_value = "10")]
    pub timeout: u64,

}

arg_enum! {
    #[derive(Copy, Clone, Debug)]
    pub enum PortMapType {
        Domain,
        Prefix,
    }
}

#[derive(Clone, Debug)]
pub enum StreamType {
    UnixDomainSocket(String),
    TcpSocket(String),
}

impl Opt {
    pub fn get_stream_type(&self, file_or_port: &str) -> anyhow::Result<StreamType> {
        match std::fs::metadata(&self.dst) {
            Ok(metadata) if metadata.is_dir() =>
                anyhow::Result::Ok(StreamType::UnixDomainSocket(format!("{}/{}", self.dst, file_or_port).to_string())),
            _ => {
                let dst = format!("{}:{}", self.dst, file_or_port);
                match async_std::net::SocketAddr::from_str(dst.as_str()) {
                    Ok(_) if self.port_restrict_min == 0 && self.port_restrict_max == 0 => anyhow::Result::Ok(StreamType::TcpSocket(dst)),
                    Ok(address) if self.port_restrict_min == 0 && address.port() <= self.port_restrict_max => anyhow::Result::Ok(StreamType::TcpSocket(dst)),
                    Ok(address) if self.port_restrict_max == 0 && address.port() >= self.port_restrict_min => anyhow::Result::Ok(StreamType::TcpSocket(dst)),
                    Ok(address) if self.port_restrict_min <= address.port() && address.port() < self.port_restrict_max => anyhow::Result::Ok(StreamType::TcpSocket(dst)),
                    Ok(address) => anyhow::Result::Err(anyhow!("Port Restrict: {}, min: {} max: {}", address.port(), self.port_restrict_min, self.port_restrict_max)),
                    Err(_) => anyhow::Result::Err(anyhow!("Invalid Unix Domain Socket or Network Address: {}", file_or_port))
                }
            }
        }
    }
}

pub fn load() -> Opt {
    Opt::from_args()
}

