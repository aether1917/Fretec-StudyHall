use std::sync::Mutex;

use crate::types::{
    now_ms, remaining_ms, Phase, TimerSnapshot, TimerStatus, BREAK_TOTAL_MS, FOCUS_TOTAL_MS,
};

#[derive(Debug)]
pub struct TimerEngine {
    inner: Mutex<TimerSnapshot>,
}

impl TimerEngine {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Self::idle_snapshot()),
        }
    }

    pub fn idle_snapshot() -> TimerSnapshot {
        TimerSnapshot {
            phase: Phase::Focus,
            total_ms: FOCUS_TOTAL_MS,
            started_at_ms: None,
            pause_offset_ms: 0,
            paused_at_ms: None,
            status: TimerStatus::Idle,
        }
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        self.inner.lock().expect("timer lock").clone()
    }

    fn set(&self, snapshot: TimerSnapshot) -> TimerSnapshot {
        *self.inner.lock().expect("timer lock") = snapshot.clone();
        snapshot
    }

    pub fn start(&self) -> TimerSnapshot {
        let mut guard = self.inner.lock().expect("timer lock");
        match guard.status {
            TimerStatus::Idle => {
                let now = now_ms();
                let snapshot = TimerSnapshot {
                    phase: Phase::Focus,
                    total_ms: FOCUS_TOTAL_MS,
                    started_at_ms: Some(now),
                    pause_offset_ms: 0,
                    paused_at_ms: None,
                    status: TimerStatus::Running,
                };
                *guard = snapshot.clone();
                snapshot
            }
            TimerStatus::Paused => {
                let now = now_ms();
                if let Some(paused_at) = guard.paused_at_ms {
                    guard.pause_offset_ms = guard
                        .pause_offset_ms
                        .saturating_add(now.saturating_sub(paused_at));
                }
                guard.paused_at_ms = None;
                guard.status = TimerStatus::Running;
                guard.clone()
            }
            TimerStatus::Running => guard.clone(),
        }
    }

    pub fn pause(&self) -> TimerSnapshot {
        let mut guard = self.inner.lock().expect("timer lock");
        if guard.status == TimerStatus::Running {
            guard.paused_at_ms = Some(now_ms());
            guard.status = TimerStatus::Paused;
        }
        guard.clone()
    }

    pub fn resume(&self) -> TimerSnapshot {
        let mut guard = self.inner.lock().expect("timer lock");
        if guard.status == TimerStatus::Paused {
            let now = now_ms();
            if let Some(paused_at) = guard.paused_at_ms {
                guard.pause_offset_ms = guard
                    .pause_offset_ms
                    .saturating_add(now.saturating_sub(paused_at));
            }
            guard.paused_at_ms = None;
            guard.status = TimerStatus::Running;
        }
        guard.clone()
    }

    pub fn stop(&self) -> TimerSnapshot {
        self.set(Self::idle_snapshot())
    }

    /// 接收局域网同步的计时状态（不触发本地广播）。
    pub fn apply_remote(&self, snapshot: TimerSnapshot) -> TimerSnapshot {
        self.set(snapshot)
    }

    /// 阶段归零后立即进入下一阶段并继续计时；返回新的快照。
    pub fn tick(&self) -> Option<TimerSnapshot> {
        let mut guard = self.inner.lock().expect("timer lock");
        if guard.status != TimerStatus::Running {
            return None;
        }
        if remaining_ms(&guard) > 0 {
            return None;
        }
        let now = now_ms();
        let next_phase = match guard.phase {
            Phase::Focus => Phase::Break,
            Phase::Break => Phase::Focus,
        };
        let total = match next_phase {
            Phase::Focus => FOCUS_TOTAL_MS,
            Phase::Break => BREAK_TOTAL_MS,
        };
        let snapshot = TimerSnapshot {
            phase: next_phase,
            total_ms: total,
            started_at_ms: Some(now),
            pause_offset_ms: 0,
            paused_at_ms: None,
            status: TimerStatus::Running,
        };
        *guard = snapshot.clone();
        Some(snapshot)
    }
}

pub fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Focus => "专注",
        Phase::Break => "休息",
    }
}

pub fn format_mmss(ms: u64) -> String {
    let total_seconds = ms.div_ceil(1000);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

pub fn tray_tooltip(snapshot: &TimerSnapshot) -> String {
    match snapshot.status {
        TimerStatus::Idle => format!("{} · {}", phase_label(snapshot.phase), format_mmss(FOCUS_TOTAL_MS)),
        TimerStatus::Paused => format!(
            "已暂停 · {} {}",
            phase_label(snapshot.phase),
            format_mmss(remaining_ms(snapshot))
        ),
        TimerStatus::Running => format!(
            "{} · {}",
            phase_label(snapshot.phase),
            format_mmss(remaining_ms(snapshot))
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elapsed_snapshot(engine: &TimerEngine) {
        let mut guard = engine.inner.lock().expect("timer lock");
        guard.started_at_ms = Some(now_ms().saturating_sub(guard.total_ms + 1_000));
        guard.paused_at_ms = None;
    }

    #[test]
    fn start_pause_resume_keeps_position() {
        let engine = TimerEngine::new();
        let running = engine.start();
        assert_eq!(running.status, TimerStatus::Running);
        let paused = engine.pause();
        assert_eq!(paused.status, TimerStatus::Paused);
        assert!(paused.paused_at_ms.is_some());
        let resumed = engine.resume();
        assert_eq!(resumed.status, TimerStatus::Running);
        assert!(resumed.paused_at_ms.is_none());
    }

    #[test]
    fn stop_resets_to_focus_idle() {
        let engine = TimerEngine::new();
        engine.start();
        let stopped = engine.stop();
        assert_eq!(stopped.status, TimerStatus::Idle);
        assert_eq!(stopped.phase, Phase::Focus);
        assert_eq!(stopped.total_ms, FOCUS_TOTAL_MS);
        assert_eq!(stopped.started_at_ms, None);
    }

    #[test]
    fn tick_advances_focus_to_break_without_waiting_for_user() {
        let engine = TimerEngine::new();
        engine.start();
        elapsed_snapshot(&engine);
        let next = engine.tick().expect("phase should advance");
        assert_eq!(next.phase, Phase::Break);
        assert_eq!(next.total_ms, BREAK_TOTAL_MS);
        assert_eq!(next.status, TimerStatus::Running);
    }

    #[test]
    fn tick_advances_break_to_focus() {
        let engine = TimerEngine::new();
        {
            let mut guard = engine.inner.lock().expect("timer lock");
            *guard = TimerSnapshot {
                phase: Phase::Break,
                total_ms: BREAK_TOTAL_MS,
                started_at_ms: Some(now_ms()),
                pause_offset_ms: 0,
                paused_at_ms: None,
                status: TimerStatus::Running,
            };
        }
        elapsed_snapshot(&engine);
        let next = engine.tick().expect("phase should advance");
        assert_eq!(next.phase, Phase::Focus);
        assert_eq!(next.total_ms, FOCUS_TOTAL_MS);
    }

    #[test]
    fn paused_timer_does_not_advance() {
        let engine = TimerEngine::new();
        engine.start();
        engine.pause();
        elapsed_snapshot(&engine);
        assert!(engine.tick().is_none());
    }
}
