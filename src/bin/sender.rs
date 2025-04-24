use std::{net::Ipv4Addr, str::FromStr};

use audio_streamer::{
    DEFAULT_PORT, SENDER_BUFFER_SIZE, enumerate, search_device, search_for_host,
    sender::{AudioSender, args::SenderCliArgs, tui::run_tui},
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
        let _sender = AudioSender::new(
            &device,
            config,
            Ipv4Addr::from_str(target_server.as_str())?,
            args.port.unwrap_or(DEFAULT_PORT),
            buf_size,
        )?;
        
        //wait_for_key("Sending... Press ctrl-c to stop");
        if args.ui {
            run_tui(&device, _sender.udp_rx, _sender.cpal_rx).unwrap();
        } else {
            println!("Sending... Press ctrl-c to stop");
            loop {}
        }
    } else {
        println!("Invalid Host Address");
    }

    Ok(())
}
