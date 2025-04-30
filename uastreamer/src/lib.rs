use std::{
    fmt::Debug, fs::{create_dir_all, File}, io::BufWriter, net::{IpAddr, SocketAddr}, str::FromStr, sync::{mpsc::{self, Receiver, Sender}, Arc, Mutex}, thread::JoinHandle, time::SystemTime
};

use bytemuck::Pod;

use components::{
    control::TcpControlFlow,
    cpal::{CpalAudioFlow, CpalStats},
    udp::{UdpStats, UdpStreamFlow},
};
use cpal::{traits::*, *};
use hound::WavWriter;

use ringbuf::{traits::{Observer, Split}, HeapCons, HeapProd};
use config::{get_cpal_config, StreamerConfig};
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

    #[cfg(debug_assertions)]
    create_dir_all("dump/")?;

    #[cfg(debug_assertions)]
    let writer = Ok(Some(hound::WavWriter::create(
        format!("dump/{}_{}.wav", timestamp.as_secs(), filename),
        spec,
    )?));

    #[cfg(not(debug_assertions))]
    let writer = Ok(None);

    writer
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
    direction: Direction,
    audio_buffer_prod: Arc<Mutex<HeapProd<T>>>,
    audio_buffer_cons: Arc<Mutex<HeapCons<T>>>,
    cpal_stats_sender: Sender<CpalStats>,
    udp_stats_sender: Sender<UdpStats>,
    config: Arc<Mutex<StreamerConfig>>,
    pub pool: ThreadPool
}

impl<T> App<T> where T: cpal::SizedSample + Send + Pod + Default + Debug + 'static {
    pub fn new(config: StreamerConfig) -> (Self, AppDebug) {
        dbg!(&config);
        let audio_buffer = ringbuf::HeapRb::<T>::new(config.buffer_size * config.selected_channels.len());
        println!("{}", audio_buffer.capacity());
        
        let (audio_buffer_prod, audio_buffer_cons) = audio_buffer.split();
        
        let (cpal_stats_sender, cpal_stats_receiver) = mpsc::channel::<CpalStats>();
        let (udp_stats_sender, udp_stats_receiver) = mpsc::channel::<UdpStats>();
        
        let direction = config.direction.clone();
        
        (Self {
            audio_buffer_prod: Arc::new(Mutex::new(audio_buffer_prod)),
            audio_buffer_cons: Arc::new(Mutex::new(audio_buffer_cons)),
            cpal_stats_sender,
            udp_stats_sender,
            config: Arc::new(Mutex::new(config)),
            pool: ThreadPool::new(5),
            direction
        },
        AppDebug {
            cpal_stats_receiver,
            udp_stats_receiver,
        })
    }
}

pub struct AppDebug {
    cpal_stats_receiver: Receiver<CpalStats>,
    udp_stats_receiver: Receiver<UdpStats>,
}

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> CpalAudioFlow<T>
    for App<T>
{
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

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> UdpStreamFlow<T>
    for App<T>
{
    fn udp_get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn udp_get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }
    
    fn get_udp_stats_sender(&self) -> Sender<UdpStats> {
        self.udp_stats_sender.clone()
    }
}

impl<T> TcpControlFlow for App<T> where 
    T: cpal::SizedSample + Send + Pod + Default + Debug + 'static
{
    fn start_stream(&self, config: StreamerConfig, target: SocketAddr) -> anyhow::Result<()> {
        //start video capture and udp sender here
        let dir = config.direction;
        {
            dbg!("Constructing CPAL Stream");
            let stats = self.get_cpal_stats_sender();
            let prod = self.get_producer();
            let cons = self.get_consumer();
            
            let streamer_config = config.clone();

            self.pool.execute(move || {
                // TODO maybe i should initialize the device here and don't store it on the streamer config
                let (d, stream_config) = get_cpal_config(config.direction, config.program_args.audio_host, config.program_args.device).unwrap();

                //let device = device.lock().unwrap();
                let _stream = Self::construct_stream(dir, &d, stream_config, streamer_config, stats, prod, cons).unwrap();
                loop {
                    //block
                }
            });
        }
        
        {
            dbg!("Constructing UDP Stream");
            let prod = self.udp_get_producer();
            let cons = self.udp_get_consumer();
            let stats = self.get_udp_stats_sender();
            dbg!(&target);

            let mut t = target.clone();
            t.set_ip(IpAddr::from_str("0.0.0.0").unwrap());
            t.set_port(42069);

            self.pool.execute(move || {
                Self::construct_udp_stream(dir, t, cons, prod, stats, config.send_stats).unwrap();
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_test_transfer() {}
}
