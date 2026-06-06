use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use ezsockets::CloseFrame;
use ezsockets::Error;
use ezsockets::Server;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use tracing::instrument;

lazy_static! {
    static ref QUEUE_MANAGER: Mutex<QueueManager> = Mutex::new(QueueManager::new());
}

const DEFAULT_QUEUE_ID: &str = "test";

struct Queue {
    current_song: Song,
    old: VecDeque<Song>,
    upcoming: VecDeque<Song>,
}

impl Queue {
    fn new() -> Self {
        Self {
            current_song: Song {
                id: "".to_string(),
                title: "".to_string(),
                artist: "".to_string(),
                album: "".to_string(),
                duration: 0,
                source: None,
                uri: None,
                external_url: None,
                quality: None,
                playable: None,
            },
            old: VecDeque::new(),
            upcoming: VecDeque::new(),
            //shuffeld queues are new queues in the queue manager with the shuffle prefix.
        }
    }

    fn has_current_song(&self) -> bool {
        !self.current_song.id.is_empty()
    }

    fn add_song(&mut self, song: Song) {
        self.upcoming.push_back(song);
    }

    fn remove_song(&mut self, song: Song) {
        self.upcoming.retain(|x| x.id != song.id);
    }

    fn move_upcoming_song(&mut self, from_index: usize, to_index: usize) -> Result<(), String> {
        let length = self.upcoming.len();
        if from_index >= length || to_index >= length {
            return Err(format!(
                "Queue move is out of range: {from_index} to {to_index} for {length} upcoming tracks."
            ));
        }
        if from_index == to_index {
            return Ok(());
        }

        let song = self
            .upcoming
            .remove(from_index)
            .ok_or_else(|| "Queued track could not be moved.".to_string())?;
        self.upcoming.insert(to_index, song);
        Ok(())
    }

    fn next_song(&mut self) {
        if self.upcoming.is_empty() {
            return;
        }
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
        self.current_song = self.old.pop_back().unwrap();
    }

    fn advance_to_song_id(&mut self, song_id: &str) {
        if self.current_song.id == song_id {
            return;
        }

        while let Some(next_song) = self.upcoming.pop_front() {
            if self.has_current_song() {
                self.old.push_back(self.current_song.clone());
            }
            let matched = next_song.id == song_id;
            self.current_song = next_song;
            if matched {
                return;
            }
        }
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
        self.queues.insert(id.to_string(), Queue::new());
    }

    fn get_queue(&mut self, id: &str) -> &mut Queue {
        if !self.queue_exists(id) {
            self.create_queue(id);
        }
        self.queues.get_mut(id).unwrap()
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

    fn move_song_in_queue(
        &mut self,
        id: &str,
        from_index: usize,
        to_index: usize,
    ) -> Result<(), String> {
        self.get_queue(id).move_upcoming_song(from_index, to_index)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: usize,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub external_url: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub playable: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QueueSnapshot {
    pub current_song: Option<Song>,
    pub old: Vec<Song>,
    pub upcoming: Vec<Song>,
}

impl Queue {
    fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            current_song: self.has_current_song().then(|| self.current_song.clone()),
            old: self.old.iter().cloned().collect(),
            upcoming: self.upcoming.iter().cloned().collect(),
        }
    }
}

#[instrument]
pub fn queue_song(song: Song) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.add_song_to_queue(DEFAULT_QUEUE_ID, song);
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn remove_song(song: Song) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.remove_song_from_queue(DEFAULT_QUEUE_ID, song);
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn move_song(from_index: usize, to_index: usize) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.move_song_in_queue(DEFAULT_QUEUE_ID, from_index, to_index)?;
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

pub fn get_queue_snapshot() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn next_queue_song() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    queue.next_song();
    Ok(queue.snapshot())
}

#[instrument]
pub fn previous_queue_song() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    queue.previous_song();
    Ok(queue.snapshot())
}

#[instrument]
pub fn advance_to_song_id(song_id: &str) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    queue.advance_to_song_id(song_id);
    Ok(queue.snapshot())
}

#[instrument]
pub fn play_track_now(song: Song) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);

    if queue.has_current_song() && queue.current_song.id != song.id {
        let current = queue.current_song.clone();
        queue.old.push_back(current);
    }

    queue.upcoming.retain(|s| s.id != song.id);
    queue.current_song = song;

    Ok(queue.snapshot())
}

fn lock_queue_manager() -> Result<MutexGuard<'static, QueueManager>, String> {
    QUEUE_MANAGER
        .lock()
        .map_err(|_| "Queue state is unavailable.".to_string())
}

// Web socket start
type SessionID = u16;
type Session = ezsockets::Session<SessionID, ()>;

//server
struct MusicServer {}
#[async_trait]
impl ezsockets::ServerExt for MusicServer {
    type Session = MusicSession;
    type Call = ();

    async fn on_connect(
        &mut self,
        socket: ezsockets::Socket,
        _request: ezsockets::Request,
        address: SocketAddr,
    ) -> Result<Session, Option<CloseFrame>> {
        let id = address.port();
        let session = Session::create(|handle| MusicSession { id, handle }, id, socket);
        Ok(session)
    }

    async fn on_disconnect(
        &mut self,
        _id: <Self::Session as ezsockets::SessionExt>::ID,
        _reason: Result<Option<CloseFrame>, Error>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn on_call(&mut self, call: Self::Call) -> Result<(), Error> {
        let () = call;
        Ok(())
    }
}

//Session
struct MusicSession {
    handle: Session,
    id: SessionID,
}

#[async_trait]
impl ezsockets::SessionExt for MusicSession {
    type ID = SessionID;
    type Call = ();

