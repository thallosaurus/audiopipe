use std::sync::{
    mpsc::{self, Receiver, Sender}, Arc, Mutex
};

use bytemuck::Pod;
use ringbuf::{traits::Split, HeapCons, HeapProd};
use uastreamer::{components::{
    control::TcpControlFlow, cpal::{CpalAudioFlow, CpalStats}, streamer::Direction, udp::{UdpStats, UdpStreamFlow}
}, streamer_config::StreamerConfig, AppTest, AppTestDebug};

use std::fmt::Debug;

fn main() {
    //let app = App
    let (config, device) = StreamerConfig::from_cli_args(Direction::Sender).unwrap();

    dbg!(&config);

    let (app, debug) = AppTest::<f32>::new(config.clone());
    app.serve("10.0.0.24:1234", config.clone(), device).unwrap();
}
