use std::sync::{
    mpsc::{self, Receiver, Sender}, Arc, Mutex
};

use bytemuck::Pod;
use ringbuf::{traits::Split, HeapCons, HeapProd};
use uastreamer::{components::{
    control::TcpControlFlow, cpal::{CpalAudioFlow, CpalStats}, streamer::Direction, udp::{UdpStats, UdpStreamFlow}
}, streamer_config::StreamerConfig};

use std::fmt::Debug;

struct AppTest<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    audio_buffer_prod: Arc<Mutex<HeapProd<T>>>,
    audio_buffer_cons: Arc<Mutex<HeapCons<T>>>,
    cpal_stats_sender: Sender<CpalStats>,
    udp_stats_sender: Sender<UdpStats>,
    config: StreamerConfig
}

impl<T> AppTest<T> where T: cpal::SizedSample + Send + Pod + Default + Debug + 'static {
    fn new(config: StreamerConfig) -> (Self, AppTestDebug) {
        let audio_buffer = ringbuf::HeapRb::<T>::new(config.buffer_size);

        let (audio_buffer_prod, audio_buffer_cons) = audio_buffer.split();

        let (cpal_stats_sender, cpal_stats_receiver) = mpsc::channel::<CpalStats>();
        let (udp_stats_sender, udp_stats_receiver) = mpsc::channel::<UdpStats>();
        
        (Self {
            audio_buffer_prod: Arc::new(Mutex::new(audio_buffer_prod)),
            audio_buffer_cons: Arc::new(Mutex::new(audio_buffer_cons)),
            cpal_stats_sender,
            udp_stats_sender,
            config,
        },
        AppTestDebug {
            cpal_stats_receiver,
            udp_stats_receiver,
        })
    }
}

struct AppTestDebug {
    cpal_stats_receiver: Receiver<CpalStats>,
    udp_stats_receiver: Receiver<UdpStats>,
}

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> CpalAudioFlow<T>
    for AppTest<T>
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
    for AppTest<T>
{
    fn udp_get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn udp_get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }
}

impl<T> TcpControlFlow for AppTest<T> where 
    T: cpal::SizedSample + Send + Pod + Default + Debug + 'static
{
    fn start_stream(&self, config: uastreamer::streamer_config::StreamerConfig, device: cpal::Device, target: &str) -> anyhow::Result<()> {
        //start video capture and udp sender here
        let dir = config.direction;
        let stream = self.construct_stream(dir, &device, config.clone())?;
        self.construct_udp_stream(dir, config.clone(), target)?;
        Ok(())
    }
    
    fn get_tcp_direction(&self) -> Direction {
        self.config.direction
    }
}

/*pub fn from_sample_format(
    format: cpal::SampleFormat,
    target: net::SocketAddr,
    device: &cpal::Device,
    streamer_config: StreamerConfig,
) -> anyhow::Result<Box<dyn CpalAudioFlow>> {
    // TODO Implement sample conversion for debug hound writer
    Ok(match format {
        cpal::SampleFormat::I16 => Self::construct::<i16>(target, device, streamer_config),
        cpal::SampleFormat::U16 => Self::construct::<u16>(target, device, streamer_config),
        cpal::SampleFormat::I8 => Self::construct::<i8>(target, device, streamer_config),
        cpal::SampleFormat::I32 => Self::construct::<i32>(target, device, streamer_config),
        cpal::SampleFormat::I64 => Self::construct::<i64>(target, device, streamer_config),
        cpal::SampleFormat::U8 => Self::construct::<u8>(target, device, streamer_config),
        cpal::SampleFormat::U32 => Self::construct::<u32>(target, device, streamer_config),
        cpal::SampleFormat::U64 => Self::construct::<u64>(target, device, streamer_config),
        cpal::SampleFormat::F64 => Self::construct::<f64>(target, device, streamer_config),
        cpal::SampleFormat::F32 => Self::construct::<f32>(target, device, streamer_config),
        _ => panic!("Unsupported Sample Format: {:?}", format),
    }?)
}*/

fn main() {
    //let app = App
    let (config, device) = StreamerConfig::from_cli_args(Direction::Sender).unwrap();

    let (app, debug) = AppTest::<f32>::new(config.clone());
    app.serve("10.0.0.24", config.clone(), device);
}