    fn id(&self) -> &Self::ID {
        &self.id
    }

    async fn on_text(&mut self, text: String) -> Result<(), Error> {
        //parse the json
        let jason: Value = serde_json::from_str(&text).unwrap();
        println!("Received text: {}", jason);
        //best way to handle rquest like play, pause would be with a if and then a match statement
        if !jason["command"].is_null() && !jason["song"].is_null() {
            let command = jason["command"].as_str().unwrap();
            println!("Command: {}", command);
            if command == "/add" {
                let song = value_to_song(jason["song"].clone());
                let _ = self
                    .handle
                    .text(format!("{} added to queue", song.title))
                    .unwrap();
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                queue_manager.add_song_to_queue(DEFAULT_QUEUE_ID, song);
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/remove" {
                let song = value_to_song(jason["song"].clone());
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                queue_manager.remove_song_from_queue(DEFAULT_QUEUE_ID, song);
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/next" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
                queue.next_song();
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/previous" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
                queue.previous_song();
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/get_queue" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
                let current_song = queue.get_current_song();
                let _ = self
                    .handle
                    .text(
                        json!({
                            "current_song": current_song,
                            "upcoming": queue.upcoming,
                            "old": queue.old
                        })
                        .to_string(),
                    )
                    .unwrap();
                drop(queue_manager);
            } else {
                let _ = self.handle.text("Invalid command").unwrap();
            }
        } else {
            let _ = self.handle.text("Invalid command").unwrap();
        }
        Ok(())
    }

    async fn on_binary(&mut self, _bytes: Vec<u8>) -> Result<(), Error> {
        unimplemented!()
    }

    async fn on_call(&mut self, _call: Self::Call) -> Result<(), Error> {
        Ok(())
    }
}

fn get_queue(queue_manager: &mut MutexGuard<QueueManager>) -> String {
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    let current_song = queue.get_current_song();
    json!({
        "current_song": current_song,
        "upcoming": queue.upcoming,
        "old": queue.old
    })
    .to_string()
}

//WS end :)
pub async fn run() {
    //start a new async thread that does not block the main thread
    tracing_subscriber::fmt::init();
    let (server, _) = Server::create(|_server| MusicServer {});
    ezsockets::tungstenite::run(server, "127.0.0.1:6969")
        .await
        .unwrap();
}

fn value_to_song(value: Value) -> Song {
    let id = value["id"].as_str().unwrap_or("None");
    let title = value["title"].as_str().unwrap_or("NoTitle");
    let artist = value["artist"].as_str().unwrap_or("NoArtist");
    let album = value["album"].as_str().unwrap_or("NoAlbum");
    let duration = value["duration"].as_u64().unwrap_or(0) as usize;
    Song {
        id: id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        duration,
        source: optional_string(&value["source"]),
        uri: optional_string(&value["uri"]),
        external_url: optional_string(&value["external_url"]),
        quality: optional_string(&value["quality"]),
        playable: value["playable"].as_bool(),
    }
}

fn optional_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

//3, 2, 4, 6, 2, 1 >=< 18, 9==4,

#[cfg(test)]
mod tests {
    use super::{Queue, Song};

    fn song(id: &str) -> Song {
        Song {
            id: id.to_string(),
            title: format!("Song {id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration: 100,
            source: None,
            uri: None,
            external_url: None,
            quality: None,
            playable: None,
        }
    }

    #[test]
    fn previous_song_returns_the_immediate_history_item() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));

        queue.next_song();
        queue.next_song();
        queue.previous_song();

        assert_eq!(queue.current_song.id, "1");
        assert_eq!(queue.upcoming.front().unwrap().id, "2");
    }

    #[test]
    fn remove_song_deletes_matching_upcoming_id() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));

        queue.remove_song(song("1"));

        assert_eq!(queue.upcoming.len(), 1);
        assert_eq!(queue.upcoming.front().unwrap().id, "2");
    }

    #[test]
    fn move_upcoming_song_reorders_by_index() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));
        queue.add_song(song("3"));

        queue.move_upcoming_song(2, 0).unwrap();

        let ids = queue
            .upcoming
            .iter()
            .map(|queued_song| queued_song.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["3", "1", "2"]);
    }

    #[test]
    fn move_upcoming_song_rejects_out_of_range_indexes() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));

        let result = queue.move_upcoming_song(0, 1);

        assert!(result.is_err());
        assert_eq!(queue.upcoming.front().unwrap().id, "1");
    }

    #[test]
    fn snapshot_exposes_current_history_and_upcoming() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));
        queue.next_song();

        let snapshot = queue.snapshot();

        assert_eq!(snapshot.current_song.unwrap().id, "1");
        assert!(snapshot.old.is_empty());
        assert_eq!(snapshot.upcoming.len(), 1);
        assert_eq!(snapshot.upcoming[0].id, "2");
    }

    #[test]
    fn advance_to_song_id_moves_upcoming_tracks_into_history() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));
        queue.add_song(song("3"));

        queue.next_song();
        queue.advance_to_song_id("3");

        assert_eq!(queue.current_song.id, "3");
        assert_eq!(
            queue
                .old
                .iter()
                .map(|queued_song| queued_song.id.as_str())
                .collect::<Vec<_>>(),
            vec!["1", "2"]
        );
        assert!(queue.upcoming.is_empty());
    }
}
