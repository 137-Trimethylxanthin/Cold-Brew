use std::collections::{HashMap, VecDeque};
use serde::Serialize;
use async_trait::async_trait;
use ezsockets::CloseFrame;
use ezsockets::Error;
use ezsockets::Request;
use ezsockets::Server;
use ezsockets::Socket;
use std::net::SocketAddr;


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
type Session = ezsockets::Session<SessionID, ()>;


//server
struct MusicServer {}
#[async_trait]
impl ezsockets::ServerExt for MusicServer {
    type Session = MusicSession;
    type Call = ();

    async fn on_connect(
        &mut self,
        socket: Socket,
        _request: Request,
        address: SocketAddr,
    ) -> Result<Session, Option<CloseFrame>> {
        let id = address.port();
        let session = Session::create(|handle| EchoSession { 
            id,
            handle,
            queue_manager: QueueManager::new(),
            }, id, socket);
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
    queue_manager: QueueManager,
}

#[async_trait]
impl ezsockets::SessionExt for MusicSession {
    type ID = SessionID;
    type Call = ();

    fn id(&self) -> &Self::ID {
        &self.id
    }

    async fn on_text(&mut self, text: String) -> Result<(), Error> {
        //best way to handle rquest like play, pause would be with a if and then a match
        self.handle.text(text).unwrap();
        Ok(())
    }

    async fn on_binary(&mut self, _bytes: Vec<u8>) -> Result<(), Error> {
        unimplemented!()
    }

    async fn on_call(&mut self, call: Self::Call) -> Result<(), Error> {
        let () = call;
        Ok(())
    }
}


//WS end :)
pub async fn run(queue_id: String){
    tracing_subscriber::fmt::init();
    let (server, _) = Server::create(|_server| MusicServer {});
    ezsockets::tungstenite::run(server, "127.0.0.1:8080")
        .await
        .unwrap();
}