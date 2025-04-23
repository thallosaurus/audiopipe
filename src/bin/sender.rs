use std::{env, net::Ipv4Addr};

use audio_streamer::AudioSender;
use cpal::traits::{DeviceTrait, HostTrait};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    

    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    let config = device.default_input_config()?;

    let _sender = AudioSender::new(device, config, Ipv4Addr::new(10,0,0,41));

    //wait_for_key("Sending... Press ctrl-c to stop");
    println!("Sending... Press ctrl-c to stop");
    loop {}

    Ok(())
}

fn wait_for_key(msg: &str) {
    println!("{}", msg);
    std::io::stdin().read_line(&mut String::new()).unwrap();
}