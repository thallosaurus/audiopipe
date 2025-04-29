use std::time::Duration;

use threadpool::ThreadPool;
use uastreamer::{components::{control::TcpCommunication, streamer::Direction}, streamer_config::StreamerConfig};

/// Test implementation for the tcp communication
fn main() -> anyhow::Result<()> {

    let (streamer_config, device) = StreamerConfig::from_cli_args(Direction::Receiver)?;

    let tcp = TcpCommunication { direction: Direction::Receiver };
    tcp.serve("0.0.0.0:1234", streamer_config, device)?;

    Ok(())
}