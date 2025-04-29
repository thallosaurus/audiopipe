use std::{net::SocketAddr, str::FromStr, time::Duration};

use threadpool::ThreadPool;
use uastreamer::{components::{control::TcpCommunication, streamer::Direction}, streamer_config::StreamerConfig};

/// Test implementation for the tcp communication
fn main() -> anyhow::Result<()> {

    let (streamer_config, device) = StreamerConfig::from_cli_args(Direction::Receiver)?;

    let tcp = TcpCommunication { direction: Direction::Receiver };
    let s = SocketAddr::from_str("0.0.0.0:1234")?;
    tcp.serve(s, streamer_config, device)?;

    Ok(())
}