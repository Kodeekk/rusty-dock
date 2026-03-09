use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Associates a dock entry index with a running process PID.
pub struct ProcessMonitor {
    tracked: HashMap<usize, u32>,
    last_poll: Instant,
    poll_interval: Duration,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            tracked: HashMap::new(),
            last_poll: Instant::now(),
            poll_interval: Duration::from_millis(1500),
        }
    }

    pub fn register(&mut self, entry_index: usize, pid: u32) {
        self.tracked.insert(entry_index, pid);
    }

    pub fn should_poll(&self) -> bool {
        self.last_poll.elapsed() >= self.poll_interval
    }

    pub fn poll_dead(&mut self) -> Vec<usize> {
        self.last_poll = Instant::now();
        let mut dead = Vec::new();
        self.tracked.retain(|&idx, &mut pid| {
            let alive = Self::pid_is_alive(pid);
            if !alive {
                dead.push(idx);
            }
            alive
        });
        dead
    }

    fn pid_is_alive(pid: u32) -> bool {
        let proc_path = format!("/proc/{}", pid);
        if let Ok(meta) = std::fs::metadata(&proc_path) {
            if meta.is_dir() {
                return !Self::is_zombie(pid);
            }
        }
        false
    }

    fn is_zombie(pid: u32) -> bool {
        let status_path = format!("/proc/{}/status", pid);
        if let Ok(content) = std::fs::read_to_string(&status_path) {
            for line in content.lines() {
                if line.starts_with("State:") {
                    return line.contains('Z');
                }
            }
        }
        false
    }
}
