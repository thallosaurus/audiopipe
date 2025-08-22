use std::fmt::Debug;

use bytemuck::Pod;
use cpal::ChannelCount;
use ringbuf::{
    HeapCons,
    traits::Consumer,
};

use crate::mixer::MixerTrackSelector;

#[derive(Debug)]
pub enum SplitterMergerError {
    SelectedChannelsOverChannelCount,
}

impl ToString for SplitterMergerError {
    fn to_string(&self) -> String {
        match self {
            SplitterMergerError::SelectedChannelsOverChannelCount => {
                String::from("more channels selected than there are available on the device")
            }
        }
    }
}

//type InputSampleType: cpal::SizedSample + Send + Pod + Default + Debug + 'static {}

/// Special Iterator over the CPAL Buffer, which returns additional infos about the sample.
/// Is used to work with the interleaved samples
/// For example for two channels (L+R): [L,R,L,R,L,R,...] and so on
/// See [SplitChannelSample]
#[derive(Default)]
pub struct ChannelSplitter<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    data: &'a [T],
    selected_channels: Vec<usize>,
    channel_count: ChannelCount,
    index: usize,
}

impl<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> ChannelSplitter<'a, T> {
    /// Constructs a new, sane ChannelSplitter
    pub fn new(
        data: &'a [T],
        selected_channels: Vec<usize>,
        channel_count: ChannelCount,
    ) -> Result<Self, SplitterMergerError> {
        let mut selection = selected_channels.clone();
        selection.dedup();

        //assert!(selection.len() < channel_count.into());
        if selection.len() > channel_count.into() {
            Err(SplitterMergerError::SelectedChannelsOverChannelCount)
        } else {
            Ok(ChannelSplitter {
                data,
                selected_channels: selection,
                channel_count,
                index: 0,
            })
        }
    }
}

/// Data which gets returned by the ChannelSplitter.
/// Holds a reference to the buffer data
#[derive(Debug)]
pub struct SplitChannelSample<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    pub sample: &'a T,
    pub current_channel: usize,
    pub on_selected_channel: bool,
}
impl<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> Iterator
    for ChannelSplitter<'a, T>
{
    type Item = SplitChannelSample<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let current_channel = self.index % self.channel_count as usize;

        if self.index < self.data.len() {
            let sample = SplitChannelSample {
                sample: &self.data[self.index],
                current_channel,
                on_selected_channel: self.selected_channels.contains(&current_channel),
            };

            self.index += 1;
            Some(sample)
        } else {
            None
        }
    }
}

/// Special Iterator that transforms the contents of a ring buffer to the format needed by CPAL
///
/// Keeps track of the current index and counts up,
/// returns a sample match if the calculated channel matches
/// 
/// It basically is the reverse of [ChannelSplitter]
pub struct ChannelMerger<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    data: &'a mut HeapCons<T>,
    //selected_channels: Vec<usize>,
    selected_channels: MixerTrackSelector,
    channel_count: ChannelCount,
    index: usize,
    output_length: usize,
}

impl<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> ChannelMerger<'a, T> {
    pub fn new(
        data: &'a mut HeapCons<T>,
        //selected_channels: &Vec<usize>,
        selected_channels: MixerTrackSelector,
        channel_count: ChannelCount,
        output_length: usize,
    ) -> Result<Self, SplitterMergerError> {
        let mut selection = selected_channels.clone();
        //selection.dedup();

        //assert!(selection.len() < channel_count.into());
        if selection.channel_count() > channel_count.into() {
            Err(SplitterMergerError::SelectedChannelsOverChannelCount)
        } else {
            Ok(ChannelMerger {
                data,
                selected_channels: selection,
                channel_count,
                output_length,
                index: 0,
            })
        }
    }
}

impl<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> Iterator
    for ChannelMerger<'a, T>
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let current_channel = self.index % self.channel_count as usize;

        if self.index < self.output_length {
            self.index += 1;
            if self.selected_channels.contains(current_channel) {
                self.data.try_pop()
            } else {
                Some(cpal::Sample::EQUILIBRIUM)
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use ringbuf::{
        HeapRb,
        traits::{Producer, Split},
    };

    use super::*;

    #[test]
    fn test_splitter() {
        let test_data: Vec<i32> = vec![1, 2, 3, 4, 5, 6];
        let selected_channels = vec![1];

        let splitter = ChannelSplitter::new(test_data.as_slice(), selected_channels, 2).unwrap();

        let mut output: Vec<SplitChannelSample<'_, i32>> = Vec::new();

        for item in splitter {
            //dbg!(&item);
            output.push(item);
        }

        assert_eq!(
            output
                .iter()
                .map(|e| { e.on_selected_channel })
                .collect::<Vec<bool>>(),
            vec![false, true, false, true, false, true]
        );

        assert_eq!(
            output
                .iter()
                .filter(|e| { e.on_selected_channel })
                .map(|e| { *e.sample })
                .collect::<Vec<i32>>(),
            vec![2, 4, 6]
        );
    }

    #[test]
    fn test_merger() {
        let test_data: Vec<i32> = vec![1, 2, 1, 2, 1, 2];

        let output_data: Vec<i32> = vec![0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0, 2];

        let buf = HeapRb::<i32>::new(test_data.len());
        let (mut prod, mut cons) = buf.split();

        for d in test_data.iter() {
            prod.try_push(*d).unwrap();
        }

        let merger = ChannelMerger::new(&mut cons, MixerTrackSelector::Stereo(1, 3), 4, output_data.len()).unwrap();

        let v: Vec<i32> = merger.into_iter().map(|e| e).collect();
        println!("{:?}", v);

        assert_eq!(v, output_data);
    }

    #[test]
    fn test_invalid_input() {
        let test_data: Vec<i32> = vec![1, 2, 3, 4, 5, 6];
        let selected_channels = vec![1, 2, 3];

        _ = ChannelSplitter::new(test_data.as_slice(), selected_channels, 2);
    }
}