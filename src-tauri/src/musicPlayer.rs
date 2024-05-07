use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use serde::Serialize;
use async_trait::async_trait;
use ezsockets::CloseFrame;
use ezsockets::Error;
use ezsockets::Server;
use std::net::SocketAddr;
use serde_json::Value;

const INTERVAL: Duration = Duration::from_secs(1);

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
                            let _ = session.call(Message::Next).unwrap();
                            tokio::time::sleep(INTERVAL).await;
                        }
                    }
                });
                MusicSession {
                    id,
                    handle,
                    queue_manager: QueueManager::new(),
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
    Play,
    Pause,
    Next,
    Previous,
    Add(String),
    Remove,
}


//Session
struct MusicSession {
    handle: Session,
    id: SessionID,
    queue_manager: QueueManager,
}

#[async_trait]
impl ezsockets::SessionExt for MusicSession {
    type ID = SessionID;
    type Call = Message;

    fn id(&self) -> &Self::ID {
        &self.id
    }

    async fn on_text(&mut self, _text: String) -> Result<(), Error> {
        //best way to handle rquest like play, pause would be with a if and then a match
        self.handle.text("Invalid command").unwrap();
        Ok(())
    }

    async fn on_binary(&mut self, _bytes: Vec<u8>) -> Result<(), Error> {
        unimplemented!()
    }

    async fn on_call(&mut self, call: Self::Call) -> Result<(), Error> {
         match call {
            Message::Add(string) => {
                println!("Adding song to queue");
                println!("{}", string);
                let res: Value = serde_json::from_str(string.as_str()).unwrap();
                let song = Song {
                    id: res["id"].as_str().unwrap().to_string(),
                    title: res["title"].as_str().unwrap().to_string(),
                    artist: res["artist"].as_str().unwrap().to_string(),
                    album: res["album"].as_str().unwrap().to_string(),
                    duration: res["duration"].as_u64().unwrap() as usize,
                };
                self.queue_manager.add_song_to_queue("default", song);
            }
            Message::Next => {
                println!("Next song");
                let queue = self.queue_manager.get_queue("default").unwrap();
                if !queue.has_current_song() {
                    queue.next_song();
                }
            }
            _ => {
                let _ = self.handle.text("Invalid command").unwrap();
            }
        };
        Ok(())
    }
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