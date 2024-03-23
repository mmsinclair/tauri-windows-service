// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[derive(Clone, serde::Serialize)]
struct Payload {
  message: String,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn get_version() -> String {
    format!("v{}", VERSION)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
