use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use serde::Serialize;
use async_trait::async_trait;
use ezsockets::CloseFrame;
use ezsockets::Error;
use ezsockets::Server;
use std::net::SocketAddr;
use serde_json::{json, Value};

const INTERVAL: Duration = Duration::from_secs(1);


lazy_static! {
    static ref QUEUE_MANAGER: Mutex<QueueManager> = Mutex::new(QueueManager::new());
}

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
                duration: 0,
            },
            old: VecDeque::new(),
            upcoming: VecDeque::new(),
            //shuffeld queues are new queues in the queue manager with the shuffle prefix.
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
}

#[derive(Clone, Serialize)]
struct Song {
    id: String,
    title: String,
    artist: String,
    album: String,
    duration: usize,
}


// Web socket start
type SessionID = u16;
type Session = ezsockets::Session<SessionID, Message>;


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
        let session = Session::create(
            |handle| {
                let counting_task = tokio::spawn({
                    let session = handle.clone();
                    async move {
                        loop {
                            tokio::time::sleep(INTERVAL).await;
                        }
                    }
                });
                MusicSession {
                    id,
                    handle,
                }
            },
            id,
            socket,
        );
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

#[derive(Debug)]
enum Message {
    getQueue,
}


//Session
struct MusicSession {
    handle: Session,
    id: SessionID,
}

#[async_trait]
impl ezsockets::SessionExt for MusicSession {
    type ID = SessionID;
    type Call = Message;

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
                let _ = self.handle.text(format!("{} added to queue", song.title)).unwrap();
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                queue_manager.add_song_to_queue("test", song);
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/remove" {
                let song = value_to_song(jason["song"].clone());
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                queue_manager.remove_song_from_queue("test", song);
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/next" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue("test");
                queue.next_song();
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/previous" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue("test");
                queue.previous_song();
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/get_queue" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue("test");
                let current_song = queue.get_current_song();
                let _ = self.handle.text(json!({
                    "current_song": current_song,
                    "upcoming": queue.upcoming,
                    "old": queue.old
                }).to_string()).unwrap();
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

    async fn on_call(&mut self, call: Self::Call) -> Result<(), Error> {
        match call {
            _ => {
                let _ = self.handle.text("Invalid command").unwrap();
            }
        };
        Ok(())
    }
}

fn get_queue(queue_manager: &mut MutexGuard<QueueManager>) -> String{
    let queue = queue_manager.get_queue("test");
                let current_song = queue.get_current_song();
                return json!({
                    "current_song": current_song,
                    "upcoming": queue.upcoming,
                    "old": queue.old
                }).to_string();
}


//WS end :)
pub async fn run(){
    //start a new async thread that does not block the main thread
    tracing_subscriber::fmt::init();
    let (server, _) = Server::create(|_server| MusicServer {});
    ezsockets::tungstenite::run(server, "127.0.0.1:6969")
        .await
        .unwrap();
    

}

fn value_to_song(value: Value) -> Song {
    let id = value["id"].as_str().unwrap();
    let title = value["title"].as_str().unwrap();
    let artist = value["artist"].as_str().unwrap();
    let album = value["album"].as_str().unwrap();
    let duration = value["duration"].as_u64().unwrap() as usize;
    Song {
        id: id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        duration,
    }
}



//3, 2, 4, 6, 2, 1 >=< 18, 9==4,