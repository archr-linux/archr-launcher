//! One serialized writer, with only the latest not-yet-started snapshot retained.
use crate::store::Store;
use std::{
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Condvar, Mutex,
    },
};
type Saved = Result<(), String>;
struct Save {
    store: Store,
    ack: Option<Sender<Saved>>,
}
#[derive(Default)]
struct Slot {
    pending: Option<Save>,
    stop: bool,
}
#[derive(Default)]
struct Shared {
    slot: Mutex<Slot>,
    wake: Condvar,
}
pub(crate) struct StoreWriter {
    shared: Arc<Shared>,
    errors: Receiver<String>,
}
impl StoreWriter {
    pub fn new(dir: PathBuf) -> Self {
        Self::spawn(move |store| {
            // A preserved invalid Store is read-only, not a failed transition.
            if store.load_error.is_some() {
                return Ok(());
            }
            store.save(&dir).map_err(|error| error.to_string())
        })
    }
    fn spawn(mut write: impl FnMut(&Store) -> Saved + Send + 'static) -> Self {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        let (errors, received) = mpsc::channel();
        std::thread::spawn(move || loop {
            let save = {
                let mut slot = worker.slot.lock().unwrap();
                while slot.pending.is_none() && !slot.stop {
                    slot = worker.wake.wait(slot).unwrap();
                }
                match slot.pending.take() {
                    Some(save) => save,
                    None => return,
                }
            };
            // Never hold the slot lock during serialization or filesystem I/O.
            let result = write(&save.store);
            if let Err(error) = &result {
                let _ = errors.send(error.clone());
            }
            if let Some(ack) = save.ack {
                let _ = ack.send(result);
            }
        });
        Self {
            shared,
            errors: received,
        }
    }
    pub fn queue(&self, store: Store) {
        self.submit(Save { store, ack: None });
    }
    fn submit(&self, save: Save) {
        self.shared.slot.lock().unwrap().pending = Some(save);
        self.shared.wake.notify_one();
    }
    /// Exit-only barrier: the final snapshot follows any in-flight older write.
    /// No queued older snapshot can overwrite the final durable state afterward.
    pub fn flush(&self, store: Store) -> Saved {
        let (ack, saved) = mpsc::channel();
        self.submit(Save {
            store,
            ack: Some(ack),
        });
        saved.recv().map_err(|error| error.to_string())?
    }
    pub fn error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }
}
impl Drop for StoreWriter {
    fn drop(&mut self) {
        self.shared.slot.lock().unwrap().stop = true;
        self.shared.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};
    #[test]
    fn queued_saves_do_not_wait_for_io_and_pending_snapshots_coalesce() {
        let (started, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let written = Arc::new(Mutex::new(Vec::new()));
        let output = written.clone();
        let mut first = true;
        let writer = StoreWriter::spawn(move |store| {
            if first {
                first = false;
                started.send(()).unwrap();
                blocked
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
            }
            output.lock().unwrap().push(store.recent().to_vec());
            Ok(())
        });
        let mut store = crate::store::Store::default();
        store.push_recent("nes/first");
        writer.queue(store.clone());
        ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        for n in 0..20 {
            store.push_recent(&format!("nes/{n}"));
            writer.queue(store.clone());
        }
        release.send(()).unwrap();
        writer.flush(store).unwrap();
        let saves = written.lock().unwrap();
        assert!(saves.len() <= 3, "pending snapshots were not coalesced");
        assert_eq!(saves.last().unwrap()[0], "nes/19");
    }
}
