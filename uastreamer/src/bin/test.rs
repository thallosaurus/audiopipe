use std::net::UdpSocket;
use cpal::traits::StreamTrait;

use cpal::{traits::{DeviceTrait, HostTrait}, StreamConfig};
use uastreamer::{enumerate, search_device};

const VBC: &'static str = "CABLE Output (VB-Audio Virtual Cable)";

fn main() {
    let host = cpal::default_host();
    enumerate(uastreamer::Direction::Sender, &host).unwrap();

    let device = host.input_devices().unwrap().find(|x| search_device(x, VBC)).expect("didnt find vbc");
    let config = device.default_input_config().unwrap();

    let mut c: StreamConfig = config.into();
    c.buffer_size = cpal::BufferSize::Fixed(1024);
    c.sample_rate = cpal::SampleRate(44100);

    println!("{:?}", c);

    let stream = device.build_input_stream(&c, move |data: &[f32], info| {
        println!("Data Length: {}, Channels: {}, Individual Buffer Size: {}", data.len(), c.channels, data.len() / c.channels as usize);
    }, move |err| eprintln!("{}", err), None).unwrap();

    stream.play();

    loop {}
}
