use std::sync::mpsc::channel;

use uastreamer::{
    config::StreamerConfig, pooled::{control::TcpControlFlow, udp::UdpSenderCommands, App}, ualog::SimpleLogger, Direction
};

fn main() {
    log::set_logger(&SimpleLogger {}).unwrap();
    let config = StreamerConfig::from_cli_args(Direction::Sender).unwrap();
    log::set_max_level(config.program_args.verbose.log_level_filter());

    let (mut app, _) = App::<f32>::new(config.clone());

    let (cmd_tx, cmd_rx) = channel::<UdpSenderCommands>();

    app.tcp_serve(config, Some(cmd_rx), None).unwrap();

    app.pool.join();

}
