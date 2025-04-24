use std::{net::Ipv4Addr, str::FromStr};

use audio_streamer::{
    args::SenderCliArgs, enumerate, search_device, search_for_host, streamer::{self, StreamComponent, Streamer}, tui::tui, DEFAULT_PORT, SENDER_BUFFER_SIZE
};
use clap::Parser;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait},
};


fn main() -> anyhow::Result<()> {
    let args = SenderCliArgs::parse();
    // parse audio system host name
    let host = if let Some(host) = args.audio_host {
        search_for_host(&host)?
    } else {
        cpal::default_host()
    };

    // parse device selector
    let device = if let Some(device) = args.device {
        //search_for_input_device(host, &device)?
        host.input_devices()?.find(|x| search_device(x, &device))
    } else {
        host.default_input_device()
    }
    .expect("no input device");

    if args.enumerate {
        enumerate(&host).unwrap();
        return Ok(())
    }

    println!(
        "Using device {}",
        device.name().unwrap_or("Unknown Device".to_string())
    );

    // parse buffer and channel selector
    let buf_size;
    let config = match (args.channel, args.buffer_size) {
        (Some(channel), Some(buffer_size)) => {
            buf_size = buffer_size;

            StreamConfig {
                channels: channel,
                sample_rate: cpal::SampleRate(44100),
                buffer_size: cpal::BufferSize::Fixed(buffer_size),
            }
        }
        (_, _) => {
            buf_size = SENDER_BUFFER_SIZE as u32;
            device.default_input_config()?.into()
        }
    };

    #[cfg(debug_assertions)]
    dbg!(&config);

    if let Some(target_server) = args.target_server {
        let sender = Streamer::construct::<f32>(streamer::Direction::Sender, 42069, Ipv4Addr::from_str(&target_server).expect("Invalid Host Address"), &device, &config, buf_size.try_into().unwrap()).unwrap();
        if args.ui {
            tui(streamer::Direction::Sender, &device, sender.net_stats, sender.cpal_stats).unwrap();
        } else {
            println!("Sending... Press ctrl-c to stop");
            loop {}
        }
    }

    Ok(())
}
