use std::{net::Ipv4Addr, str::FromStr};

use audio_streamer::{
    enumerate, receiver::{args::ReceiverCliArgs, tui::run_tui}, search_device, search_for_host, streamer::{self, StreamComponent, Streamer}, DEFAULT_PORT, SENDER_BUFFER_SIZE
};
use clap::Parser;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait},
};

fn main() -> anyhow::Result<()> {
    //enumerate().unwrap();

    let args = ReceiverCliArgs::parse();
    //if args.enumerate {
    //enumerate().unwrap();
    //    return Ok(());
    //} else {
    // parse audio system host name
    let host = if let Some(host) = args.audio_host {
        search_for_host(&host)?
    } else {
        cpal::default_host()
    };

    if args.enumerate {
        enumerate(&host).unwrap();
        return Ok(())
    }

    // parse device selector
    let device = if let Some(device) = args.device {
        //search_for_input_device(host, &device)?
        host.output_devices()?.find(|x| search_device(x, &device))
    } else {
        host.default_output_device()
    }
    .expect("no output device");

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
            device.default_output_config()?.into()
        }
    };

    dbg!(&config);

    #[cfg(debug_assertions)]
    let wave_output = args.wave_output;
    
    #[cfg(not(debug_assertions))]
    let wave_output = false;

    let receiver = Streamer::construct::<f32>(streamer::Direction::Receiver, 42069, Ipv4Addr::from_str("0.0.0.0").expect("Invalid Host Address"), &device, &config, buf_size.try_into().unwrap()).unwrap();
    if args.ui {
        run_tui(&device, receiver.net_stats, receiver.cpal_stats).unwrap();
    } else {
        println!("Receiving. Press ctrl+c to exit");
        loop {}
    }

    Ok(())
}
