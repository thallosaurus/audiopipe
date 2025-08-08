use clap::Parser;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use log::info;
use uastreamer::{
    async_comp::{
        audio::{select_input_device_config, select_output_device_config, setup_master_output},
        tcp::tcp_server,
    },
    cli::{Cli, Commands},
    search_device, search_for_host,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    env_logger::init();

    let bsize = cli.global.buffer_size.unwrap_or(1024);
    let srate = cli.global.samplerate.unwrap_or(44100);
    match cli.command {
        Commands::Sender(sender_commands) => {
            init_sender(
                sender_commands.target,
                cli.global.audio_host,
                cli.global.device,
                bsize,
                srate,
            )
            .await
        }
        Commands::Receiver => {
            init_receiver(cli.global.audio_host, cli.global.device, bsize, srate).await
        }
        Commands::EnumDevices => {
            enumerate_devices(cli.global.audio_host, cli.global.device);
        }
    }
}

async fn init_sender(
    target: String,
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: u32,
    srate: u32,
) {
    let audio_host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    let input_device = match device_name {
        Some(d) => audio_host
            .input_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => audio_host.default_input_device(),
    }
    .expect("no input device");

    let config = select_input_device_config(&input_device, bsize, srate, 2);

    let bs = *config.buffer_size();
    let max_bufsize = match bs {
        cpal::SupportedBufferSize::Range { min, max } => max,
        cpal::SupportedBufferSize::Unknown => panic!("Unknown Supported Buffer Size"),
    };

    let sconfig: StreamConfig = config.into();

    info!(
        "Using Audio Device {}, Sample Rate: {}, Buffer Size: {:?}, Channel Count: {}",
        input_device
            .name()
            .unwrap_or("Unknown Device Name".to_string()),
        sconfig.sample_rate.0,
        sconfig.buffer_size,
        sconfig.channels
    );

    let master_stream = uastreamer::async_comp::audio::setup_master_input(
        input_device,
        &sconfig,
        max_bufsize as usize,
        vec![0, 1],
    )
    .await
    .expect("couldn't build master output");

    master_stream.play().unwrap();

    uastreamer::async_comp::tcp::tcp_client(&target, &sconfig, 2, max_bufsize)
        .await
        .unwrap();
}

async fn init_receiver(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: u32,
    srate: u32,
) {
    let audio_host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    let output_device = match device_name {
        Some(d) => audio_host
            .output_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => audio_host.default_output_device(),
    }
    .expect("no output device");

    let config = select_output_device_config(&output_device, bsize, srate, 2);
    let bs = *config.buffer_size();

    let max_bufsize = match bs {
        cpal::SupportedBufferSize::Range { min, max } => max,
        cpal::SupportedBufferSize::Unknown => panic!("Unknown Supported Buffer Size"),
    };

    info!("max_buffer_size: {}", max_bufsize);

    let sconfig: StreamConfig = config.into();

    info!(
        "Using Audio Device {}, Sample Rate: {}, Buffer Size: {:?}, Channel Count: {}",
        output_device
            .name()
            .unwrap_or("Unknown Device Name".to_string()),
        sconfig.sample_rate.0,
        sconfig.buffer_size,
        sconfig.channels
    );

    let master_stream =
        setup_master_output(output_device, sconfig, max_bufsize as usize, vec![0, 1])
            .await
            .expect("couldn't build master output");

    master_stream.play().unwrap();

    tcp_server("0.0.0.0", 2, bsize).await.unwrap();
}

fn enumerate_devices(audio_host: Option<String>, device_name: Option<String>) {
    let host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    let output_device = match device_name {
        Some(ref d) => host
            .output_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => host.default_output_device(),
    }
    .expect("no output device");

    let input_device = match device_name {
        Some(ref d) => host.input_devices().unwrap().find(|x| search_device(x, &d)),
        None => host.default_input_device(),
    }
    .expect("no input device");

    println!("Supported Configs for Device {:?}", input_device.name());
    for c in input_device.supported_input_configs().unwrap() {
        println!(
            "- BufferSize: {:?}, Channels: {}, Min Supported Sample Rate: {}, Max Supported Sample Rate: {}",
            c.buffer_size(),
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0
        );
    }

    println!("");

    println!("Supported Configs for Device {:?}", output_device.name());
    for c in output_device.supported_output_configs().unwrap() {
        println!(
            "- BufferSize: {:?}, Channels: {}, Min Supported Sample Rate: {}, Max Supported Sample Rate: {}",
            c.buffer_size(),
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0
        );
    }

    println!("");
}
