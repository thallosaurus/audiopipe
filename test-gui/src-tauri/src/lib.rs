use std::{cell::OnceCell, sync::Arc};

use audiopipe_core::{control::server::TcpServer, init_receiver, mixer::MixerTrackSelector};
use once_cell::sync::Lazy;
use tauri::async_runtime::Mutex;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

static RECEIVER: Lazy<Mutex<Option<Arc<TcpServer>>>> = Lazy::new(|| Mutex::new(None));

#[tauri::command]
async fn run_receiver(addr: String, audio_host: String, device: String, buffer_size: usize, samplerate: usize) {
    /*let r = init_receiver(Some(audio_host), Some(device), buffer_size, samplerate, Some(addr), MixerTrackSelector::Stereo(0, 1)).await;

    let mut global = RECEIVER.lock().await;
    *global = Some(Arc::new(r));*/

}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, run_receiver])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
