use crate::db::Database;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

fn upsert_downloaded_video_record(conn: &rusqlite::Connection, video_path: &std::path::Path) -> Result<(), String> {
    let video_path_str = video_path.to_string_lossy().to_string();

    let exists = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM videos WHERE video_path = ?",
            rusqlite::params![video_path_str],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|e| format!("查询视频记录失败: {}", e))?;

    if exists {
        return Ok(());
    }

    let file_stem = video_path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("unknown");
    let dir_path = video_path
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_size = video_path
        .metadata()
        .map(|metadata| metadata.len() as i64)
        .unwrap_or(0);
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO videos (
            id, local_id, title, original_title, video_path, dir_path,
            file_size, scan_status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            Uuid::new_v4().to_string(),
            file_stem,
            file_stem,
            file_stem,
            video_path_str,
            dir_path,
            file_size,
            1,
            now,
            now,
        ],
    )
    .map_err(|e| format!("插入下载视频记录失败: {}", e))?;

    Ok(())
}

fn update_directory_count_for_video(conn: &rusqlite::Connection, video_path: &std::path::Path) -> Result<(), String> {
    let video_path_str = video_path.to_string_lossy().replace('\\', "/");

    let mut stmt = conn
        .prepare("SELECT path FROM directories")
        .map_err(|e| format!("查询目录失败: {}", e))?;
    let directories = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("读取目录列表失败: {}", e))?;

    for directory in directories {
        let directory = directory.map_err(|e| format!("解析目录记录失败: {}", e))?;
        let normalized = directory.replace('\\', "/").trim_end_matches('/').to_string();
        if video_path_str == normalized || video_path_str.starts_with(&(normalized.clone() + "/")) {
            let count = Database::count_videos_in_directory(conn, &directory)
                .map_err(|e| format!("统计目录视频数量失败: {}", e))?;
            Database::update_directory_video_count(conn, &directory, count)
                .map_err(|e| format!("更新目录视频数量失败: {}", e))?;
            break;
        }
    }

    Ok(())
}

fn rename_if_exists(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }

    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    std::fs::rename(from, to).map_err(|e| {
        format!(
            "重命名文件失败: {} -> {}: {}",
            from.display(),
            to.display(),
            e
        )
    })
}

fn cleanup_empty_dir(path: &std::path::Path, stop_at: &std::path::Path) {
    let mut current = path.to_path_buf();

    while current.exists() && !crate::download::is_same_path(&current, stop_at) {
        let is_empty = std::fs::read_dir(&current)
            .ok()
            .and_then(|mut iter| iter.next().transpose().ok())
            .is_none();

        if !is_empty {
            break;
        }

        if std::fs::remove_dir(&current).is_err() {
            break;
        }

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
}

fn rename_completed_download_files(
    save_path: &str,
    old_filename: &str,
    new_filename: &str,
) -> Result<(), String> {
    let old_video_path = crate::download::find_existing_video_path(save_path, old_filename)
        .ok_or_else(|| format!("未找到已下载文件: {}", old_filename))?;
    let old_parent_dir = old_video_path.parent().ok_or("无效的视频路径")?.to_path_buf();
    let new_parent_dir = crate::download::resolve_task_save_dir(save_path, Some(new_filename));

    std::fs::create_dir_all(&new_parent_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let new_video_name = match old_video_path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{}.{}", new_filename, ext),
        _ => new_filename.to_string(),
    };
    let new_video_path = new_parent_dir.join(new_video_name);
    std::fs::rename(&old_video_path, &new_video_path).map_err(|e| {
        format!(
            "重命名视频文件失败: {} -> {}: {}",
            old_video_path.display(),
            new_video_path.display(),
            e
        )
    })?;

    let old_nfo = old_parent_dir.join(format!("{}.nfo", old_filename));
    let new_nfo = new_parent_dir.join(format!("{}.nfo", new_filename));
    rename_if_exists(&old_nfo, &new_nfo)?;

    let old_poster = old_parent_dir.join(format!("{}-poster.jpg", old_filename));
    let new_poster = new_parent_dir.join(format!("{}-poster.jpg", new_filename));
    rename_if_exists(&old_poster, &new_poster)?;

    let old_assets_dir = old_parent_dir.join(old_filename);
    let new_assets_dir = new_parent_dir.join(new_filename);
    if old_assets_dir.exists() {
        if new_assets_dir.exists() {
            std::fs::remove_dir_all(&new_assets_dir)
                .map_err(|e| format!("清理旧资源目录失败: {}", e))?;
        }
        std::fs::rename(&old_assets_dir, &new_assets_dir).map_err(|e| {
            format!(
                "重命名资源目录失败: {} -> {}: {}",
                old_assets_dir.display(),
                new_assets_dir.display(),
                e
            )
        })?;
    }

    cleanup_empty_dir(&old_parent_dir, std::path::Path::new(save_path));

    Ok(())
}

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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
pub async fn sync_completed_download_to_library(app: AppHandle, task_id: String) -> Result<bool, String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let (save_path, filename): (String, Option<String>) = conn
        .query_row(
            "SELECT save_path, filename FROM downloads WHERE id = ?",
            rusqlite::params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("读取下载任务失败: {}", e))?;

    let Some(filename) = filename else {
        return Ok(false);
    };

    let Some(video_path) = crate::download::find_existing_video_path(&save_path, &filename) else {
        return Ok(false);
    };

    upsert_downloaded_video_record(&conn, &video_path)?;
    update_directory_count_for_video(&conn, &video_path)?;

    Ok(true)
}

#[tauri::command]
pub async fn add_download_task(
    app: AppHandle,
    url: String,
    save_path: String,
    filename: Option<String>,
) -> Result<String, String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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

    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
        6 => {
            // Completed: rename actual files + update DB
            if let Some(old_name) = &old_filename {
                rename_completed_download_files(&save_path, old_name, &new_filename)?;
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
            // Not started (0, 1) or Paused/Failed/Cancelled (4, 5, 7, 8, 9): just update DB
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
    let db = Database::new(&app).map_err(|e| e.to_string())?;
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
        6 => {
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
