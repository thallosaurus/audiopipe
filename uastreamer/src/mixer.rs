use std::sync::Arc;

use cpal::Sample;
use log::debug;
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Consumer, Split},
};
use tokio::sync::Mutex;

pub enum MixerTrackSelector {
    Mono(usize),
    Stereo(usize, usize),
}

/// Paired Mixer Track for processing
/*pub enum MixerTrack {
    Mono(AsyncRawInputMixerTrack),
    Stereo(AsyncRawInputMixerTrack, AsyncRawInputMixerTrack),
}*/

//type Cpal<T> = Arc<std::sync::Mutex<T>>;

pub type Input = HeapProd<f32>;
pub type Output = HeapCons<f32>;

pub type AsyncRawMixerTrack<I> = Arc<Mutex<I>>;
pub type SyncRawMixerTrack<O> = Arc<std::sync::Mutex<O>>;

pub enum MixerError {
    InputChannelNotFound,
    OutputChannelNotFound,
}

type MixerResult<T> = Result<T, MixerError>;

/// The Input Side of the Mixer
#[derive(Clone)]
pub struct AsyncMixerInputEnd {
    inputs: Vec<Arc<Mutex<Input>>>,
    channel_count: usize,
}

impl MixerTrait for AsyncMixerInputEnd {
    type Inner = AsyncRawMixerTrack<Input>;
    fn tracks(&mut self) -> Vec<Self::Inner> {
        self.inputs.clone()
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }
}

//type SharedInputMixer = Arc<Mutex<InputMixer>>;

/// On the receiver side (server side), the cpal stream lies in a sync thread and the mixer in the tokio runtime
pub type ServerMixer = (SyncMixerOutputEnd, AsyncMixerInputEnd);

pub struct SyncMixerOutputEnd {
    channel_count: usize,
    buffer_size: usize,
    outputs: Vec<Arc<std::sync::Mutex<HeapCons<f32>>>>,
}

impl MixerTrait for SyncMixerOutputEnd {
    fn tracks(&mut self) -> Vec<Self::Inner> {
        self.outputs.clone()
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }
    
    type Inner = SyncRawMixerTrack<Output>;
}

pub enum MixerTrack<T> {
    Mono(T),
    Stereo(T, T),
}

/// Writes the
pub fn mixdown<M>(mixer: &mut M, output_buffer: &mut [f32]) -> usize
where
    M: MixerTrait<Inner = SyncRawMixerTrack<Output>>
{
    let mut ch = 0;

    let mut consumed = 0;
    for o in output_buffer.iter_mut() {
        if let Ok(c) = mixer.get_raw_channel(ch) {
            let mut c = c.lock().unwrap();

            *o = c.try_pop().unwrap_or(Sample::EQUILIBRIUM);
            consumed += 1;

            ch = (ch + 1) % mixer.channel_count();
        }
    }

    consumed
}

pub fn default_server_mixer(chcount: usize, bufsize_per_channel: usize) -> ServerMixer {
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
        SyncMixerOutputEnd {
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            //inputs,
            outputs,
        },
        AsyncMixerInputEnd {
            inputs,
            channel_count: chcount,
        },
    )
}

pub trait MixerTrait {
    type Inner: Clone;
    fn tracks(&mut self) -> Vec<Self::Inner>;

    fn channel_count(&self) -> usize;

    //impl MixerInputEnd {
    fn get_raw_channel(&mut self, channel: usize) -> MixerResult<Self::Inner> {
        if channel < self.channel_count() {
            Ok(self.tracks()[channel].clone())
        } else {
            Err(MixerError::InputChannelNotFound)
        }
    }

    /*fn get_channel_count(&self) -> usize {
        self.tracks().len()
    }*/

    fn get_channel(&mut self, selector: MixerTrackSelector) -> MixerResult<MixerTrack<Self::Inner>> {
        match selector {
            MixerTrackSelector::Stereo(l, r) => Ok(MixerTrack::Stereo(
                self.get_raw_channel(l)?,
                self.get_raw_channel(r)?,
            )),
            MixerTrackSelector::Mono(c) => Ok(MixerTrack::Mono(self.get_raw_channel(c)?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use ringbuf::traits::Producer;

    use crate::mixer::{default_server_mixer, mixdown, MixerTrait};

    #[tokio::test]
    async fn test_mixer() {
        let (mut output, mut input) = default_server_mixer(2, 8);

        for (ch, c) in input.inputs.iter_mut().enumerate() {
            c.lock().await.try_push((ch + 1) as f32).unwrap();
        }

        let mut master_buf = vec![0.0f32; 16];
        mixdown(&mut output, &mut master_buf);
        //output.mixdown(&mut master_buf);

        assert_eq!(&master_buf[..4], vec![1.0, 2.0, 0.0, 0.0]);
    }
}
