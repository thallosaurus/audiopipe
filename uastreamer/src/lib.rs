use std::{
    fmt::Debug,
    fs::{File, create_dir_all},
    io::BufWriter,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, channel},
    },
    time::SystemTime,
};

use bytemuck::Pod;

use components::{
    control::{TcpControlFlow, TcpErrors, TcpResult},
    cpal::{CpalAudioFlow, CpalStats, CpalStatus},
    udp::{NetworkUDPStats, UdpStatus, UdpStreamFlow},
};
use cpal::{traits::*, *};
use hound::WavWriter;

use config::{StreamerConfig, get_cpal_config};
use log::{debug, error, info};
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Observer, Split},
};
use threadpool::ThreadPool;

/// Default Port if none is specified
pub const DEFAULT_PORT: u16 = 42069;

/// Default Buffer Size if none is specified
pub const SENDER_BUFFER_SIZE: usize = 1024;

/// Re-export of the Cargo Package Name
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// Re-export of the Cargo Package Version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Re-export of the Cargo Package Description
pub const PKG_DESC: &str = env!("CARGO_PKG_DESCRIPTION");

/// Holds all things related to the statistics debug window
pub mod tui;

/// Holds everything needed for command line argument parsing
pub mod args;

/// Holds everything related to the audio buffer splitter
pub mod splitter;

/// Holds the config struct which gets passed around
pub mod config;

/// Holds all flows this app offers
pub mod components;

pub mod ualog;

/// Defines the behavior of the stream
///
/// Sender: Captures from an audio input stream and sends it over the network
/// Receiver: Receives from the network and outputs it to a audio output stream
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Sender,
    Receiver,
}

/// Enumerate all available devices on the system
pub fn enumerate(direction: Direction, host: &Host) -> anyhow::Result<()> {
    println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);

    println!("Devices:");
    for device in host.devices()?.into_iter() {
        match direction {
            Direction::Sender => {
                println!(
                    " - {:?} (Input)",
                    device.name().unwrap_or("Unknown Device".to_string())
                );
                if let Ok(config) = device.supported_input_configs() {
                    for supported_config in config.into_iter() {
                        let buf_size = match supported_config.buffer_size() {
                            SupportedBufferSize::Range { min, max } => format!("{}/{}", min, max),
                            SupportedBufferSize::Unknown => format!("Unknown"),
                        };

                        println!(
                            "   - Buffer Min/Max: {}, Channels: {}, Sample Format: {}, Sample Rate: {:?}",
                            buf_size,
                            supported_config.channels(),
                            supported_config.sample_format(),
                            supported_config.max_sample_rate()
                        )
                    }
                } else {
                    println!("   <not supported>");
                }
                println!("");
            }
            Direction::Receiver => {
                println!(
                    " - {:?} (Output)",
                    device.name().unwrap_or("Unknown Device".to_string())
                );

                if let Ok(conf) = device.supported_output_configs() {
                    for supported_config in conf.into_iter() {
                        let buf_size = match supported_config.buffer_size() {
                            SupportedBufferSize::Range { min, max } => format!("{}/{}", min, max),
                            SupportedBufferSize::Unknown => format!("Unknown"),
                        };

                        println!(
                            "   - Buffer Min/Max: {}, Channels: {}, Sample Format: {}, Sample Rate: {:?}",
                            buf_size,
                            supported_config.channels(),
                            supported_config.sample_format(),
                            supported_config.max_sample_rate()
                        )
                    }
                } else {
                    println!("   <not supported>")
                }
                println!("");
            }
        };
    }

    Ok(())
}

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

pub type DebugWavWriter = WavWriter<BufWriter<File>>;

/// Creates the debug wav writer
pub fn create_wav_writer(
    filename: String,
    channels: u16,
    sample_rate: u32,
) -> anyhow::Result<Option<DebugWavWriter>> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let now = SystemTime::now();

    let timestamp = now.duration_since(SystemTime::UNIX_EPOCH)?;

    let mut writer: Option<DebugWavWriter> = None;
    #[cfg(debug_assertions)]
    {
        let fname = format!("dump/{}_{}.wav", timestamp.as_secs(), filename);
        info!("Initializing Debug Wav Writer at {}", fname);
        create_dir_all("dump/")?;
        writer = Some(hound::WavWriter::create(fname, spec)?);
    }

    Ok(writer)
}

pub fn write_debug<T: cpal::SizedSample + Send + Pod + Default + 'static>(
    writer: &mut Option<DebugWavWriter>,
    sample: T,
) {
    if let Some(writer) = writer {
        let s: f32 = bytemuck::cast(sample);
        writer.write_sample(s).unwrap();
    }
}

/// Struct that holds everything the library needs
pub struct App<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    audio_buffer_prod: Arc<Mutex<HeapProd<T>>>,
    audio_buffer_cons: Arc<Mutex<HeapCons<T>>>,
    cpal_stats_sender: Sender<CpalStats>,
    udp_stats_sender: Sender<NetworkUDPStats>,
    udp_command_sender: Option<Sender<UdpStatus>>,
    cpal_command_sender: Option<Sender<CpalStatus>>,

    _config: Arc<Mutex<StreamerConfig>>,
    pub pool: ThreadPool,
    //thread_channels: Option<(Sender<bool>, Sender<bool>)>,
}

