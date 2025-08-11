use std::{fs::File, io::{BufWriter}, time::SystemTime};

use bytemuck::Pod;
use hound::WavWriter;

pub mod packet;
pub mod receiver;
pub mod sender;

pub type DebugWavWriter = WavWriter<BufWriter<File>>;

/// Creates the debug wav writer
pub fn create_wav_writer(filename: String, channels: usize, sample_rate: usize) -> anyhow::Result<DebugWavWriter> {
    let spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate: sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let now = SystemTime::now();

    let timestamp = now.duration_since(SystemTime::UNIX_EPOCH)?;

    use std::fs::create_dir_all;

    use log::info;

    let fname = format!("dump/{}_{}.wav", timestamp.as_secs(), filename);
    info!("Initializing Debug Wav Writer at {}", fname);
    create_dir_all("dump/")?;
    let writer = hound::WavWriter::create(fname, spec)?;

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
