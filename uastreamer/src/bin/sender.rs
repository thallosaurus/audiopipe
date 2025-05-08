use std::sync::mpsc::channel;

use uastreamer::{
    components::control::{TcpControlFlow, UdpSenderCommands}, config::StreamerConfig, App, Direction
};

fn main() {
    //let app = App
    let config = StreamerConfig::from_cli_args(Direction::Sender).unwrap();
    let (mut app, _) = App::<f32>::new(config.clone());

    let (cmd_tx, cmd_rx) = channel::<UdpSenderCommands>();

    app.tcp_serve(config, Some(cmd_rx), None).unwrap();

    app.pool.join();
}
