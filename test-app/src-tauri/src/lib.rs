use uastreamer::{components::control::TcpControlFlow, config::StreamerConfig, App, AppDebug, Direction};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

struct TestState {
    streamer: App<f32>,
    //streamer_stats: AppDebug
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = StreamerConfig::new(Direction::Receiver, 1024, vec![0, 1], None, None, None, Some(22222)).unwrap();
    let (streamer, streamer_stats) = App::<f32>::new(config.clone());

    let state = TestState {
        streamer,
        //streamer_stats
    };

    //app.serve(config).unwrap();

    //app.pool.join();
    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
