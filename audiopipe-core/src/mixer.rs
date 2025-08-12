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
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum MixerTrackSelector {
    Mono(usize),
    Stereo(usize, usize),
}

impl MixerTrackSelector {
    pub fn channel_count(&self) -> usize {
        match self {
            MixerTrackSelector::Mono(_) => 1,
            MixerTrackSelector::Stereo(_, _) => 2,
        }
    }

    pub fn contains(&self, ch: usize) -> bool {
        match self {
            MixerTrackSelector::Mono(c) => *c == ch,
            MixerTrackSelector::Stereo(l, r) => *l == ch || *r == ch,
        }
    }
}

impl From<String> for MixerTrackSelector {
    fn from(value: String) -> Self {
        let sp: Vec<&str> = value.split(",").collect();

        if sp.len() == 1 {
            let v: usize = sp.get(0).unwrap().parse().expect("not a valid number");
            Self::Mono(v)
        } else if sp.len() == 2 {
            let l: usize = sp.get(0).unwrap().parse().expect("not a valid number");
            let r: usize = sp.get(1).unwrap().parse().expect("not a valid number");
            Self::Stereo(l, r)
        } else {
            println!("Using default mixer tracks (ch1, ch2)");
            Self::Stereo(0, 1)
        }

        //let from = sp.get(0);
        //let to = sp.get(0);
    }
}

pub type Input = HeapProd<f32>;
pub type Output = HeapCons<f32>;

pub type AsyncRawMixerTrack<I> = Arc<Mutex<I>>;
pub type SyncRawMixerTrack<I> = Arc<std::sync::Mutex<I>>;

