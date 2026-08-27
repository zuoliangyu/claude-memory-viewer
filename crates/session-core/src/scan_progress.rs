//! 冷启动扫描进度（best-effort 全局状态）。
//!
//! 初次启动时，provider 要并行扫描成百上千个会话文件来建缓存，这期间界面会
//! 几秒无响应。这里用一组全局原子记录"已扫描 / 总数 / 阶段"，前端在加载态轮询
//! [`snapshot`] 显示进度条，避免用户误以为卡死。
//!
//! 每次扫描持有独立 token。新扫描开始后，旧扫描迟到的进度和结束信号会被忽略，
//! 避免并发刷新把计数累加到新的扫描任务上。

use std::sync::OnceLock;

use parking_lot::Mutex;

/// 扫描阶段，决定前端展示的文案。
#[derive(Debug, Clone, Copy)]
pub enum Phase {
    Projects = 1,
    Sessions = 2,
    Index = 3,
}

#[derive(Debug, Clone, Copy)]
pub struct ScanToken(u64);

#[derive(Default)]
struct ProgressState {
    generation: u64,
    active: bool,
    scanned: u64,
    total: u64,
    phase: u8,
}

#[derive(Default)]
struct ProgressTracker {
    state: Mutex<ProgressState>,
}

impl ProgressTracker {
    fn begin(&self, phase: Phase, total: u64) -> ScanToken {
        let mut state = self.state.lock();
        state.generation = state.generation.wrapping_add(1);
        state.active = true;
        state.scanned = 0;
        state.total = total;
        state.phase = phase as u8;
        ScanToken(state.generation)
    }

    fn inc(&self, token: ScanToken) {
        let mut state = self.state.lock();
        if state.active && state.generation == token.0 {
            state.scanned = state.scanned.saturating_add(1).min(state.total);
        }
    }

    fn finish(&self, token: ScanToken) {
        let mut state = self.state.lock();
        if state.generation == token.0 {
            state.active = false;
        }
    }

    fn snapshot(&self) -> ScanProgress {
        let state = self.state.lock();
        ScanProgress {
            active: state.active,
            scanned: state.scanned,
            total: state.total,
            phase: phase_label(state.phase),
        }
    }
}

fn tracker() -> &'static ProgressTracker {
    static TRACKER: OnceLock<ProgressTracker> = OnceLock::new();
    TRACKER.get_or_init(ProgressTracker::default)
}

/// 发给前端的进度快照。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub active: bool,
    pub scanned: u64,
    pub total: u64,
    pub phase: String,
}

fn phase_label(code: u8) -> String {
    match code {
        1 => "扫描项目",
        2 => "扫描会话",
        3 => "建立索引",
        _ => "扫描中",
    }
    .to_string()
}

/// 开始一段扫描：重置计数并标记 active。`total` 为预期处理的条目数。
pub fn begin(phase: Phase, total: u64) -> ScanToken {
    tracker().begin(phase, total)
}

/// 完成一个条目（应在每次迭代结束时调用，无论结果是否被保留）。
pub fn inc(token: ScanToken) {
    tracker().inc(token);
}

/// 结束当前扫描段。
pub fn finish(token: ScanToken) {
    tracker().finish(token);
}

/// 读取当前进度快照。
pub fn snapshot() -> ScanProgress {
    tracker().snapshot()
}

/// 把 rayon 全局线程池限制为「核数 − 1」（至少 1），给 UI 主线程留出一个核，
/// 否则冷启动并行扫描会吃满全部 CPU，导致界面（甚至整机）卡顿、连进度条都画
/// 不动。应在程序启动早期调用一次；重复调用或已初始化时静默忽略。
pub fn configure_rayon_pool() {
    let cores = std::thread::available_parallelism()
        .map(|c| c.get())
        .unwrap_or(1);
    let threads = cores.saturating_sub(1).max(1);
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_scan_updates_do_not_pollute_current_progress() {
        let tracker = ProgressTracker::default();
        let stale = tracker.begin(Phase::Projects, 3);
        tracker.inc(stale);

        let current = tracker.begin(Phase::Index, 1);
        tracker.inc(stale);
        tracker.finish(stale);
        tracker.inc(current);

        let snapshot = tracker.snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.scanned, 1);
        assert_eq!(snapshot.total, 1);
        assert_eq!(snapshot.phase, "建立索引");

        tracker.inc(current);
        tracker.finish(current);
        let snapshot = tracker.snapshot();
        assert!(!snapshot.active);
        assert_eq!(snapshot.scanned, 1);
    }
}
