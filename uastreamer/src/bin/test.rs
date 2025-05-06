use std::net::UdpSocket;

use cpal::{traits::{DeviceTrait, HostTrait}, StreamConfig};
use uastreamer::{enumerate, search_device};

fn main() {
    let host = cpal::default_host();
    enumerate(uastreamer::Direction::Sender, &host).unwrap();

    let device = host.input_devices().unwrap().find(|x| search_device(x, &"BlackHole 64ch")).expect("didnt find blackhole");
    let config = device.default_input_config().unwrap();

    let mut c: StreamConfig = config.into();
    c.buffer_size = cpal::BufferSize::Fixed(1024);

    println!("{:?}", c);

    let stream = device.build_input_stream(&c, move |data: &[f32], info| {
        println!("Data Length: {}, Channels: {}, Individual Buffer Size: {}", data.len(), c.channels, data.len() / c.channels as usize);
    }, move |err| eprintln!("{}", err), None).unwrap();

    loop {}
}
