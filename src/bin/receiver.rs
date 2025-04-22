use audio_streamer::{enumerate, receiver, receiver_tui::run_tui};

fn main() {
    enumerate().unwrap();
    let rx_stats = receiver().unwrap();

    run_tui(rx_stats).unwrap();
}