impl<T> App<T>
where
    T: cpal::SizedSample + Send + Pod + Default + Debug + 'static,
{
    pub fn new(config: StreamerConfig) -> (Self, AppDebug) {
        debug!("Using Config: {:?}", &config);
        let audio_buffer =
            ringbuf::HeapRb::<T>::new(config.buffer_size * config.selected_channels.len());
        info!("Audio Buffer Size: {}", audio_buffer.capacity());

        let (audio_buffer_prod, audio_buffer_cons) = audio_buffer.split();

        let (cpal_stats_sender, cpal_stats_receiver) = mpsc::channel::<CpalStats>();
        let (udp_stats_sender, udp_stats_receiver) = mpsc::channel::<NetworkUDPStats>();

        (
            Self {
                audio_buffer_prod: Arc::new(Mutex::new(audio_buffer_prod)),
                audio_buffer_cons: Arc::new(Mutex::new(audio_buffer_cons)),
                cpal_stats_sender,
                udp_stats_sender,
                _config: Arc::new(Mutex::new(config)),
                pool: ThreadPool::new(5),
                udp_command_sender: None,
                cpal_command_sender: None,
                //thread_channels: None,
            },
            AppDebug {
                cpal_stats_receiver,
                udp_stats_receiver,
            },
        )
    }
}

pub struct AppDebug {
    cpal_stats_receiver: Receiver<CpalStats>,
    udp_stats_receiver: Receiver<NetworkUDPStats>,
}

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> CpalAudioFlow<T> for App<T> {
    fn get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }

    fn get_cpal_stats_sender(&self) -> Sender<CpalStats> {
        self.cpal_stats_sender.clone()
    }
}

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> UdpStreamFlow<T> for App<T> {
    fn udp_get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn udp_get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }

    fn get_udp_stats_sender(&self) -> Sender<NetworkUDPStats> {
        self.udp_stats_sender.clone()
    }
}

impl<T> TcpControlFlow for App<T>
where
    T: cpal::SizedSample + Send + Pod + Default + Debug + 'static,
{
    fn start_stream(
        &mut self,
        config: StreamerConfig,
        target: SocketAddr,
    ) -> TcpResult<()> {
        // Sync Channel for the buffer
        // If sent, the udp thread is instructed to empty the contents of the buffer and send them
        let (chan_sync_tx, chan_sync_rx) = channel::<bool>();

        let (cpal_channel_tx, cpal_channel_rx) = channel::<CpalStatus>();
        let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

        self.udp_command_sender = Some(udp_msg_tx);
        self.cpal_command_sender = Some(cpal_channel_tx);

        //start video capture and udp sender here
        let dir = config.direction;
        {
            info!("Constructing CPAL Stream");
            let stats = self.get_cpal_stats_sender();
            let prod = self.get_producer();
            let cons = self.get_consumer();
            let streamer_config = config.clone();

            self.pool.execute(move || {
                let (d, stream_config) = get_cpal_config(
                    config.direction,
                    config.program_args.audio_host,
                    config.program_args.device,
                )
                .unwrap();

                //let device = device.lock().unwrap();
                match Self::construct_stream(
                    dir,
                    &d,
                    stream_config,
                    streamer_config,
                    stats,
                    prod,
                    cons,
                    chan_sync_tx,
                ) {
                    Ok(stream) => {
                        stream.play().unwrap();
                        loop {
                            //block
                            if let Ok(msg) = cpal_channel_rx.try_recv() {
                                match msg {
                                    CpalStatus::DidEnd => {
                                        println!("Stopping CPAL Stream");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => error!("{}", err),
                }
            });
        }

        {
            info!("Constructing UDP Stream");
            let prod = self.udp_get_producer();
            let cons = self.udp_get_consumer();
            let stats = self.get_udp_stats_sender();
            debug!("{:?}", target);

            let t = target.clone();
            //t.set_ip(IpAddr::from_str("0.0.0.0").unwrap());
            //t.set_port(42069);let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

            self.pool.execute(move || {
                Self::construct_udp_stream(
                    dir,
                    t,
                    cons,
                    prod,
                    Some(stats),
                    udp_msg_rx,
                    chan_sync_rx,
                )
                .unwrap();
            });
        }
        Ok(())
    }

    fn stop_stream(&self) -> TcpResult<()> {
        if let Some(ch) = &self.udp_command_sender {
            ch.send(UdpStatus::DidEnd).map_err(|e| {
                TcpErrors::UdpStatsSendError(e)
            })?;
        }

        if let Some(ch) = &self.cpal_command_sender {
            ch.send(CpalStatus::DidEnd).map_err(|e| {
                TcpErrors::CpalStatsSendError(e)
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn x_test_transfer() {}
}
