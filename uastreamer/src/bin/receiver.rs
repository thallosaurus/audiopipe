use uastreamer::{
    App, Direction, components::control::TcpControlFlow, config::StreamerConfig,
};

fn main() {
    //let app = App
    let config = StreamerConfig::from_cli_args(Direction::Receiver).unwrap();

    let (mut app, _) = App::<f32>::new(config.clone());
    app.serve(config).unwrap();

    app.pool.join();
}
