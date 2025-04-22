use audio_streamer::{enumerate, receiver, receiver_tui::run_tui};

fn main() {
    enumerate().unwrap();
    let receiver = receiver().unwrap();

    run_tui(receiver.rx).unwrap();
}