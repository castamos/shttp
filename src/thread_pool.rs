use std::thread;
use std::sync::{
    mpsc,   // Multiple Producer Single Consumer channel
    Arc,    // Atomic Reference Counter
    Mutex,
};

use log::{debug, trace};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {

    /// Create a new `ThreadPool`.
    /// - `size` is the number of threads in the pool
    ///
    /// # Panics
    /// When passed a zero value to `size`.
    ///
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender: Some(sender) }
    }

    /// Executes the given job `f` in the pool's next available thread.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
        // add `callback` to queue.
    }
}


impl Drop for ThreadPool {

    fn drop(&mut self) {
        drop( self.sender.take() );

        for worker in &mut self.workers {
            debug!("Shutting down worker {} ...", worker.id);
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}


struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {

    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    trace!("Worker {id} got a job; executing ...");
                    job();
                    trace!("Worker {id} done executing job.");
                },
                Err(_) => {
                    trace!("Worker {id} exiting (sender closed).");
                    break;
                }
            }
        });
        Worker { id, thread: Some(thread) }
    }
}

