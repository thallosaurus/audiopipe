use std::sync::mpsc::channel;

use uastreamer::{
    components::{control::TcpControlFlow, udp::UdpReceiverCommands}, config::StreamerConfig, App, Direction
};

fn main() {
    //let app = App
    let config = StreamerConfig::from_cli_args(Direction::Receiver).unwrap();

    let (mut app, _) = App::<f32>::new(config.clone());

    let (cmd_tx, cmd_rx) = channel::<UdpReceiverCommands>();

    app.tcp_serve(config, None, Some(cmd_rx)).unwrap();

    app.pool.join();
}
