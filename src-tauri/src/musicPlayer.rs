use std::thread;
use std::sync::{Arc, Mutex};
use crate::{Queue, QUEUE_MANAGER};
use crate::QueueManager; // import the QUEUE


pub fn run(queue_id: String){
    thread::spawn(move || {
        let queue_manager_arc = Arc::clone(&QUEUE_MANAGER);
        let mut queue_manager = queue_manager_arc.lock().unwrap();
        if !queue_manager.queue_exists(&queue_id) {
            queue_manager.create_queue(&queue_id);
        }
        let queue: &mut Queue = queue_manager.get_queue(&queue_id).unwrap();
        if !queue.has_current_song() {
            queue.next_song();
        }
        
        drop(queue_manager);
        loop {
            
            thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}