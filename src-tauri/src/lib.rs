mod jellyfin;
use crate::jellyfin::Api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![display_song_list])
    .run(tauri::generate_context!(  ))
    .expect("error while running tauri application");
}

#[tauri::command]
async fn display_song_list() {
    let api = Api::new("https://jelly.plskill.me".to_string(), "maxi".to_string(), "gNtFiFglCNiNejFFRgfGDvJIuTCvENbRdunGnE".to_string());
    let songs = api.await.get_all_songs().await.unwrap();
    print!("{}", songs)
}





