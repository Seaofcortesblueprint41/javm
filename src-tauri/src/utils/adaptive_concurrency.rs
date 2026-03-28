//! 自适应并发控制器
//!
//! 根据 CPU 核心数和系统实时负载动态调整并发上限。
//! - 空闲时：并发数 = CPU 核心数
//! - 繁忙时：自动降低并发数，最低保留 1 路
//!
//! 内部使用 AtomicUsize + Notify 实现无锁限流，
//! 后台监控线程每 3 秒采样一次 CPU 使用率并调整上限。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// 自适应并发限制器
///
/// 使用方法：
/// ```ignore
/// let limiter = AdaptiveLimiter::start(None);
/// let guard = limiter.acquire().await;
/// // ... 执行工作 ...
/// drop(guard); // 释放槽位
/// limiter.shutdown();
/// ```
pub struct AdaptiveLimiter {
    inner: Arc<Inner>,
    monitor_handle: Option<JoinHandle<()>>,
}

struct Inner {
    /// 当前活跃任务数
    active: AtomicUsize,
    /// 当前允许的最大并发数（由监控线程动态调整）
    limit: AtomicUsize,
    /// CPU 核心数（并发上限）
    max_limit: usize,
    /// 有空位时通知等待中的任务
    notify: Notify,
    /// 停止监控线程
    shutdown: AtomicBool,
}

/// RAII 守卫，Drop 时自动释放并发槽位
pub struct LimiterGuard {
    inner: Arc<Inner>,
}

impl Drop for LimiterGuard {
    fn drop(&mut self) {
        self.inner.active.fetch_sub(1, Ordering::Release);
        self.inner.notify.notify_waiters();
    }
}

impl AdaptiveLimiter {
    /// 启动自适应限制器
    ///
    /// `max_override`: 可选的最大并发数上限。不传则使用 CPU 核心数。
    pub fn start(max_override: Option<usize>) -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let max_limit = max_override.unwrap_or(cpu_cores).max(1);

        let inner = Arc::new(Inner {
            active: AtomicUsize::new(0),
            limit: AtomicUsize::new(max_limit),
            max_limit,
            notify: Notify::new(),
            shutdown: AtomicBool::new(false),
        });

        let monitor_inner = inner.clone();
        let monitor_handle = tokio::spawn(async move {
            monitor_loop(monitor_inner).await;
        });

        Self {
            inner,
            monitor_handle: Some(monitor_handle),
        }
    }

    /// 获取一个并发槽位（可能等待）
    pub async fn acquire(&self) -> LimiterGuard {
        loop {
            let active = self.inner.active.load(Ordering::Acquire);
            let limit = self.inner.limit.load(Ordering::Acquire);

            if active < limit {
                // 尝试原子地 +1
                if self
                    .inner
                    .active
                    .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
                {
                    return LimiterGuard {
                        inner: self.inner.clone(),
                    };
                }
                // CAS 失败，重试（不 await，直接循环）
                continue;
            }

            // 已满，等待通知
            self.inner.notify.notified().await;
        }
    }

    /// 当前并发上限
    pub fn current_limit(&self) -> usize {
        self.inner.limit.load(Ordering::Relaxed)
    }

    /// 当前活跃任务数
    pub fn active_count(&self) -> usize {
        self.inner.active.load(Ordering::Relaxed)
    }

    /// CPU 核心数（最大上限）
    pub fn max_limit(&self) -> usize {
        self.inner.max_limit
    }

    /// 停止后台监控
    pub fn shutdown(&mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.monitor_handle.take() {
            handle.abort();
        }
    }
}

impl Drop for AdaptiveLimiter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 后台监控循环：每 3 秒采样 CPU 使用率，动态调整并发上限
async fn monitor_loop(inner: Arc<Inner>) {
    use sysinfo::System;

    let mut sys = System::new();

    // 首次需要刷新两次才能得到有效的 CPU 使用率
    sys.refresh_cpu_usage();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    loop {
        if inner.shutdown.load(Ordering::Acquire) {
            break;
        }

        sys.refresh_cpu_usage();
        let cpu_usage = sys.global_cpu_usage(); // 0.0 ~ 100.0

        // 计算期望并发数：
        //   系统空闲 (< 30%) → 用满核心数
        //   系统中等 (30-70%) → 线性缩减
        //   系统繁忙 (> 80%) → 最少 1 路
        //
        // 公式：desired = max_limit * (1 - (cpu_usage - 30) / 60)
        // 裁剪到 [1, max_limit]
        let desired = if cpu_usage <= 30.0 {
            inner.max_limit
        } else if cpu_usage >= 90.0 {
            1
        } else {
            let ratio = 1.0 - (cpu_usage as f64 - 30.0) / 60.0;
            let raw = (inner.max_limit as f64 * ratio).round() as usize;
            raw.clamp(1, inner.max_limit)
        };

        let old = inner.limit.swap(desired, Ordering::Release);
        if desired > old {
            // 上限增加了，唤醒可能在等待的任务
            inner.notify.notify_waiters();
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
