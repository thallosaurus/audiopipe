use std::sync::Arc;

use cpal::Sample;
use log::debug;
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Consumer, Producer, Split},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Input enum which selects one or two channels together
#[derive(Serialize, Deserialize, Debug)]
pub enum MixerTrackSelector {
    Mono(usize),
    Stereo(usize, usize),
}

pub type Input = HeapProd<f32>;
pub type Output = HeapCons<f32>;

pub type AsyncRawMixerTrack<I> = Arc<Mutex<I>>;
pub type SyncRawMixerTrack<I> = Arc<std::sync::Mutex<I>>;

pub enum MixerError {
    InputChannelNotFound,
    OutputChannelNotFound,
}

type MixerResult<T> = Result<T, MixerError>;

/// On the receiver side (server side), the mixer output must be non-async
pub type ServerMixer = (SyncMixerOutputEnd, AsyncMixerInputEnd);

/// On the sender side (client side), the mixer input must be non-async
pub type ClientMixer = (AsyncMixerOutputEnd, SyncMixerInputEnd);

/// Mixer Input that lies in an async context
pub struct AsyncMixerInputEnd {
    inputs: Vec<Arc<Mutex<Input>>>,
    channel_count: usize,
    buffer_size: usize,
    sample_rate: usize,
}

impl MixerTrait for AsyncMixerInputEnd {
    type Inner = AsyncRawMixerTrack<Input>;
    fn tracks(&self) -> Vec<Self::Inner> {
        self.inputs.clone()
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn default(
        tracks: Vec<Self::Inner>,
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            inputs: tracks,
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            sample_rate,
        }
    }

    fn create_reference_input(inner: Input) -> Option<Self::Inner> {
        Some(Arc::new(Mutex::new(inner)))
    }

    fn create_reference_output(inner: Output) -> Option<Self::Inner> {
        None
    }
}

/// Mixer Input, that lies in a sync context
pub struct SyncMixerInputEnd {
    inputs: Vec<Arc<std::sync::Mutex<Input>>>,
    channel_count: usize,
    buffer_size: usize,
    sample_rate: usize,
}

impl MixerTrait for SyncMixerInputEnd {
    type Inner = SyncRawMixerTrack<Input>;

    fn tracks(&self) -> Vec<Self::Inner> {
        self.inputs.clone()
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn default(
        tracks: Vec<Self::Inner>,
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            inputs: tracks,
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            sample_rate,
        }
    }

    fn create_reference_input(inner: Input) -> Option<Self::Inner> {
        Some(Arc::new(std::sync::Mutex::new(inner)))
    }

    fn create_reference_output(inner: Output) -> Option<Self::Inner> {
        None
    }
}

/// Mixer Output that lies in an async context
pub struct AsyncMixerOutputEnd {
    channel_count: usize,
    outputs: Vec<Arc<Mutex<HeapCons<f32>>>>,
    buffer_size: usize,
    sample_rate: usize,
}

impl MixerTrait for AsyncMixerOutputEnd {
    type Inner = AsyncRawMixerTrack<Output>;

    fn tracks(&self) -> Vec<Self::Inner> {
        self.outputs.clone()
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn default(
        tracks: Vec<Self::Inner>,
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            channel_count: chcount,
            outputs: tracks,
            buffer_size: bufsize_per_channel,
            sample_rate,
        }
    }

    fn create_reference_input(inner: Input) -> Option<Self::Inner> {
        None
    }

    fn create_reference_output(inner: Output) -> Option<Self::Inner> {
        Some(Arc::new(Mutex::new(inner)))
    }
}

/// Mixer Output that lies in a sync context
pub struct SyncMixerOutputEnd {
    channel_count: usize,
    buffer_size: usize,
    outputs: Vec<Arc<std::sync::Mutex<HeapCons<f32>>>>,
    sample_rate: usize,
}

impl MixerTrait for SyncMixerOutputEnd {
    fn tracks(&self) -> Vec<Self::Inner> {
        self.outputs.clone()
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }

    type Inner = SyncRawMixerTrack<Output>;

    fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn default(
        tracks: Vec<Self::Inner>,
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            outputs: tracks,
            sample_rate,
        }
    }

    fn create_reference_input(inner: Input) -> Option<Self::Inner> {
        None
    }

    fn create_reference_output(inner: Output) -> Option<Self::Inner> {
        Some(Arc::new(std::sync::Mutex::new(inner)))
    }
}

pub enum MixerTrack<T> {
    Mono(T),
    Stereo(T, T),
}

/// Sync Implementation for Mixdown
pub fn mixdown_sync<M>(mixer: &M, output_buffer: &mut [f32]) -> usize
where
    M: MixerTrait<Inner = SyncRawMixerTrack<Output>>,
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

