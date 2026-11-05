//! The evaluation worker.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use hactorio_core::Settings;
use hactorio_core::lang::{Evaluation, evaluate};
use hactorio_data::Registry;

/// Owns evaluation off the UI thread. Latest request wins, and a request that
/// is superseded is cancelled rather than run to completion (§2.7).
pub struct Evaluator {
    tx: Sender<Request>,
    rx: Receiver<Response>,
    generation: u64,
    /// Set when the request being evaluated is superseded.
    cancel: Arc<AtomicBool>,
}

struct Request {
    generation: u64,
    text: String,
    settings: Settings,
    registry: Arc<Registry>,
    cancel: Arc<AtomicBool>,
}

pub struct Response {
    pub generation: u64,
    pub evaluation: Evaluation,
}

impl Evaluator {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::channel::<Request>();
        let (responses, rx) = mpsc::channel::<Response>();

        thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let cancel = request.cancel.clone();
                let evaluation =
                    evaluate(&request.text, &request.registry, &request.settings, &|| {
                        cancel.load(Ordering::Relaxed)
                    });
                if evaluation.cancelled {
                    continue;
                }
                if responses
                    .send(Response {
                        generation: request.generation,
                        evaluation,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        Evaluator {
            tx,
            rx,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn request(&mut self, text: String, settings: Settings, registry: Arc<Registry>) -> u64 {
        self.generation += 1;
        // Abandon whatever is running; its result would be stale anyway.
        let previous = std::mem::replace(&mut self.cancel, Arc::new(AtomicBool::new(false)));
        previous.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Request {
            generation: self.generation,
            text,
            settings,
            registry,
            cancel: self.cancel.clone(),
        });
        self.generation
    }

    pub fn poll(&self) -> Option<Response> {
        self.rx.try_recv().ok()
    }
}
