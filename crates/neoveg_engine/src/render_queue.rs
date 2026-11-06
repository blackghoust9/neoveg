//! The render queue: several renders waiting to run one after the other.
//!
//! The queue is generic over what a job carries (the interface puts its launch request there), so
//! the order, the states and the "run the next one" rule are plain data with tests and need no
//! window, no FFmpeg and no thread. Only one job runs at a time; the interface starts the job the
//! queue hands out and reports back how it ended.

/// Where a queued render stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueState {
    Waiting,
    Running,
    Done,
    /// The render stopped with this message.
    Failed(String),
    /// Taken out of the run by the user before or while it ran.
    Cancelled,
}

impl QueueState {
    /// `true` once the job will not run (again).
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Done | Self::Failed(_) | Self::Cancelled)
    }
}

/// One entry of the queue.
#[derive(Debug, Clone)]
pub struct QueuedJob<T> {
    pub id: u64,
    /// Name shown in the list (the output file name, for example).
    pub label: String,
    pub state: QueueState,
    pub payload: T,
}

/// The list of renders, in the order they run.
#[derive(Debug, Clone)]
pub struct RenderQueue<T> {
    jobs: Vec<QueuedJob<T>>,
    next_id: u64,
}

impl<T> Default for RenderQueue<T> {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            next_id: 1,
        }
    }
}

impl<T> RenderQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn jobs(&self) -> &[QueuedJob<T>] {
        &self.jobs
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&QueuedJob<T>> {
        self.jobs.iter().find(|j| j.id == id)
    }

    /// Adds a job at the end, waiting. Returns its id.
    pub fn enqueue(&mut self, label: impl Into<String>, payload: T) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push(QueuedJob {
            id,
            label: label.into(),
            state: QueueState::Waiting,
            payload,
        });
        id
    }

    /// The job that is running, if any.
    pub fn running(&self) -> Option<&QueuedJob<T>> {
        self.jobs.iter().find(|j| j.state == QueueState::Running)
    }

    /// Number of jobs still waiting.
    pub fn waiting_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.state == QueueState::Waiting)
            .count()
    }

    /// Marks the first waiting job as running and returns it. `None` while a job is already
    /// running (only one runs at a time) or nothing waits.
    pub fn start_next(&mut self) -> Option<&QueuedJob<T>> {
        if self.running().is_some() {
            return None;
        }
        let job = self
            .jobs
            .iter_mut()
            .find(|j| j.state == QueueState::Waiting)?;
        job.state = QueueState::Running;
        Some(&*job)
    }

    /// Records how the running job `id` ended: `Ok` is done, `Err` carries the message.
    /// Returns `false` when `id` is not running.
    pub fn finish(&mut self, id: u64, result: Result<(), String>) -> bool {
        match self
            .jobs
            .iter_mut()
            .find(|j| j.id == id && j.state == QueueState::Running)
        {
            Some(job) => {
                job.state = match result {
                    Ok(()) => QueueState::Done,
                    Err(e) => QueueState::Failed(e),
                };
                true
            }
            None => false,
        }
    }

    /// Takes a waiting job out of the list. A running or finished job stays (cancel the running
    /// one first, clear the finished ones with [`Self::clear_finished`]).
    pub fn remove(&mut self, id: u64) -> bool {
        match self
            .jobs
            .iter()
            .position(|j| j.id == id && j.state == QueueState::Waiting)
        {
            Some(i) => {
                self.jobs.remove(i);
                true
            }
            None => false,
        }
    }

    /// Marks a waiting or running job as cancelled. The caller stops the thread of a running one.
    pub fn cancel(&mut self, id: u64) -> bool {
        match self
            .jobs
            .iter_mut()
            .find(|j| j.id == id && !j.state.is_finished())
        {
            Some(job) => {
                job.state = QueueState::Cancelled;
                true
            }
            None => false,
        }
    }

    /// Moves a waiting job one place earlier (`up`) or later, never past a job that is not
    /// waiting. Returns `true` when it moved.
    pub fn move_job(&mut self, id: u64, up: bool) -> bool {
        let Some(i) = self
            .jobs
            .iter()
            .position(|j| j.id == id && j.state == QueueState::Waiting)
        else {
            return false;
        };
        let j = if up {
            if i == 0 {
                return false;
            }
            i - 1
        } else {
            i + 1
        };
        if j >= self.jobs.len() || self.jobs[j].state != QueueState::Waiting {
            return false;
        }
        self.jobs.swap(i, j);
        true
    }

    /// Drops every job that is done, failed or cancelled.
    pub fn clear_finished(&mut self) {
        self.jobs.retain(|j| !j.state.is_finished());
    }

    /// Finished jobs over all jobs, for a "2 de 5" readout.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.jobs.iter().filter(|j| j.state.is_finished()).count(),
            self.jobs.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue() -> (RenderQueue<&'static str>, u64, u64, u64) {
        let mut q = RenderQueue::new();
        let a = q.enqueue("a.mp4", "A");
        let b = q.enqueue("b.wav", "B");
        let c = q.enqueue("c.mp4", "C");
        (q, a, b, c)
    }

    #[test]
    fn jobs_run_in_order_one_at_a_time() {
        let (mut q, a, b, c) = queue();
        assert_eq!(q.start_next().unwrap().id, a);
        assert!(q.start_next().is_none(), "only one runs at a time");
        assert!(q.finish(a, Ok(())));
        assert_eq!(q.start_next().unwrap().id, b);
        assert!(q.finish(b, Err("sem espaço".into())));
        assert_eq!(q.start_next().unwrap().id, c);
        assert!(q.finish(c, Ok(())));
        assert!(q.start_next().is_none());
        assert_eq!(
            q.get(b).unwrap().state,
            QueueState::Failed("sem espaço".into())
        );
        assert_eq!(q.progress(), (3, 3));
    }

    #[test]
    fn a_failed_job_does_not_stop_the_next() {
        let (mut q, a, b, _) = queue();
        q.start_next();
        q.finish(a, Err("x".into()));
        assert_eq!(q.start_next().unwrap().id, b);
    }

    #[test]
    fn only_waiting_jobs_can_be_removed_or_moved() {
        let (mut q, a, b, c) = queue();
        q.start_next();
        assert!(!q.remove(a), "running job stays");
        assert!(!q.move_job(b, true), "cannot move before a running job");
        assert!(q.move_job(c, true));
        assert_eq!(q.jobs()[1].id, c);
        assert!(q.remove(b));
        assert_eq!(q.jobs().len(), 2);
    }

    #[test]
    fn cancel_skips_the_job_and_clear_finished_cleans_up() {
        let (mut q, a, b, c) = queue();
        assert!(q.cancel(b));
        assert!(!q.cancel(b), "already finished");
        q.start_next();
        q.finish(a, Ok(()));
        assert_eq!(q.start_next().unwrap().id, c, "cancelled job is skipped");
        q.clear_finished();
        assert_eq!(q.jobs().len(), 1);
        assert_eq!(q.waiting_count(), 0);
        assert_eq!(q.running().unwrap().id, c);
    }

    #[test]
    fn finish_only_works_on_the_running_job() {
        let (mut q, a, _, _) = queue();
        assert!(!q.finish(a, Ok(())));
        assert!(!q.finish(999, Ok(())));
    }
}