/// Async Implementation for Mixdown
pub async fn mixdown_async<M>(mixer: &M, output_buffer: &mut [f32]) -> usize
where
    M: MixerTrait<Inner = AsyncRawMixerTrack<Output>>,
{
    let mut ch = 0;

    let mut consumed = 0;
    for o in output_buffer.iter_mut() {
        if let Ok(c) = mixer.get_raw_channel(ch) {
            let mut c = c.lock().await;

            *o = c.try_pop().unwrap_or(Sample::EQUILIBRIUM);
            consumed += 1;

            ch = (ch + 1) % mixer.channel_count();
        }
    }

    consumed
}

/// Function to transfer a input buffer asynchronously
pub async fn transfer_async<M>(mixer: &M, input_buffer: &[f32]) -> (usize, usize)
where
    M: MixerTrait<Inner = AsyncRawMixerTrack<Input>>,
{
    let mut ch = 0;

    let mut consumed = 0;
    let mut dropped = 0;
    for o in input_buffer.iter() {
        if let Ok(c) = mixer.get_raw_channel(ch) {
            let mut c = c.lock().await;

            if let Ok(_) = c.try_push(*o) {
                consumed += 1;
            } else {
                dropped += 1;
            }

            ch = (ch + 1) % mixer.channel_count();
        }
    }

    (consumed, dropped)
}

/// Function to transfer the CPAL buffer synchronously
pub fn transfer_sync<M>(mixer: &M, input_buffer: &[f32]) -> (usize, usize)
where
    M: MixerTrait<Inner = SyncRawMixerTrack<Input>>,
{
    let mut ch = 0;

    let mut consumed = 0;
    let mut dropped = 0;
    for o in input_buffer.iter() {
        if let Ok(c) = mixer.get_raw_channel(ch) {
            let mut c = c.lock().expect("failed to open mixer");

            if let Ok(_) = c.try_push(*o) {
                consumed += 1;
            } else {
                dropped += 1;
            }
            //            *o = c.try_pop().unwrap_or(Sample::EQUILIBRIUM);

            ch = (ch + 1) % mixer.channel_count();
        }
    }

    (consumed, dropped)
}

/// Constructs a default server mixer with sync mixer output and async mixer input
pub fn default_server_mixer(
    chcount: usize,
    bufsize_per_channel: usize,
    sample_rate: usize,
) -> ServerMixer {
    custom_mixer(chcount, bufsize_per_channel, sample_rate)
}

/// Constructs a default client mixer with async mixer output and sync mixer input
pub fn default_client_mixer(
    chcount: usize,
    bufsize_per_channel: usize,
    sample_rate: usize,
) -> ClientMixer {
    custom_mixer(chcount, bufsize_per_channel, sample_rate)
}

fn custom_mixer<I, O>(chcount: usize, bufsize_per_channel: usize, sample_rate: usize) -> (O, I)
where
    I: MixerTrait,
    O: MixerTrait,
{
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    for _ in 0..chcount {
        let buf = ringbuf::HeapRb::<f32>::new(bufsize_per_channel);

        let (prod, cons) = buf.split();
        inputs.push(I::create_reference_input(prod).expect("not an input type"));
        outputs.push(O::create_reference_output(cons).expect("not an output type"));
    }

    debug!(
        "Creating Mixer with {} Channels and bufsize of {} bytes",
        chcount, bufsize_per_channel
    );
    (
        O::default(outputs, chcount, bufsize_per_channel, sample_rate),
        I::default(inputs, chcount, bufsize_per_channel, sample_rate),
    )
}

pub trait MixerTrait {
    type Inner: Clone;
    fn default(
        tracks: Vec<Self::Inner>,
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> Self;
    fn create_reference_input(inner: Input) -> Option<Self::Inner>;
    fn create_reference_output(inner: Output) -> Option<Self::Inner>;
    fn tracks(&self) -> Vec<Self::Inner>;

    fn channel_count(&self) -> usize;

    fn buffer_size(&self) -> usize;

    fn sample_rate(&self) -> usize;

    //impl MixerInputEnd {
    fn get_raw_channel(&self, channel: usize) -> MixerResult<Self::Inner> {
        if channel < self.channel_count() {
            Ok(self.tracks()[channel].clone())
        } else {
            Err(MixerError::InputChannelNotFound)
        }
    }

    fn get_channel(&self, selector: MixerTrackSelector) -> MixerResult<MixerTrack<Self::Inner>> {
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

    use crate::mixer::{default_server_mixer, mixdown_sync};

    #[tokio::test]
    async fn test_mixer() {
        let (mut output, mut input) = default_server_mixer(2, 8, 44100);

        for (ch, c) in input.inputs.iter_mut().enumerate() {
            c.lock().await.try_push((ch + 1) as f32).unwrap();
        }

        let mut master_buf = vec![0.0f32; 16];
        mixdown_sync(&mut output, &mut master_buf);
        //output.mixdown(&mut master_buf);

        assert_eq!(&master_buf[..4], vec![1.0, 2.0, 0.0, 0.0]);
    }

    #[tokio::test]
    async fn test_mixer_ends() {
        let channels = 16;
        {
            // TODO add tests here
        }
    }
}
