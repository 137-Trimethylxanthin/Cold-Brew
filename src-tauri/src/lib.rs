mod jellyfin;
mod musicPlayer;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use serde::Serialize;
#[macro_use]
extern crate lazy_static;

use crate::jellyfin::Api;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|_app| {
      tauri::async_runtime::spawn(async move {
        musicPlayer::run().await;
      });
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![display_song_list])
    .run(tauri::generate_context!(  ))
    .expect("error while running tauri application");
}

#[tauri::command(rename_all = "snake_case")]
async fn display_song_list() -> Value{
    let api = Api::new("https://jelly.plskill.me".to_string(), "maxi".to_string(), "gNtFiFglCNiNejFFRgfGDvJIuTCvENbRdunGnE".to_string());
    let songs = api.await.get_all_songs().await.unwrap();
    songs["Items"].clone()
}

