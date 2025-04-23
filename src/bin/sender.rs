use std::{env, net::Ipv4Addr, str::FromStr};

use audio_streamer::{AudioSender, enumerate};
use cpal::{
    Device, Host, StreamConfig,
    traits::{DeviceTrait, HostTrait},
};

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = SenderCliArgs::parse();
    if args.enumerate {
        enumerate().unwrap();
        return Ok(());
    } else {
        let host = if let Some(host) = args.audio_host {
            search_for_host(&host)?
        } else {
            cpal::default_host()
        };

        let device = if let Some(device) = args.device {
            //search_for_input_device(host, &device)?
            host.input_devices()?.find(|x| search_device(x, &device))
        } else {
            host.default_input_device()
        }
        .expect("no input device");


        let config = if let Some(buffer_size) = args.buffer_size {
            StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(44100),
                buffer_size: cpal::BufferSize::Fixed(buffer_size)
            }
        } else {
            device.default_input_config()?.into()
        };

        dbg!(&config);

        let _sender = AudioSender::new(
            device,
            config,
            Ipv4Addr::from_str(args.target_server.as_str())?,
        );

        //wait_for_key("Sending... Press ctrl-c to stop");
        println!("Sending... Press ctrl-c to stop");
        loop {}
    }

    Ok(())
}

fn search_for_host(name: &str) -> anyhow::Result<Host> {
    let host_id = cpal::available_hosts()
        .into_iter()
        .find(|id| id.name() == name)
        .expect("error while retriving Sound Host Name");

    Ok(cpal::host_from_id(host_id)?)
}

fn search_device(x: &Device, name: &str) -> bool {
    x.name().map(|y| y == name).unwrap_or(false)
}

fn wait_for_key(msg: &str) {
    println!("{}", msg);
    std::io::stdin().read_line(&mut String::new()).unwrap();
}

#[derive(Parser, Debug)]
#[command(version, about = "Sender", long_about = None)]
struct SenderCliArgs {
    /// Name of the Audio Host
    #[arg(short, long)]
    audio_host: Option<String>,

    /// Name of the used Audio Device
    #[arg(short, long)]
    device: Option<String>,

    /// Buffer Size
    #[arg(short, long)]
    buffer_size: Option<u32>,

    /// Dump Audio Config
    #[arg(short)]
    enumerate: bool,

    /// Target IP of the server
    #[arg(short)]
    target_server: String,
}
