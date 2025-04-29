use std::sync::{mpsc::{Receiver, Sender}, Arc, Mutex};

use bytemuck::Pod;
use ringbuf::{HeapCons, HeapProd};
use uastreamer::components::{cpal::{CpalAudioFlow, CpalStats}, udp::UdpStreamFlow};

use std::fmt::Debug;


struct AppTest<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    audio_buffer_prod: Arc<Mutex<HeapProd<T>>>,
    audio_buffer_cons: Arc<Mutex<HeapCons<T>>>,
    cpal_stats_sender: Sender<CpalStats>
}

struct AppTestDebug {
    cpal_stats_receiver: Receiver<CpalStats>
}

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> CpalAudioFlow<T> for AppTest<T> {
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

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> UdpStreamFlow<T> for AppTest<T> {}

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

}
