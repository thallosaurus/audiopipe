use std::fmt::Debug;

use bytemuck::Pod;
use cpal::ChannelCount;
use ringbuf::{traits::{Consumer, Observer}, HeapCons};

//trait InputSampleType: cpal::SizedSample + Send + Pod + Default + Debug + 'static {}

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
    pub fn new(data: &'a [T], selected_channels: Vec<usize>, channel_count: ChannelCount) -> Self {
        ChannelSplitter {
            data,
            selected_channels,
            channel_count,
            index: 0,
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

pub struct ChannelMerger<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    data: &'a mut HeapCons<T>,
    selected_channels: Vec<usize>,
    channel_count: ChannelCount,
    index: usize,
}

impl<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> ChannelMerger<'a, T> {
    pub fn new(data: &'a mut HeapCons<T>, selected_channels: Vec<usize>, channel_count: ChannelCount) -> Self {
        ChannelMerger {
            data,
            selected_channels,
            channel_count,
            index: 0,
        }
    }
}

impl<'a, T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> Iterator for ChannelMerger<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.data.try_pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitter() {
        let test_data: Vec<i32> = vec![1, 2, 3, 4, 5, 6];
        let selected_channels = vec![1];

        let splitter = ChannelSplitter::new(test_data.as_slice(), selected_channels, 2);

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
}
