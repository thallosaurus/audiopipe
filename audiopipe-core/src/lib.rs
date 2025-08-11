
use cpal::{traits::*, *};

//use config::{StreamerConfig, get_cpal_config};
use log::{debug, info};

use crate::{
    audio::{
        select_input_device_config, select_output_device_config, set_global_master_input_mixer,
        set_global_master_output_mixer, setup_master_output,
    },
    control::{client::TcpClient, server::TcpServer},
    mixer::{MixerTrackSelector, default_client_mixer, default_server_mixer},
    streamer::{receiver::AudioReceiverHandle, sender::AudioSenderHandle},
};

/// maximum permitted size of one UDP payload
pub const MAX_UDP_CLIENT_PAYLOAD_SIZE: usize = 512;

/// Holds everything related to the audio buffer splitter
//pub mod splitter;

/// holds all audio related stuff
pub mod audio;

/// implementation of the mixer module
pub mod mixer;

/// implementation of the server control stack over TCP
pub mod control;

/// implementation of the audio sender oder UDP
pub mod streamer;

pub static MESSAGE: &'static str = "\nmay the bytes that flow through this logic make your life a little less shit\n";

/// Searches for the specified Audio [cpal::HostId] encoded as string
pub fn search_for_host(name: &str) -> anyhow::Result<Host> {
    let host_id = cpal::available_hosts()
        .into_iter()
        .find(|id| id.name() == name)
        .expect("error while retriving sound host name");

    Ok(cpal::host_from_id(host_id)?)
}

/// Lambda function to check if the specified [cpal::Device::name] is the specified name
pub fn search_device(x: &Device, name: &str) -> bool {
    x.name().map(|y| y == name).unwrap_or(false)
}

pub async fn init_sender(
    target: String,
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: usize,
    srate: usize,
    master_track_selector: MixerTrackSelector,
) -> TcpClient {
    let (input_device, sconfig) = setup_cpal_input(audio_host, device_name, bsize, srate);

    let mixer = default_client_mixer(sconfig.channels.into(), bsize, srate);

    let master_stream =
        audio::setup_master_input(input_device, &sconfig, mixer.1, master_track_selector)
            .await
            .expect("couldn't build master output");

    set_global_master_input_mixer(mixer.0).await;

    master_stream.play().unwrap();

    TcpClient::new(target, AudioSenderHandle::new)
}

pub async fn init_receiver(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: usize,
    srate: usize,
    addr: Option<String>,
    master_track_selector: MixerTrackSelector,
) -> TcpServer {
    let (output_device, sconfig) = setup_cpal_output(audio_host, device_name, bsize, srate);

    let chcount = sconfig.channels;

    let (output, input) = default_server_mixer(chcount as usize, bsize, srate);

    let master_stream = setup_master_output(output_device, sconfig, output, master_track_selector)
        .await
        .expect("couldn't build master output");

    set_global_master_output_mixer(input).await;

    master_stream.play().unwrap();

    let server = TcpServer::new(
        String::from(addr.unwrap_or("0.0.0.0".to_string())),
        AudioReceiverHandle::new,
    );
    server
    //server.block();
}

fn setup_cpal_output(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: usize,
    srate: usize,
) -> (Device, StreamConfig) {
    let audio_host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    debug!("Searching for device name: {:?}", device_name);

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
    let chcount = sconfig.channels;

    info!(
        "Using Audio Device {}, Sample Rate: {}, Buffer Size: {:?}, Channel Count: {}",
        output_device
            .name()
            .unwrap_or("Unknown Device Name".to_string()),
        sconfig.sample_rate.0,
        sconfig.buffer_size,
        chcount
    );

    (output_device, sconfig)
}

fn setup_cpal_input(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: usize,
    srate: usize,
) -> (Device, StreamConfig) {
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

    (input_device, sconfig)
}

pub fn enumerate_devices(audio_host: Option<String>, device_name: Option<String>) {
    println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    println!("");
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);
    println!("");

    let host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    println!("Available Output Devices:");
    for d in host.output_devices().unwrap() {
        println!("{}", d.name().unwrap());
    }

    println!("");

    println!("Available Input Devices:");
    for d in host.input_devices().unwrap() {
        println!("{}", d.name().unwrap());
    }

    println!("");

    let input_device = match device_name {
        Some(ref d) => host.input_devices().unwrap().find(|x| search_device(x, &d)),
        None => {
            println!("Using system default input device");
            host.default_input_device()
        }
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

    let output_device = match device_name {
        Some(ref d) => host
            .output_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => {
            println!("Using system default output device");
            host.default_output_device()
        }
    }
    .expect("no output device");

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

#[cfg(test)]
mod tests {

    #[test]
    fn x_test_transfer() {}

    pub fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }
}
