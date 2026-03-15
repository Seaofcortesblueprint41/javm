use crate::db::Database;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskResponse {
    pub id: String,
    pub url: String,
    pub filename: Option<String>,
    pub save_path: String,
    pub status: String,
    pub progress: f64,
    pub speed: u64,
    pub downloaded: u64,
    pub total: u64,
    pub downloader: String,
    pub retry_count: i32,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// 状态码转状态字符串
fn status_code_to_string(code: i32) -> String {
    match code {
        0 => "queued".to_string(),
        1 => "preparing".to_string(),
        2 => "downloading".to_string(),
        3 => "merging".to_string(),
        4 => "scraping".to_string(),
        5 => "paused".to_string(),
        6 => "completed".to_string(),
        7 => "failed".to_string(),
        8 => "retrying".to_string(),
        9 => "cancelled".to_string(),
        _ => "unknown".to_string(),
    }
}

/// 状态码转中文状态名
fn status_code_to_chinese(code: i32) -> String {
    match code {
        0 => "排队中".to_string(),
        1 => "准备中".to_string(),
        2 => "下载中".to_string(),
        3 => "合并中".to_string(),
        4 => "刮削中".to_string(),
        5 => "已暂停".to_string(),
        6 => "已完成".to_string(),
        7 => "失败".to_string(),
        8 => "重试中".to_string(),
        9 => "已取消".to_string(),
        _ => "未知".to_string(),
    }
}

#[tauri::command]
pub async fn get_download_tasks(app: AppHandle) -> Result<Vec<DownloadTaskResponse>, String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, url, save_path, filename, total_bytes, downloaded_bytes,
                    status, error_message, downloader_type, retry_count, progress,
                    created_at, updated_at, completed_at
             FROM downloads
             ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let tasks = stmt
        .query_map([], |row| {
            let total_bytes: Option<i64> = row.get(4)?;
            let downloaded_bytes: i64 = row.get(5).unwrap_or(0);
            let status_code: i32 = row.get(6)?;
            let total = total_bytes.unwrap_or(0) as u64;
            let downloaded = downloaded_bytes as u64;

            let progress: f64 = row.get::<_, Option<f64>>(10)?.unwrap_or_else(|| {
                if total > 0 {
                    (downloaded as f64 / total as f64) * 100.0
                } else {
                    0.0
                }
            });

            Ok(DownloadTaskResponse {
                id: row.get(0)?,
                url: row.get(1)?,
                save_path: row.get(2)?,
                filename: row.get(3)?,
                total,
                downloaded,
                status: status_code_to_string(status_code),
                progress,
                speed: 0,
                downloader: row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "N_m3u8DL-RE".to_string()),
                retry_count: row.get(9).unwrap_or(0),
                error: row.get(7)?,
                created_at: row.get(11)?,
                started_at: row.get::<_, Option<String>>(12).ok().flatten(),
                completed_at: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for task in tasks {
        result.push(task.map_err(|e| e.to_string())?);
    }

    Ok(result)
}

#[tauri::command]
pub async fn add_download_task(
    app: AppHandle,
    url: String,
    save_path: String,
    filename: Option<String>,
) -> Result<String, String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let existing_task: Option<(String, i32)> = conn
        .query_row(
            "SELECT id, status FROM downloads WHERE url = ? LIMIT 1",
            [&url],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    if let Some((_existing_id, status)) = existing_task {
        let status_name = status_code_to_chinese(status);
        return Err(format!(
            "该视频任务已存在（状态：{}），请勿重复添加",
            status_name
        ));
    }

    let id = Uuid::new_v4().to_string();
    let filename_to_save = filename.or_else(|| extract_filename_from_url(&url));

    conn.execute(
        "INSERT INTO downloads (id, url, save_path, filename, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 0, datetime('now'), datetime('now'))",
        rusqlite::params![id, url, save_path, filename_to_save],
    )
    .map_err(|e| {
        e.to_string()
    })?;

    if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
        let task = crate::download::manager::DownloadTask {
            id: id.clone(),
            url: url.clone(),
            save_path: save_path.clone(),
            filename: filename_to_save.clone(),
        };
        manager.add_task(task).await;

        let app_clone = app.clone();
        let manager_clone = manager.inner().clone();
        tokio::spawn(async move {
            manager_clone.schedule_next(app_clone).await;
        });
    } else {
        return Err("DownloadManager not initialized".to_string());
    }

    app.emit("download-task-added", &id)
        .map_err(|e| e.to_string())?;

    Ok(id)
}

fn extract_filename_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    parsed
        .path_segments()?
        .last()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
pub async fn pause_download_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE downloads SET status = 5, updated_at = datetime('now') WHERE id = ?",
        [&task_id],
    )
    .map_err(|e| e.to_string())?;

    app.emit("download-task-paused", &task_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn resume_download_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let (url, save_path, filename): (String, String, Option<String>) = conn
        .query_row(
            "SELECT url, save_path, filename FROM downloads WHERE id = ?",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| {
            e.to_string()
        })?;

    conn.execute(
        "UPDATE downloads SET status = 0, updated_at = datetime('now') WHERE id = ?",
        [&task_id],
    )
    .map_err(|e| {
        e.to_string()
    })?;

    if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
        let task = crate::download::manager::DownloadTask {
            id: task_id.clone(),
            url,
            save_path,
            filename,
        };
        manager.add_task(task).await;

        let app_clone = app.clone();
        let manager_clone = manager.inner().clone();
        tokio::spawn(async move {
            manager_clone.schedule_next(app_clone).await;
        });
    } else {
        return Err("DownloadManager not initialized".to_string());
    }

    app.emit("download-task-resumed", &task_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn cancel_download_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE downloads SET status = 9, updated_at = datetime('now') WHERE id = ?",
        [&task_id],
    )
    .map_err(|e| e.to_string())?;

    app.emit("download-task-cancelled", &task_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_download_task(app: AppHandle, task_id: String) -> Result<(), String> {
    if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
        manager.stop_task(&task_id).await?;
    }

    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE downloads SET status = 9, updated_at = datetime('now') WHERE id = ?",
        [&task_id],
    )
    .map_err(|e| e.to_string())?;

    app.emit("download-task-stopped", &task_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn retry_download_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let (url, save_path, filename): (String, String, Option<String>) = conn
        .query_row(
            "SELECT url, save_path, filename FROM downloads WHERE id = ?",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| {
            e.to_string()
        })?;

    conn.execute(
        "UPDATE downloads SET status = 0, downloaded_bytes = 0, error_message = NULL,
         retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?",
        [&task_id],
    )
    .map_err(|e| {
        e.to_string()
    })?;

    if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
        let task = crate::download::manager::DownloadTask {
            id: task_id.clone(),
            url,
            save_path,
            filename,
        };
        manager.add_task(task).await;

        let app_clone = app.clone();
        let manager_clone = manager.inner().clone();
        tokio::spawn(async move {
            manager_clone.schedule_next(app_clone).await;
        });
    } else {
        return Err("DownloadManager not initialized".to_string());
    }

    app.emit("download-task-retried", &task_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_download_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let temp_path: Option<String> = conn
        .query_row(
            "SELECT temp_path FROM downloads WHERE id = ?",
            [&task_id],
            |row| row.get(0),
        )
        .ok();

    if let Some(path) = temp_path {
        if std::path::Path::new(&path).exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    conn.execute("DELETE FROM downloads WHERE id = ?", [&task_id])
        .map_err(|e| e.to_string())?;

    app.emit("download-task-deleted", &task_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn rename_download_task(
    app: AppHandle,
    task_id: String,
    new_filename: String,
) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    if new_filename.trim().is_empty() {
        return Err("文件名不能为空".to_string());
    }

    let (status, old_filename, save_path): (i32, Option<String>, String) = conn
        .query_row(
            "SELECT status, filename, save_path FROM downloads WHERE id = ?",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    if let Some(old) = &old_filename {
        if old == &new_filename {
            return Ok(());
        }
    }

    match status {
        5 => {
            // Completed: rename actual files + update DB
            if let Some(old_name) = &old_filename {
                let exts = [".mp4", ".mkv", ".ts", ".m4a"];
                for ext in exts.iter() {
                    let old_path =
                        std::path::Path::new(&save_path).join(format!("{}{}", old_name, ext));
                    if old_path.exists() {
                        let new_path = std::path::Path::new(&save_path)
                            .join(format!("{}{}", new_filename, ext));
                        let _ = std::fs::rename(old_path, new_path);
                    }
                }
            }
            conn.execute(
                "UPDATE downloads SET filename = ?, updated_at = datetime('now') WHERE id = ?",
                rusqlite::params![new_filename, task_id],
            )
            .map_err(|e| e.to_string())?;
            app.emit("download-task-renamed", &task_id)
                .map_err(|e| e.to_string())?;
        }
        2 | 3 | 7 => {
            // Downloading, Merging, Retrying: stop, reset, rename, and enqueue
            if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
                let _ = manager.stop_task(&task_id).await;
            }

            conn.execute(
                "UPDATE downloads SET filename = ?, status = 0, downloaded_bytes = 0, progress = 0, error_message = NULL, updated_at = datetime('now') WHERE id = ?",
                rusqlite::params![new_filename, task_id],
            )
            .map_err(|e| e.to_string())?;

            app.emit("download-task-renamed", &task_id)
                .map_err(|e| e.to_string())?;

            let progress_payload = crate::download::manager::DownloadProgress {
                task_id: task_id.clone(),
                progress: 0.0,
                speed: 0,
                downloaded: 0,
                total: 0,
                status: 0,
            };
            app.emit("download-progress", &progress_payload).ok();

            // Re-enqueue
            let url: String = conn
                .query_row(
                    "SELECT url FROM downloads WHERE id = ?",
                    [&task_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
                let task = crate::download::manager::DownloadTask {
                    id: task_id.clone(),
                    url,
                    save_path: save_path.clone(),
                    filename: Some(new_filename.clone()),
                };
                manager.add_task(task).await;
                let app_clone = app.clone();
                let manager_clone = manager.inner().clone();
                tokio::spawn(async move {
                    manager_clone.schedule_next(app_clone).await;
                });
            }
        }
        _ => {
            // Not started (0, 1) or Paused/Failed/Cancelled (4, 6, 8): just update DB
            conn.execute(
                "UPDATE downloads SET filename = ?, updated_at = datetime('now') WHERE id = ?",
                rusqlite::params![new_filename, task_id],
            )
            .map_err(|e| e.to_string())?;

            app.emit("download-task-renamed", &task_id)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn change_download_save_path(
    app: AppHandle,
    task_id: String,
    new_save_path: String,
) -> Result<(), String> {
    let db = Database::new(&app);
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    if new_save_path.trim().is_empty() {
        return Err("保存路径不能为空".to_string());
    }

    let (status, old_save_path, filename): (i32, String, Option<String>) = conn
        .query_row(
            "SELECT status, save_path, filename FROM downloads WHERE id = ?",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    if old_save_path == new_save_path {
        return Ok(());
    }

    match status {
        5 => {
            // Completed: Cannot change save path, handled by frontend
            return Err("已完成的任务无法修改保存路径".to_string());
        }
        2 | 3 | 7 => {
            // Downloading, Merging, Retrying: stop, reset, rename, and enqueue
            if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
                let _ = manager.stop_task(&task_id).await;
            }

            conn.execute(
                "UPDATE downloads SET save_path = ?, status = 0, downloaded_bytes = 0, progress = 0, error_message = NULL, updated_at = datetime('now') WHERE id = ?",
                rusqlite::params![new_save_path, task_id],
            )
            .map_err(|e| e.to_string())?;

            app.emit("download-task-path-changed", &task_id)
                .map_err(|e| e.to_string())?;

            let progress_payload = crate::download::manager::DownloadProgress {
                task_id: task_id.clone(),
                progress: 0.0,
                speed: 0,
                downloaded: 0,
                total: 0,
                status: 0,
            };
            app.emit("download-progress", &progress_payload).ok();

            // Re-enqueue
            let url: String = conn
                .query_row(
                    "SELECT url FROM downloads WHERE id = ?",
                    [&task_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
                let task = crate::download::manager::DownloadTask {
                    id: task_id.clone(),
                    url,
                    save_path: new_save_path.clone(),
                    filename,
                };
                manager.add_task(task).await;
                let app_clone = app.clone();
                let manager_clone = manager.inner().clone();
                tokio::spawn(async move {
                    manager_clone.schedule_next(app_clone).await;
                });
            }
        }
        _ => {
            // Not started (0, 1) or Paused/Failed/Cancelled (4, 6, 8): just update DB
            conn.execute(
                "UPDATE downloads SET save_path = ?, updated_at = datetime('now') WHERE id = ?",
                rusqlite::params![new_save_path, task_id],
            )
            .map_err(|e| e.to_string())?;

            app.emit("download-task-path-changed", &task_id)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_default_download_path(app: AppHandle) -> Result<String, String> {
    if let Ok(path) = app.path().download_dir() {
        return Ok(path.to_string_lossy().to_string());
    }

    if let Ok(path) = app.path().home_dir() {
        return Ok(path.join("Downloads").to_string_lossy().to_string());
    }

    Err("无法解析系统默认下载目录".to_string())
}

#[tauri::command]
pub async fn batch_pause_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut failed = Vec::new();
    for task_id in task_ids {
        if let Err(_e) = pause_download_task(app.clone(), task_id.clone()).await {
            failed.push(task_id);
        }
    }
    Ok(failed)
}

#[tauri::command]
pub async fn batch_resume_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut failed = Vec::new();
    for task_id in task_ids {
        if let Err(_e) = resume_download_task(app.clone(), task_id.clone()).await {
            failed.push(task_id);
        }
    }
    Ok(failed)
}

#[tauri::command]
pub async fn batch_stop_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut failed = Vec::new();
    for task_id in task_ids {
        if let Err(_e) = stop_download_task(app.clone(), task_id.clone()).await {
            failed.push(task_id);
        }
    }
    Ok(failed)
}

#[tauri::command]
pub async fn batch_retry_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut failed = Vec::new();
    for task_id in task_ids {
        if let Err(_e) = retry_download_task(app.clone(), task_id.clone()).await {
            failed.push(task_id);
        }
    }
    Ok(failed)
}

#[tauri::command]
pub async fn batch_delete_tasks(
    app: AppHandle,
    task_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut failed = Vec::new();
    for task_id in task_ids {
        if let Err(_e) = delete_download_task(app.clone(), task_id.clone()).await {
            failed.push(task_id);
        }
    }
    Ok(failed)
}
