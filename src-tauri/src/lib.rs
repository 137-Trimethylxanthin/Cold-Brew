mod jellyfin;
mod musicPlayer;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use serde::Serialize;
#[macro_use]
extern crate lazy_static;


use crate::jellyfin::Api;


struct Queue {
    id: String,
    current_song: Song,
    old: VecDeque<Song>,
    upcoming: VecDeque<Song>,
}


impl Queue {
    fn new(id: String) -> Self {
        Self {
            id,
            current_song: Song {
                id: "".to_string(),
                title: "".to_string(),
                artist: "".to_string(),
                album: "".to_string(),
                duration: "".to_string(),
            },
            old: VecDeque::new(),
            upcoming: VecDeque::new(),
        }
    }

    fn has_current_song(&self) -> bool {
        self.current_song.id != ""
    }

    fn add_song(&mut self, song: Song) {
        self.upcoming.push_back(song);
    }

    fn remove_song(&mut self, song: Song) {
        self.upcoming.retain(|x| x.id != song.id);
    }


    fn next_song(&mut self) {
        if self.has_current_song() {
            self.old.push_back(self.current_song.clone());
        }
        self.current_song = self.upcoming.pop_front().unwrap();
    }

    fn get_current_song(&self) -> Song {
        self.current_song.clone()
    }

    fn previous_song(&mut self) {
        if self.old.is_empty() {
            return;
        }
        self.upcoming.push_front(self.current_song.clone());
        self.current_song = self.old.pop_front().unwrap();
    }

}

struct QueueManager {
    queues: HashMap<String, Queue>,
}

impl QueueManager {
    fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }

    fn queue_exists(&self, id: &str) -> bool {
        self.queues.contains_key(id)
    }

    fn create_queue(&mut self, id: &str) {
        self.queues.insert(id.to_string(), Queue::new(id.to_string()));
    }

    fn get_queue(&mut self, id: &str) -> Option<&mut Queue> {
        if self.queue_exists(id) {
            Some(self.queues.get_mut(id).unwrap())
        } else {
            None
        }
    }

    fn add_song_to_queue(&mut self, id: &str, song: Song) {
        if !self.queue_exists(id) {
            self.create_queue(id);
        }
        self.queues.get_mut(id).unwrap().add_song(song);
    }

    fn remove_song_from_queue(&mut self, id: &str, song: Song) {
        if self.queue_exists(id) {
            self.queues.get_mut(id).unwrap().remove_song(song);
        }
    }
}

#[derive(Clone, Serialize)]
struct Song {
    id: String,
    title: String,
    artist: String,
    album: String,
    duration: String,
}


lazy_static! {
    static ref QUEUE_MANAGER: Arc<Mutex<QueueManager>> = Arc::new(Mutex::new(QueueManager::new()));
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    musicPlayer::run("test".to_string());
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![display_song_list, add_song, remove_song, get_current_song, old_queue, upcoming_queue])
    .run(tauri::generate_context!(  ))
    .expect("error while running tauri application");
}

#[tauri::command(rename_all = "snake_case")]
async fn display_song_list() -> Value{
    let api = Api::new("https://jelly.plskill.me".to_string(), "maxi".to_string(), "gNtFiFglCNiNejFFRgfGDvJIuTCvENbRdunGnE".to_string());
    let songs = api.await.get_all_songs().await.unwrap();
    songs["Items"].clone()
}


#[tauri::command(rename_all = "snake_case")]
fn add_song(song: Value, id:&str) {
    let song: Song = Song {
        id: song["Id"].to_string(),
        title: song["Name"].to_string(),
        artist: song["Artists"][0]["Name"].to_string(),
        album: song["Album"].to_string(),
        duration: song["RunTimeTicks"].to_string(),
    };
    let mut queue_manager = QUEUE_MANAGER.lock().unwrap();

    queue_manager.add_song_to_queue(id, song);
}

#[tauri::command(rename_all = "snake_case")]
fn remove_song(song: Value, id:&str) {
    let song: Song = Song {
        id: song["Id"].to_string(),
        title: song["Name"].to_string(),
        artist: song["Artists"][0]["Name"].to_string(),
        album: song["Album"].to_string(),
        duration: song["RunTimeTicks"].to_string(),
    };
    let mut queue_manager = QUEUE_MANAGER.lock().unwrap();

    queue_manager.remove_song_from_queue(id, song);
}

#[tauri::command(rename_all = "snake_case")]
fn get_current_song(id:&str) -> Value {
    let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
    if !queue_manager.queue_exists(id) {
        queue_manager.create_queue(id);
    }
    let queue = queue_manager.get_queue(id).unwrap();
    let song = queue.get_current_song();
    serde_json::json!(song)
}

#[tauri::command(rename_all = "snake_case")]
fn old_queue(id:&str) -> Value {
    let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
    let queue = queue_manager.get_queue(id).unwrap();
    let old = queue.old.clone();
    serde_json::json!(old)
}

#[tauri::command(rename_all = "snake_case")]
fn upcoming_queue(id:&str) -> Value {
    let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
    if !queue_manager.queue_exists(id) {
        queue_manager.create_queue(id);
    }
    let queue = queue_manager.get_queue(id).unwrap();
    let upcoming = queue.upcoming.clone();
    serde_json::json!(upcoming)
}







