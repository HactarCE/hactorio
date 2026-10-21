//! The evaluation worker.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use hactorio_core::{LineCache, Settings, StatementOutput, evaluate};
use hactorio_data::Registry;

/// Owns the line cache off the UI thread. Latest request wins.
pub struct Evaluator {
    tx: Sender<Request>,
    rx: Receiver<Response>,
    generation: u64,
}

struct Request {
    generation: u64,
    text: String,
    settings: Settings,
    registry: Arc<Registry>,
}

pub struct Response {
    pub generation: u64,
    pub outputs: Vec<StatementOutput>,
}

impl Evaluator {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::channel::<Request>();
        let (responses, rx) = mpsc::channel::<Response>();

        thread::spawn(move || {
            let mut cache = LineCache::new();
            let mut registry: Option<Arc<Registry>> = None;
            while let Ok(request) = requests.recv() {
                if registry
                    .as_ref()
                    .is_none_or(|previous| !Arc::ptr_eq(previous, &request.registry))
                {
                    cache.clear();
                    registry = Some(Arc::clone(&request.registry));
                }
                let outputs = evaluate(
                    &request.text,
                    &request.settings,
                    &request.registry,
                    &mut cache,
                );
                if responses
                    .send(Response {
                        generation: request.generation,
                        outputs,
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
        }
    }

    pub fn request(&mut self, text: String, settings: Settings, registry: Arc<Registry>) -> u64 {
        self.generation += 1;
        let _ = self.tx.send(Request {
            generation: self.generation,
            text,
            settings,
            registry,
        });
        self.generation
    }

    pub fn poll(&self) -> Option<Response> {
        self.rx.try_recv().ok()
    }
}
