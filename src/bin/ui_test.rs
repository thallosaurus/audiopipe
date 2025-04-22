use std::{sync::mpsc, time::Duration};

use audio_streamer::{enumerate, receiver, receiver_tui::run_tui, UdpStats};
use rand::prelude::*;

fn main() {
    //enumerate().unwrap();
    //let rx_stats = receiver().unwrap();

    let (tx_stats, rx_stats) = mpsc::channel();

    std::thread::spawn(move || {
        let mut rng = rand::rng();
        loop {
            let rnd1 = rng.random::<u16>();
            let rnd2 = rng.random::<u16>();
            
            let stat = UdpStats {
                received: rnd1.into(),
                occupied_buffer: rnd2.into(),
            };
            tx_stats.send(stat).unwrap();
            std::thread::sleep(Duration::from_millis(250));
        }
    });

    run_tui(rx_stats).unwrap();
}