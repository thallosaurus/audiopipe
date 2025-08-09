use std::sync::{Arc};

use cpal::Sample;
use log::debug;
use ringbuf::{traits::{Consumer, Split}, HeapCons, HeapProd};
use tokio::sync::Mutex;


pub enum MixerTrackSelector {
    Mono(usize),
    Stereo(usize, usize),
}

/// Paired Mixer Track for processing
pub enum MixerTrack {
    Mono(RawInputMixerTrack),
    Stereo(RawInputMixerTrack, RawInputMixerTrack),
}

/// Shared Reference to the Input Mixer Ringbuffer Producer
pub type RawInputMixerTrack = Arc<Mutex<HeapProd<f32>>>;
pub type RawOutputMixerTrack = Arc<std::sync::Mutex<HeapCons<f32>>>;

pub enum MixerError {
    InputChannelNotFound,
    OutputChannelNotFound
}

type MixerResult<T> = Result<T, MixerError>;

/// The Input Side of the Mixer
#[derive(Clone)]
pub struct MixerInputEnd {
    inputs: Vec<Arc<Mutex<HeapProd<f32>>>>,
    channel_count: usize
}

impl MixerInputEnd {
    pub fn get_raw_channel(&mut self, channel: usize) -> MixerResult<RawInputMixerTrack> {
        if channel < self.channel_count {
            Ok(self.inputs[channel].clone())
        } else {
            Err(MixerError::InputChannelNotFound)
        }
    }

    pub fn get_channel_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn get_channel(&mut self, selector: MixerTrackSelector) -> MixerResult<MixerTrack> {
        match selector {
            MixerTrackSelector::Stereo(l, r) => {
                Ok(MixerTrack::Stereo(
                    self.get_raw_channel(l)?,
                    self.get_raw_channel(r)?
                ))
            }
            MixerTrackSelector::Mono(c) => {
                Ok(MixerTrack::Mono(self.get_raw_channel(c)?))
            },
        }
    }

    /*fn get_stereo_channel(&mut self, l_channel: usize, r_channel: usize) -> PairedMixer {
        let mut left_channel = self.get_channel(l_channel);
        let mut right_channel = self.get_channel(r_channel);

        PairedMixer::Stereo(left_channel, right_channel)
    }*/
}

//type SharedInputMixer = Arc<Mutex<InputMixer>>;

pub type Mixer = (MixerOutputEnd, MixerInputEnd);

type MasterOutputChannelCount = usize;

#[deprecated]
pub type RawInputChannel = Arc<Mutex<HeapProd<f32>>>;

pub struct MixerOutputEnd {
    channel_count: MasterOutputChannelCount,

    buffer_size: usize,
    outputs: Vec<RawOutputMixerTrack>,
}

/// TODO Currently without errors until i figure out how to do this with cpal (threaded vs tokio)
impl MixerOutputEnd {

    // TODO implement error handling - look at inputmixer
    fn get_channel_output_buffer(
        &mut self,
        channel: usize,
    //) -> &mut HeapCons<f32> {
    ) -> MixerResult<RawOutputMixerTrack> {
        if channel < self.channel_count {
            Ok(self.outputs[channel].clone())
            //self.outputs.get_mut(channel).expect("channel not found")
        } else {
            Err(MixerError::OutputChannelNotFound)
        }
    }

    pub fn mixdown(&mut self, output_buffer: &mut [f32]) -> usize {
        let mut ch = 0;

        let mut consumed = 0;
        for o in output_buffer.iter_mut() {
            if let Ok(c) = self.get_channel_output_buffer(ch) {
                let mut c = c.lock().unwrap();

                *o = c.try_pop().unwrap_or(Sample::EQUILIBRIUM);
                consumed += 1;
                
                ch = (ch + 1) % self.channel_count;
            }
        }

        consumed
    }
}

pub fn default_mixer(
    chcount: MasterOutputChannelCount,
    bufsize_per_channel: usize,
) -> Mixer {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    for _ in 0..chcount {
        let buf = ringbuf::HeapRb::<f32>::new(bufsize_per_channel);

        let (prod, cons) = buf.split();
        inputs.push(Arc::new(Mutex::new(prod)));
        outputs.push(Arc::new(std::sync::Mutex::new(cons)));
    }

    debug!(
        "Creating Mixer with {} Channels and bufsize of {} bytes",
        chcount, bufsize_per_channel
    );
    (
        MixerOutputEnd {
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            //inputs,
            outputs,
        },
        MixerInputEnd { inputs, channel_count: chcount },
    )
}


#[cfg(test)]
mod tests {
    use ringbuf::traits::Producer;

    use crate::mixer::default_mixer;

    #[tokio::test]
    async fn test_mixer() {
        let (mut output, mut input) = default_mixer(2, 8);

        for (ch, c) in input.inputs.iter_mut().enumerate() {
            c.lock().await.try_push((ch + 1) as f32).unwrap();
        }

        let mut master_buf = vec![0.0f32; 16];
        output.mixdown(&mut master_buf);

        assert_eq!(&master_buf[..4], vec![1.0, 2.0, 0.0, 0.0]);
    }
}