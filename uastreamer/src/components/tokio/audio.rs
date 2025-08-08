use cpal::{
    BuildStreamError, Device, InputCallbackInfo, Sample, Stream, StreamConfig, traits::DeviceTrait,
};
use log::{trace, warn};
use once_cell::sync::Lazy;
use ringbuf::{traits::{Producer, Split}, HeapCons, HeapProd, HeapRb};
use tokio::sync::Mutex;

use crate::splitter::{ChannelMerger, ChannelSplitter};

// TODO: Replace with the master mixer
pub static GLOBAL_MASTER_OUTPUT: Lazy<Mutex<Option<HeapProd<f32>>>> = Lazy::new(|| Mutex::new(None));
pub static GLOBAL_MASTER_INPUT: Lazy<Mutex<Option<HeapCons<f32>>>> = Lazy::new(|| Mutex::new(None));

pub async fn setup_master_output(
    device: Device,
    config: StreamConfig,
    bsize: usize,
    selected_channels: Vec<usize>,
) -> Result<Stream, BuildStreamError> {
    let rbuf = HeapRb::<f32>::new(bsize * config.channels as usize);
    let (prod, mut cons) = rbuf.split();

    let mut master_out = GLOBAL_MASTER_OUTPUT.lock().await;
    *master_out = Some(prod);

    device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            //trace!("Callback wants {:?} ", data.len());

            let mut consumed = 0;

            let mut merger =
                ChannelMerger::new(&mut cons, &selected_channels, config.channels, data.len())
                    .unwrap();
            //cons.read(buf)
            for sample in data.iter_mut() {
                let s = merger.next();
                if let Some(s) = s {
                    if !cfg!(test) {
                        *sample = s;
                        //write_debug(writer, *sample);
                    } else {
                        *sample = Sample::EQUILIBRIUM;
                    }
                }

                consumed += 1;
            }
        },
        move |err| {},
        None,
    )
}

pub async fn setup_master_input(
    device: Device,
    config: &StreamConfig,
    bsize: usize,
    selected_channels: Vec<usize>,
) -> Result<Stream, BuildStreamError> {
    let rbuf = HeapRb::<f32>::new(bsize * config.channels as usize);
    let (mut prod, cons) = rbuf.split();

    let mut master_in = GLOBAL_MASTER_INPUT.lock().await;
    *master_in = Some(cons);

    let chcount = config.channels;

    device.build_input_stream(
        &config,
        move |data: &[f32], _: &InputCallbackInfo| {
            let splitter = ChannelSplitter::new(
                data,
                //streamer_config.selected_channels.clone(),
                selected_channels.clone(),
                chcount,
            )
            .unwrap();

            let mut dropped = 0;
            // Iterate through the input buffer and save data
            for s in splitter {
                if s.on_selected_channel {
                    if !cfg!(test) {
                        if let Ok(()) = prod.try_push(*s.sample) {
                            //write_debug(writer, *s.sample);
                        } else {
                            dropped += 1;
                            //udp_urge_channel.send(true).unwrap();

                            // Urge the UDP thread to send the buffer immediately
                            //udp_urge_channel.send(true).unwrap();

                            // drop remaining samples
                            //break;
                            //return consumed
                        }
                        //output.try_push(*s.sample).unwrap();
                    } else {
                        prod.try_push(Sample::EQUILIBRIUM).unwrap();
                    }
                }
            }
            if dropped > 0 {
                warn!("OVERFLOW - Dropped {} Samples", dropped);
            }
            //udp_urge_channel.send(true).unwrap();
        },
        move |err| {},
        None,
    )
}