#[derive(Debug)]
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
    //debug_writer: Option<DebugWavWriter>
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
        /*let debug_writer = if cfg!(debug_assertions) {
            Some(create_wav_writer("mixer-input".to_string(), chcount, sample_rate).unwrap())
        } else {
            None
        };*/

        Self {
            inputs: tracks,
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            sample_rate,
            //debug_writer
        }
    }

    fn create_reference_input(inner: Input) -> Option<Self::Inner> {
        Some(Arc::new(Mutex::new(inner)))
    }

    fn create_reference_output(_: Output) -> Option<Self::Inner> {
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

    fn create_reference_output(_: Output) -> Option<Self::Inner> {
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

    fn create_reference_input(_: Input) -> Option<Self::Inner> {
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

    fn create_reference_input(_: Input) -> Option<Self::Inner> {
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

/// Syncronously empties the whole mixer and writes it to the output_buffer
pub fn read_from_mixer_sync<M>(
    mixer: &M,
    output_buffer: &mut [f32],
    //sel: MixerTrackSelector,
) -> Result<(usize, usize), MixerError>
where
    M: MixerTrait<Inner = SyncRawMixerTrack<Output>>,
{
    let mut ch = 0;
    let mut consumed = 0;
    let mut dropped = 0;

    for o in output_buffer.iter_mut() {
        let raw_channel = mixer.get_raw_channel(ch % mixer.channel_count())?;
        let mut c = raw_channel.lock().expect("couldn't acquire channel lock");

        if let Some(v) = c.try_pop() {
            *o = v;
            consumed += 1;
        } else {
            *o = Sample::EQUILIBRIUM;
            dropped += 1;
        }
        ch += 1;
    }

    Ok((consumed, dropped))
}

/// TODO Async Implementation for Mixdown
pub async fn read_from_mixer_async<M>(
    mixer: &M,
    output_buffer: &mut [f32],
    sel: MixerTrackSelector,
) -> (usize, usize)
where
    M: MixerTrait<Inner = AsyncRawMixerTrack<Output>>,
{
    let mut consumed = 0;
    let mut dropped = 0;

    if let Ok(mixer) = mixer.get_channel(sel) {
        match mixer {
            MixerTrack::Mono(c) => {
                for o in output_buffer.iter_mut() {
                    if let Some(v) = c.lock().await.try_pop() {
                        *o = v;
                        consumed += 1;
                    } else {
                        *o = Sample::EQUILIBRIUM;
                        dropped += 1;
                    }
                }
            }
            MixerTrack::Stereo(l, r) => {
                let mut ch = 0;
                for o in output_buffer.iter_mut() {
                    let c = if ch & 1 == 0 { l.clone() } else { r.clone() };

                    if let Some(v) = c.lock().await.try_pop() {
                        *o = v;
                        consumed += 1;
                    } else {
                        *o = Sample::EQUILIBRIUM;
                        dropped += 1;
                    }

                    ch = (ch + 1) % sel.channel_count();
                }
            }
        }
    }

    (consumed, dropped)
}

/// TODO Function to transfer a input buffer asynchronously
pub async fn write_to_mixer_async<M>(
    mixer: &M,
    input_buffer: &[f32],
    sel: MixerTrackSelector,
) -> (usize, usize)
where
    M: MixerTrait<Inner = AsyncRawMixerTrack<Input>>,
{
    let mut consumed = 0;
    let mut dropped = 0;

    if let Ok(mixer) = mixer.get_channel(sel) {
        match mixer {
            MixerTrack::Mono(c) => {
                for o in input_buffer.iter() {
                    let mut c = c.lock().await;

                    if let Ok(_) = c.try_push(*o) {
                        consumed += 1;
                    } else {
                        dropped += 1;
                    }

                    //ch = (ch + 1) % sel.channel_count();
                }
            }
            MixerTrack::Stereo(l, r) => {
                let mut ch = 0;
                for o in input_buffer.iter() {
                    let c = if ch & 1 == 0 { l.clone() } else { r.clone() };
                    let mut c = c.lock().await;

                    if let Ok(_) = c.try_push(*o) {
                        consumed += 1;
                    } else {
                        dropped += 1;
                    }

                    ch = (ch + 1) % sel.channel_count();
                }
            }
        }
    }

    (consumed, dropped)
}

/// TODO Function to transfer the CPAL buffer synchronously
pub fn write_to_mixer_sync<M>(
    mixer: &M,
    input_buffer: &[f32],
    sel: MixerTrackSelector,
) -> (usize, usize)
where
    M: MixerTrait<Inner = SyncRawMixerTrack<Input>>,
{
    let mut consumed = 0;
    let mut dropped = 0;
    if let Ok(mixer) = mixer.get_channel(sel) {
        match mixer {
            MixerTrack::Mono(c) => {
                for o in input_buffer.iter() {
                    let mut c = c.lock().expect("couldn't acquire channel lock");

                    if let Ok(_) = c.try_push(*o) {
                        consumed += 1;
                    } else {
                        dropped += 1;
                    }
                }
            }
            MixerTrack::Stereo(l, r) => {
                let mut ch = 0;
                for o in input_buffer.iter() {
                    let c = if ch & 1 == 0 { l.clone() } else { r.clone() };
                    let mut c = c.lock().expect("couldn't acquire channel lock");

                    if let Ok(_) = c.try_push(*o) {
                        consumed += 1;
                    } else {
                        dropped += 1;
                    }

                    ch = (ch + 1) % sel.channel_count();
                }
            }
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
pub(crate) mod tests {

    use crate::mixer::{
        AsyncMixerInputEnd, AsyncMixerOutputEnd, MixerTrackSelector, SyncMixerInputEnd,
        SyncMixerOutputEnd, custom_mixer, read_from_mixer_async, read_from_mixer_sync,
        write_to_mixer_async, write_to_mixer_sync,
    };

    type DebugMixer = (AsyncMixerOutputEnd, AsyncMixerInputEnd);
    type SyncDebugMixer = (SyncMixerOutputEnd, SyncMixerInputEnd);

    pub fn debug_mixer(
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> DebugMixer {
        custom_mixer(chcount, bufsize_per_channel, sample_rate)
    }

    pub fn sync_debug_mixer(
        chcount: usize,
        bufsize_per_channel: usize,
        sample_rate: usize,
    ) -> SyncDebugMixer {
        custom_mixer(chcount, bufsize_per_channel, sample_rate)
    }

    fn mono_test_data(length: usize) -> Vec<f32> {
        vec![0u8; length]
            .iter()
            .enumerate()
            .map(|(i, _)| i as f32)
            .collect()
    }

    fn stereo_test_data(length: usize) -> Vec<f32> {
        vec![0u8; length * 2]
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i & 1 > 0 {
                    // is odd
                    (i - 1) as f32
                } else {
                    i as f32
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn test_mono_mixer_async() {
        let bufsize = 16;

        let (output, input) = debug_mixer(1, 16, 44100);

        let input_data = mono_test_data(16);
        write_to_mixer_async(&input, input_data.as_slice(), MixerTrackSelector::Mono(0)).await;

        let mut output_buffer = vec![0.0f32; bufsize];
        read_from_mixer_async(&output, &mut output_buffer, MixerTrackSelector::Mono(0)).await;

        assert_eq!(input_data, output_buffer);
    }

    #[test]
    fn test_mono_mixer_sync() {
        let bufsize = 16;

        let (output, input) = sync_debug_mixer(1, 16, 44100);

        let input_data = mono_test_data(16);
        write_to_mixer_sync(&input, input_data.as_slice(), MixerTrackSelector::Mono(0));

        let mut output_buffer = vec![0.0f32; bufsize];
        read_from_mixer_sync(&output, &mut output_buffer).unwrap();

        assert_eq!(input_data, output_buffer);
    }

    #[tokio::test]
    async fn test_stereo_mixer_async() {
        let bufsize = 16;

        let (output, input) = debug_mixer(2, bufsize, 44100);

        let input_data = stereo_test_data(bufsize);
        write_to_mixer_async(
            &input,
            input_data.as_slice(),
            MixerTrackSelector::Stereo(0, 1),
        )
        .await;

        // stereo data is twice as long because its two channels
        let mut output_buffer = vec![0.0f32; bufsize * 2];
        read_from_mixer_async(
            &output,
            &mut output_buffer,
            MixerTrackSelector::Stereo(0, 1),
        )
        .await;

        assert_eq!(input_data, output_buffer);
    }

    #[test]
    fn test_stereo_mixer_sync() {
        let bufsize = 16;

        let (output, input) = sync_debug_mixer(2, bufsize, 44100);

        let input_data = stereo_test_data(bufsize);
        write_to_mixer_sync(
            &input,
            input_data.as_slice(),
            MixerTrackSelector::Stereo(0, 1),
        );

        // stereo data is twice as long because its two channels
        let mut output_buffer = vec![0.0f32; bufsize * 2];
        read_from_mixer_sync(&output, &mut output_buffer).unwrap();

        assert_eq!(input_data, output_buffer);
    }

    #[test]
    fn test_eight_channel_mixer_sync() {
        let bufsize = 4;
        let chcount = 4;

        let (output, input) = sync_debug_mixer(16, bufsize, 44100);

        let input_data = stereo_test_data(bufsize * chcount);
        write_to_mixer_sync(
            &input,
            input_data.as_slice(),
            MixerTrackSelector::Stereo(0, 1),
        );

        // stereo data is twice as long because its two channels
        let mut output_buffer = vec![0.0f32; bufsize * chcount];
        read_from_mixer_sync(&output, &mut output_buffer).unwrap();

        assert_eq!(input_data, output_buffer);
    }
}
