use std::{
    net::{self, UdpSocket},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
};

use cpal::{
    Device, Sample, Stream, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{traits::{Consumer, Observer, Producer, Split}, HeapRb};

const PORT: u16 = 42069;
const RECEIVER_BUFFER_SIZE: usize = 8192;
const SENDER_BUFFER_SIZE: usize = 8192;

pub mod receiver_tui;

pub struct UdpStats {
    pub received: usize,
    pub occupied_buffer: usize,
}

pub struct CpalStats {
    requested_sample_length: usize,
}

pub struct AudioReceiver {
    _stream: Stream,
    pub udp_rx: Receiver<UdpStats>,
    pub cpal_rx: Receiver<CpalStats>,
}

impl AudioReceiver {
    pub fn new(
        device: &Device,
        config: SupportedStreamConfig,
        closing_rx: Receiver<bool>
    ) -> anyhow::Result<AudioReceiver> {
        //let device = host.default_output_device().expect("no output device available");
        //let config = device.default_output_config()?;
        println!("Using device: {}", device.name()?);
        println!("Sample format: {:?}", config.sample_format());

        let (tx, udp_rx) = mpsc::channel::<UdpStats>();

        let ring = ringbuf::HeapRb::<f32>::new(RECEIVER_BUFFER_SIZE);
        let (producer, mut consumer) = ring.split();
        let producer = Arc::new(Mutex::new(producer));

        let producer_clone = Arc::clone(&producer);
        std::thread::spawn(move || {
            let socket = UdpSocket::bind(("0.0.0.0", PORT)).expect("Failed to bind UDP socket");
            println!("Listening on UDP port {}", PORT);

            let mut buf = [0u8; RECEIVER_BUFFER_SIZE];

            loop {
                match socket.recv(&mut buf) {
                    Ok(received) => {
                        let samples = received / std::mem::size_of::<f32>();
                        let float_samples = unsafe {
                            std::slice::from_raw_parts(buf.as_ptr() as *const f32, samples)
                        };

                        let mut prod = producer_clone.lock().unwrap();

                        for &sample in float_samples {
                            let _ = prod.try_push(sample);
                        }

                        let occupied_buffer = prod.occupied_len();

                        if let Err(ok) = tx.send(UdpStats {
                            received,
                            occupied_buffer,
                        }) {
                            return;
                        }
                    }
                    Err(e) => eprintln!("UDP receive error: {:?}", e),
                }
            }
        });

        let (cpal_tx, cpal_rx) = mpsc::channel::<CpalStats>();

        let stream = device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _| {
                for sample in output.iter_mut() {
                    *sample = consumer.try_pop().unwrap_or(Sample::EQUILIBRIUM);
                }
                
                if let Err(ok) = cpal_tx
                    .send(CpalStats {
                        requested_sample_length: output.len(),
                    }) {
                        return;
                    }
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?;

        stream.play()?;

        Ok(AudioReceiver {
            _stream: stream,
            udp_rx,
            cpal_rx,
        })
    }
}

pub struct AudioSender {
    _stream: Stream
}

pub fn sender(ip: net::Ipv4Addr) -> anyhow::Result<AudioSender> {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    let config = device.default_input_config()?;

    println!("Using device: {}", device.name()?);
    println!("Sample format: {:?}", config.sample_format());

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(format!("{}:{}", ip, PORT))?;

    //let err_fn = |err| eprintln!("Stream error: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_f32_stream(&device, &config.into(), socket),
        /*cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), socket),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), socket),
        cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config.into(), socket),
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config.into(), socket),
        cpal::SampleFormat::I64 => build_stream::<i64>(&device, &config.into(), socket),
        cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config.into(), socket),
        cpal::SampleFormat::U32 => build_stream::<u32>(&device, &config.into(), socket),
        cpal::SampleFormat::U64 => build_stream::<u64>(&device, &config.into(), socket),
        cpal::SampleFormat::F64 => build_stream::<f64>(&device, &config.into(), socket),*/
        _ => todo!(),
    }?;

    stream.play()?;

    wait_for_key("Stream started... Press enter to stop");
    Ok(AudioSender { _stream: stream })
}

fn build_f32_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    socket: UdpSocket,
) -> Result<cpal::Stream, anyhow::Error>
{
    //let channels = config.channels as usize;
    let buf = HeapRb::<u8>::new(SENDER_BUFFER_SIZE);
    let (prod, mut cons) = buf.split();

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _| {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    data.as_ptr() as *const f32,
                    data.len() * std::mem::size_of::<f32>(),
                )
            };
        },
        |err| eprintln!("Stream error: {}", err),
        None,
    )?;
    
    let udp_loop = std::thread::spawn(move || {
        loop {
            if cons.is_full() {
                let mut buf = [0u8; RECEIVER_BUFFER_SIZE];

                let data = cons.pop_slice(&mut buf);
                let _ = socket.send(buf.as_mut_slice());
            }
        }
    });

    Ok(stream)
}

fn wait_for_key(msg: &str) {
    println!("{}", msg);
    std::io::stdin().read_line(&mut String::new()).unwrap();
}

pub fn enumerate() -> Result<(), anyhow::Error> {
    println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);

    for host_id in available_hosts {
        println!("{}", host_id.name());
        let host = cpal::host_from_id(host_id)?;

        let default_in = host.default_input_device().map(|e| e.name().unwrap());
        let default_out = host.default_output_device().map(|e| e.name().unwrap());
        println!("  Default Input Device:\n    {:?}", default_in);
        println!("  Default Output Device:\n    {:?}", default_out);

        let devices = host.devices()?;
        println!("  Devices: ");
        for (device_index, device) in devices.enumerate() {
            println!("  {}. \"{}\"", device_index + 1, device.name()?);

            // Input configs
            if let Ok(conf) = device.default_input_config() {
                println!("    Default input stream config:\n      {:?}", conf);
            }
            let input_configs = match device.supported_input_configs() {
                Ok(f) => f.collect(),
                Err(e) => {
                    println!("    Error getting supported input configs: {:?}", e);
                    Vec::new()
                }
            };
            if !input_configs.is_empty() {
                println!("    All supported input stream configs:");
                for (config_index, config) in input_configs.into_iter().enumerate() {
                    println!(
                        "      {}.{}. {:?}",
                        device_index + 1,
                        config_index + 1,
                        config
                    );
                }
            }

            // Output configs
            if let Ok(conf) = device.default_output_config() {
                println!("    Default output stream config:\n      {:?}", conf);
            }
            let output_configs = match device.supported_output_configs() {
                Ok(f) => f.collect(),
                Err(e) => {
                    println!("    Error getting supported output configs: {:?}", e);
                    Vec::new()
                }
            };
            if !output_configs.is_empty() {
                println!("    All supported output stream configs:");
                for (config_index, config) in output_configs.into_iter().enumerate() {
                    println!(
                        "      {}.{}. {:?}",
                        device_index + 1,
                        config_index + 1,
                        config
                    );
                }
            }
        }
    }

    Ok(())
}
