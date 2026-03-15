//! 扫描相关的 Tauri 命令

use crate::db::Database;
use crate::scanner::{ScanProgress, ScannerService};
use std::path::Path;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub async fn scan_directory(app: AppHandle, path: String) -> Result<u32, String> {
    let db = Database::new(&app);
    let scanner = ScannerService::new(db.clone());
    let app_clone = app.clone();
    let app_clone2 = app.clone();

    // 异步扫描，带进度回调
    let count = scanner
        .scan_directory_async(&path, move |progress| {
            let _ = app_clone.emit("scan-progress", progress);
        })
        .await?;

    // 更新 directories 表中的视频数量
    let db_clone = db.clone();
    let path_clone = path.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let conn = db_clone.get_connection().map_err(|e| e.to_string())?;

        // 查找该路径对应的目录记录
        let dir_exists: bool =
            Database::check_directory_exists(&conn, &path_clone).map_err(|e| e.to_string())?;

        if dir_exists {
            // 规范化路径
            let normalized_path = Path::new(&path_clone).to_string_lossy().replace('\\', "/");

            let path_pattern = if normalized_path.ends_with('/') {
                format!("{}%", normalized_path)
            } else {
                format!("{}/%", normalized_path)
            };

            // 统计该目录及其子目录下的视频数量
            let video_count: i64 = Database::get_directory_video_count(
                &conn,
                &path_clone,
                &normalized_path,
                &path_pattern,
            )
            .map_err(|e| e.to_string())?;

            // 更新目录的视频数量和更新时间
            Database::update_directory_video_count(&conn, &path_clone, video_count)
                .map_err(|e| e.to_string())?;
        }

        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())??;

    // 发送扫描完成信号（null 进度）
    let _ = app_clone2.emit("scan-progress", Option::<ScanProgress>::None);

    Ok(count)
}
