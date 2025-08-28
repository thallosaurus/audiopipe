use std::sync::Arc;

use audiopipe_core::{control::server::TcpServer, init_receiver, setup_cpal_output};
use once_cell::sync::Lazy;
use tauri::async_runtime::Mutex;

static RECEIVER: Lazy<Mutex<Option<Arc<TcpServer>>>> = Lazy::new(|| Mutex::new(None));

#[tauri::command]
fn run_receiver(state: tauri::State<MyState>, bsize: usize, srate: usize) -> Result<(), ()> {
    let (output_device, sconfig) = setup_cpal_output(None, None, bsize, srate);
    Ok(())
}

struct MyState {
    receiver: Arc<Mutex<Option<TcpServer>>>
    //sender: Arc<Mutex<Option<TcpClient>>>
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    .manage(MyState { receiver: Arc::new(Mutex::new(None)) })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![run_receiver])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
