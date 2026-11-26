//! The ordered writer for the saved-connection order.
//!
//! `choose_sort` runs on the UI thread while the file write runs on a blocking
//! worker. Spawning one task per choice would let an older write finish after
//! a newer one, leaving the older order on disk. Every window instead shares
//! one FIFO writer: choices enter the queue in the order the user made them,
//! and a read that reopens the picker queues behind the writes it must
//! observe. A failed write is reported without stopping later choices.

use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};

use tterm_core::config::load_config_file;
use tterm_core::config::profile_sort_file::save_profile_sort;

use super::ProfileSort;

/// The error a queued write or read reports.
type SortResult<T> = Result<T, String>;

/// Waits for one queued choice to finish.
pub(super) struct SaveTicket(Receiver<SortResult<()>>);

impl SaveTicket {
    /// Waits for the write, reporting a stopped writer as an error.
    pub(super) fn wait(self) -> SortResult<()> {
        self.0
            .recv()
            .map_err(|_| "The saved-connection order writer stopped.".to_string())?
    }
}

/// Waits for one queued read to finish.
struct LoadTicket(Receiver<SortResult<ProfileSort>>);

impl LoadTicket {
    fn wait(self) -> SortResult<ProfileSort> {
        self.0
            .recv()
            .map_err(|_| "The saved-connection order writer stopped.".to_string())?
    }
}

enum Command {
    Save {
        sort: ProfileSort,
        done: Sender<SortResult<()>>,
    },
    Load {
        done: Sender<SortResult<ProfileSort>>,
    },
}

/// The file operations the writer performs on its own thread.
trait Backend: Send + 'static {
    fn save(&mut self, sort: ProfileSort) -> SortResult<()>;

    fn load(&mut self) -> SortResult<ProfileSort>;
}

/// The shared config file the picker reads and writes.
struct ConfigFile;

impl Backend for ConfigFile {
    fn save(&mut self, sort: ProfileSort) -> SortResult<()> {
        save_profile_sort(sort.config_value())
    }

    fn load(&mut self) -> SortResult<ProfileSort> {
        load_config_file().map(|config| ProfileSort::from_config(&config.profile_sort))
    }
}

/// One FIFO writer thread and the queue feeding it.
struct Writer {
    sender: Sender<Command>,
}

impl Writer {
    fn spawn(backend: impl Backend) -> SortResult<Self> {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("tterm-profile-sort".to_string())
            .spawn(move || run(backend, &receiver))
            .map_err(|error| {
                format!("Failed to start the saved-connection order writer: {error}")
            })?;
        Ok(Self { sender })
    }

    fn save(&self, sort: ProfileSort) -> SortResult<SaveTicket> {
        let (done, receiver) = mpsc::channel();
        self.sender
            .send(Command::Save { sort, done })
            .map_err(|_| "The saved-connection order writer stopped.".to_string())?;
        Ok(SaveTicket(receiver))
    }

    fn enqueue_load(&self) -> SortResult<LoadTicket> {
        let (done, receiver) = mpsc::channel();
        self.sender
            .send(Command::Load { done })
            .map_err(|_| "The saved-connection order writer stopped.".to_string())?;
        Ok(LoadTicket(receiver))
    }

    fn load(&self) -> SortResult<ProfileSort> {
        self.enqueue_load()?.wait()
    }
}

fn run(mut backend: impl Backend, receiver: &Receiver<Command>) {
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Save { sort, done } => {
                let _ = done.send(backend.save(sort));
            }
            Command::Load { done } => {
                let _ = done.send(backend.load());
            }
        }
    }
}

static WRITER: OnceLock<Result<Writer, String>> = OnceLock::new();

fn writer() -> Result<&'static Writer, String> {
    WRITER
        .get_or_init(|| Writer::spawn(ConfigFile))
        .as_ref()
        .map_err(|error| error.clone())
}

/// Queues one chosen order behind the choices made before it.
#[cfg(not(test))]
pub(super) fn request_save(sort: ProfileSort) -> SortResult<SaveTicket> {
    writer()?.save(sort)
}

/// Reads the stored order after the queued writes have finished.
pub(super) fn load() -> SortResult<ProfileSort> {
    writer()?.load()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(1);

    struct GatedBackend {
        writes: Arc<Mutex<Vec<ProfileSort>>>,
        started: Sender<ProfileSort>,
        gate: Receiver<()>,
        first: bool,
    }

    impl Backend for GatedBackend {
        fn save(&mut self, sort: ProfileSort) -> SortResult<()> {
            let _ = self.started.send(sort);
            if std::mem::take(&mut self.first) {
                let _ = self.gate.recv();
            }
            self.writes.lock().unwrap().push(sort);
            Ok(())
        }

        fn load(&mut self) -> SortResult<ProfileSort> {
            Ok(self
                .writes
                .lock()
                .unwrap()
                .last()
                .copied()
                .unwrap_or(ProfileSort::Group))
        }
    }

    struct FailingBackend {
        writes: Arc<Mutex<Vec<ProfileSort>>>,
        attempts: usize,
    }

    impl Backend for FailingBackend {
        fn save(&mut self, sort: ProfileSort) -> SortResult<()> {
            self.attempts += 1;
            if self.attempts == 1 {
                return Err("disk full".to_string());
            }
            self.writes.lock().unwrap().push(sort);
            Ok(())
        }

        fn load(&mut self) -> SortResult<ProfileSort> {
            Ok(self
                .writes
                .lock()
                .unwrap()
                .last()
                .copied()
                .unwrap_or(ProfileSort::Group))
        }
    }

    #[test]
    fn later_choices_are_written_after_earlier_ones_and_a_reopen_waits_for_them() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let (started, started_rx) = mpsc::channel();
        let (gate, gate_rx) = mpsc::channel();
        let writer = Writer::spawn(GatedBackend {
            writes: Arc::clone(&writes),
            started,
            gate: gate_rx,
            first: true,
        })
        .unwrap();

        let first = writer.save(ProfileSort::Recent).unwrap();
        assert_eq!(
            started_rx.recv_timeout(TIMEOUT).unwrap(),
            ProfileSort::Recent
        );
        let middle = writer.save(ProfileSort::Group).unwrap();
        let last = writer.save(ProfileSort::Recent).unwrap();
        assert!(matches!(
            started_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));

        let reopened = writer.enqueue_load().unwrap();

        gate.send(()).unwrap();
        assert_eq!(first.wait(), Ok(()));
        assert_eq!(middle.wait(), Ok(()));
        assert_eq!(last.wait(), Ok(()));
        assert_eq!(reopened.wait(), Ok(ProfileSort::Recent));
        assert_eq!(
            *writes.lock().unwrap(),
            vec![ProfileSort::Recent, ProfileSort::Group, ProfileSort::Recent]
        );
    }

    #[test]
    fn a_failed_write_does_not_block_a_later_choice_or_a_reopen() {
        let writer = Writer::spawn(FailingBackend {
            writes: Arc::new(Mutex::new(Vec::new())),
            attempts: 0,
        })
        .unwrap();

        assert_eq!(
            writer.save(ProfileSort::Group).unwrap().wait(),
            Err("disk full".to_string())
        );
        assert_eq!(writer.save(ProfileSort::Recent).unwrap().wait(), Ok(()));
        assert_eq!(writer.load(), Ok(ProfileSort::Recent));
    }
}
