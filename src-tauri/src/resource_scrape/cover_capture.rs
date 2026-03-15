//! 批量截图封面模块
//!
//! 支持多线程异步截取视频帧作为封面，保存到本地并更新数据库
//! 任务持久化到 cover_capture_tasks 表。

use crate::db::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Semaphore};

/// 截图封面任务
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverCaptureTask {
    pub id: String,
    pub video_id: String,
    pub video_path: String,
    pub status: String,
    pub cover_path: Option<String>,
    pub error: Option<String>,
}

/// 批量截图封面进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverCaptureProgress {
    pub task_id: String,
    pub video_id: String,
    pub status: String,
    pub cover_path: Option<String>,
    pub error: Option<String>,
    pub completed: usize,
    pub total: usize,
}

/// 批量截图封面管理器
///
/// 使用 Arc 包装内部状态，支持 Clone，确保 commands 和 spawn 共享同一实例。
#[derive(Clone)]
pub struct CoverCaptureManager {
    app: AppHandle,
    is_running: Arc<Mutex<bool>>,
    is_stopped: Arc<Mutex<bool>>,
}

impl CoverCaptureManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            is_running: Arc::new(Mutex::new(false)),
            is_stopped: Arc::new(Mutex::new(false)),
        }
    }

    /// 批量截图封面
    ///
    /// 对传入的视频列表，使用 ffmpeg 在 10%~20% 位置随机截取一帧作为封面。
    /// 每个任务完成后同步更新数据库中的 cover_capture_tasks 和 videos 表。
    pub async fn batch_capture(
        &self,
        tasks: Vec<CoverCaptureTask>,
        concurrency: usize,
    ) -> Result<(), String> {
        {
            let mut running = self.is_running.lock().await;
            if *running {
                return Err("批量截图任务正在运行中".to_string());
            }
            *running = true;
        }
        {
            let mut stopped = self.is_stopped.lock().await;
            *stopped = false;
        }

        let total = tasks.len();
        let completed = Arc::new(Mutex::new(0usize));
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let is_stopped = self.is_stopped.clone();
        let app = self.app.clone();

        let mut handles = Vec::new();

        for task in tasks {
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| e.to_string())?;
            let app = app.clone();
            let completed = completed.clone();
            let is_stopped = is_stopped.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit;

                // 检查是否已停止
                if *is_stopped.lock().await {
                    return;
                }

                let db = Database::new(&app);

                // 更新数据库状态为 running
                let _ = db.update_cover_capture_task(&task.video_id, "running", None, None);

                // 发送运行中事件
                let _ = app.emit(
                    "cover-capture-progress",
                    CoverCaptureProgress {
                        task_id: task.id.clone(),
                        video_id: task.video_id.clone(),
                        status: "running".to_string(),
                        cover_path: None,
                        error: None,
                        completed: *completed.lock().await,
                        total,
                    },
                );

                // 再次检查停止标志（可能在等待信号量期间被停止）
                if *is_stopped.lock().await {
                    let _ = db.update_cover_capture_task(&task.video_id, "waiting", None, None);
                    return;
                }

                // 执行截图 (截取1张)
                let temp_dir = std::env::temp_dir()
                    .join(format!("jav_batch_captures_{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&temp_dir).unwrap_or_default();
                let output_path =
                    temp_dir.join(format!("cover_batch_{}.jpg", uuid::Uuid::new_v4()));
                let output_str = output_path.to_string_lossy().to_string();
                let video_path_owned = task.video_path.clone();

                let result = tokio::task::spawn_blocking(move || {
                    let duration_res = crate::utils::ffmpeg::get_video_duration(&video_path_owned);
                    if let Ok(duration) = duration_res {
                        let percentage: f64 = {
                            let mut rng = rand::thread_rng();
                            use rand::Rng;
                            rng.gen_range(0.00..0.10)
                        };
                        let timestamp = duration * percentage;
                        crate::utils::ffmpeg::extract_frame(
                            &video_path_owned,
                            timestamp,
                            &output_str,
                        )
                    } else {
                        Err("无法获取视频时长".to_string())
                    }
                })
                .await
                .unwrap_or(Err("Task join failed".to_string()));

                let mut count = completed.lock().await;
                *count += 1;
                let current_completed = *count;

                match result {
                    Ok(frame_path) => {
                        // 保存为封面
                        match crate::utils::media_assets::save_frame_as_cover(
                            &task.video_path,
                            &frame_path,
                        ) {
                            Ok(cover_path) => {
                                if let Ok(conn) = db.get_connection() {
                                    let _ = Database::update_video_poster_path(
                                        &conn,
                                        &task.video_id,
                                        &cover_path,
                                    );
                                }

                                // 更新截图任务表
                                let _ = db.update_cover_capture_task(
                                    &task.video_id,
                                    "completed",
                                    Some(&cover_path),
                                    None,
                                );

                                let _ = app.emit(
                                    "cover-capture-progress",
                                    CoverCaptureProgress {
                                        task_id: task.id.clone(),
                                        video_id: task.video_id.clone(),
                                        status: "completed".to_string(),
                                        cover_path: Some(cover_path),
                                        error: None,
                                        completed: current_completed,
                                        total,
                                    },
                                );
                            }
                            Err(e) => {
                                let _ = db.update_cover_capture_task(
                                    &task.video_id,
                                    "failed",
                                    None,
                                    Some(&e),
                                );

                                let _ = app.emit(
                                    "cover-capture-progress",
                                    CoverCaptureProgress {
                                        task_id: task.id.clone(),
                                        video_id: task.video_id.clone(),
                                        status: "failed".to_string(),
                                        cover_path: None,
                                        error: Some(e),
                                        completed: current_completed,
                                        total,
                                    },
                                );
                            }
                        }
                        // 清理临时文件
                        let _ = std::fs::remove_file(&frame_path);
                    }
                    Err(e) => {
                        let _ =
                            db.update_cover_capture_task(&task.video_id, "failed", None, Some(&e));

                        let _ = app.emit(
                            "cover-capture-progress",
                            CoverCaptureProgress {
                                task_id: task.id.clone(),
                                video_id: task.video_id.clone(),
                                status: "failed".to_string(),
                                cover_path: None,
                                error: Some(e),
                                completed: current_completed,
                                total,
                            },
                        );
                    }
                }
            });

            handles.push(handle);
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        // 发送完成事件
        let _ = self.app.emit(
            "cover-capture-done",
            serde_json::json!({
                "total": total,
                "completed": *completed.lock().await,
            }),
        );

        {
            let mut running = self.is_running.lock().await;
            *running = false;
        }

        Ok(())
    }

    /// 停止批量截图
    pub async fn stop(&self) {
        {
            let mut stopped = self.is_stopped.lock().await;
            *stopped = true;
        }
        // 将运行中的任务重置为等待
        let db = Database::new(&self.app);
        let _ = db.reset_running_cover_capture_tasks();
        {
            let mut running = self.is_running.lock().await;
            *running = false;
        }
    }

    /// 是否正在运行
    #[allow(dead_code)]
    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }
}
