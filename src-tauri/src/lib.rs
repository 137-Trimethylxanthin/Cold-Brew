mod jellyfin;
use serde_json::Value;

use crate::jellyfin::Api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
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





