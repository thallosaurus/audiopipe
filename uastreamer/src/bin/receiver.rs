use std::sync::mpsc::channel;

use uastreamer::{
    components::{control::TcpControlFlow, udp::UdpReceiverCommands}, config::StreamerConfig, ualog::SimpleLogger, App, Direction
};

fn main() {
    log::set_logger(&SimpleLogger {}).unwrap();
    let config = StreamerConfig::from_cli_args(Direction::Receiver).unwrap();
    log::set_max_level(config.program_args.verbose.log_level_filter());

    let (mut app, _) = App::<f32>::new(config.clone());

    let (cmd_tx, cmd_rx) = channel::<UdpReceiverCommands>();

    /*ctrlc::set_handler(move || {
        cmd_tx.send(UdpReceiverCommands::Stop).unwrap();
    }).unwrap();*/

    app.tcp_serve(config, None, Some(cmd_rx)).unwrap();

    app.pool.join();
}
