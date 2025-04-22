use audio_streamer::{enumerate, receiver, receiver_tui::run_tui};

fn main() {
    enumerate().unwrap();
    let receiver = receiver().unwrap();

    run_tui(receiver.udp_rx, receiver.cpal_rx).unwrap();
}