//! What runs in the background, in one place (from lumen's job registry): each job has a kind
//! (`histology`, `model`, `export`…), a name saying what it is doing, how far it is, and a cancel
//! flag. Only one job that drives the GPU runs at a time (a second is refused, saying what
//! runs). The status bar lists them with a cancel button each; the work itself (off the UI
//! thread) updates its job through its [`JobHandle`], which ends the job when dropped (finished,
//! failed or cancelled alike).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// A job running.
#[derive(Debug, Clone)]
pub struct Job {
    /// Its id (unique in this run of the app).
    pub id: u64,
    /// What kind of work it is (`model`, `export`).
    pub kind: String,
    /// What it is doing now ("Finding cells: tile 3 of 12").
    pub name: String,
    /// How far (0..1), when known.
    pub progress: Option<f32>,
    /// Whether it drives the GPU (one such job at a time).
    pub gpu: bool,
    /// Set to ask it to stop.
    pub cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct Inner {
    list: Vec<Job>,
    next: u64,
}

fn inner() -> &'static Mutex<Inner> {
    static J: OnceLock<Mutex<Inner>> = OnceLock::new();
    J.get_or_init(Default::default)
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

fn with<T>(f: impl FnOnce(&mut Inner) -> T) -> T {
    let mut g = inner().lock().unwrap_or_else(|e| e.into_inner());
    let out = f(&mut g);
    GENERATION.fetch_add(1, Ordering::Relaxed);
    out
}

/// Starts a job; refused while another GPU job runs (when this one is one too).
pub fn start(kind: impl Into<String>, name: &str, gpu: bool) -> Result<JobHandle, String> {
    let kind = kind.into();
    with(|j| {
        if gpu && let Some(other) = j.list.iter().find(|o| o.gpu) {
            return Err(format!("{} is still running", other.name));
        }
        j.next += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        j.list.push(Job { id: j.next, kind, name: name.to_string(), progress: None, gpu, cancel: cancel.clone() });
        Ok(JobHandle { id: j.next, cancel })
    })
}

/// The jobs running, oldest first.
pub fn running() -> Vec<Job> {
    let g = inner().lock().unwrap_or_else(|e| e.into_inner());
    g.list.clone()
}

/// The latest job of `kind`.
pub fn of(kind: &str) -> Option<Job> {
    running().into_iter().rev().find(|o| o.kind == kind)
}

/// Asks every job of `kind` to stop (when its work next checks).
pub fn cancel(kind: &str) {
    with(|j| j.list.iter().filter(|o| o.kind == kind).for_each(|o| o.cancel.store(true, Ordering::Relaxed)));
}

/// Asks the job `id` to stop.
pub fn cancel_id(id: u64) {
    with(|j| j.list.iter().filter(|o| o.id == id).for_each(|o| o.cancel.store(true, Ordering::Relaxed)));
}

/// A number that changes whenever a job starts, moves or ends (the status bar redraws then).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// A running job, held by the work doing it.
pub struct JobHandle {
    /// The job's id.
    pub id: u64,
    /// Set when someone asked it to stop.
    pub cancel: Arc<AtomicBool>,
}

impl JobHandle {
    /// Says what it is doing now, and how far.
    pub fn set(&self, name: &str, progress: Option<f32>) {
        with(|j| {
            if let Some(o) = j.list.iter_mut().find(|o| o.id == self.id) {
                o.name = name.to_string();
                o.progress = progress;
            }
        });
    }

    /// Whether someone asked it to stop.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        let id = self.id;
        with(|j| j.list.retain(|o| o.id != id));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn one_gpu_job_at_a_time_and_a_job_ends_with_its_handle() {
        let g0 = super::generation();
        let a = super::start("test-a", "first", true).unwrap();
        assert!(super::start("test-b", "second", true).is_err(), "a second GPU job is refused");
        let cpu = super::start("test-c", "on the CPU", false).unwrap();
        a.set("first: half", Some(0.5));
        assert_eq!(super::of("test-a").unwrap().progress, Some(0.5));
        super::cancel("test-a");
        assert!(a.cancelled());
        super::cancel_id(cpu.id);
        assert!(cpu.cancelled());
        drop(a);
        assert!(super::of("test-a").is_none());
        assert!(super::start("test-b", "second", true).is_ok(), "free again");
        drop(cpu);
        assert!(super::generation() > g0);
    }
}
