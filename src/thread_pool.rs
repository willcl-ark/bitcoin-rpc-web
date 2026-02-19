use std::sync::{Arc, Mutex, mpsc};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    Run(Job),
    Shutdown,
}

struct Worker {
    handle: Option<thread::JoinHandle<()>>,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

#[derive(Debug, Clone, Copy)]
pub struct EnqueueError;

impl ThreadPool {
    pub fn new(size: usize) -> Arc<Self> {
        assert!(size > 0, "thread pool size must be greater than zero");

        let (sender, receiver) = mpsc::channel::<Message>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for _ in 0..size {
            let receiver = Arc::clone(&receiver);
            let handle = thread::spawn(move || {
                loop {
                    let message = match receiver.lock().unwrap().recv() {
                        Ok(message) => message,
                        Err(_) => break,
                    };
                    match message {
                        Message::Run(job) => job(),
                        Message::Shutdown => break,
                    }
                }
            });
            workers.push(Worker {
                handle: Some(handle),
            });
        }

        Arc::new(Self { workers, sender })
    }

    pub fn execute<F>(&self, f: F) -> Result<(), EnqueueError>
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender
            .send(Message::Run(Box::new(f)))
            .map_err(|_| EnqueueError)
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        let current_thread_id = thread::current().id();
        for _ in &self.workers {
            let _ = self.sender.send(Message::Shutdown);
        }
        for worker in &mut self.workers {
            if let Some(handle) = worker.handle.take() {
                if handle.thread().id() == current_thread_id {
                    continue;
                }
                let _ = handle.join();
            }
        }
    }
}
