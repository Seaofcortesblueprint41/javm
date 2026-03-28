use crate::db::Database;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

/// 视频截图任务的取消令牌管理
pub struct CaptureState {
    pub cancel_token: Mutex<Option<tokio_util::sync::CancellationToken>>,
}

#[tauri::command]
pub async fn capture_video_frames(
    app: AppHandle,
    state: State<'_, CaptureState>,
    video_path: String,
    count: usize,
) -> Result<Vec<String>, String> {
    // 取消之前可能还在运行的截图任务，并创建新的取消令牌
    let token = {
        let mut token_guard = state.cancel_token.lock().await;
        if let Some(old_token) = token_guard.take() {
            old_token.cancel();
        }
        let new_token = tokio_util::sync::CancellationToken::new();
        let cloned = new_token.clone();
        *token_guard = Some(new_token);
        cloned
    };

    // 使用流式截图：每成功一帧就通过事件推送给前端
    let result =
        super::ffmpeg::capture_random_frames_streaming(&app, &video_path, count, token).await;

    result
}

#[tauri::command]
pub async fn cancel_capture(state: State<'_, CaptureState>) -> Result<(), String> {
    let mut token_guard = state.cancel_token.lock().await;
    if let Some(token) = token_guard.take() {
        token.cancel();
    }
    Ok(())
}

/// 删除封面：删除本地文件 + 清空数据库中的封面字段
#[tauri::command]
pub async fn delete_cover(app: AppHandle, video_id: String) -> Result<(), String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    // 查询当前封面路径
    let (poster, thumb): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT poster, thumb FROM videos WHERE id = ?",
            [&video_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    // 删除本地封面文件
    if let Some(ref path) = poster {
        let p = std::path::Path::new(path);
        if p.exists() {
            std::fs::remove_file(p).map_err(|e| e.to_string())?;
        }
    }

    if let Some(ref path) = thumb {
        if poster.as_deref() != Some(path.as_str()) {
            let p = std::path::Path::new(path);
            if p.exists() {
                std::fs::remove_file(p).map_err(|e| e.to_string())?;
            }
        }
    }

    // 清空数据库中的封面字段
    conn.execute(
        "UPDATE videos SET poster = NULL, thumb = NULL, updated_at = datetime('now') WHERE id = ?",
        rusqlite::params![&video_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCapturedCoverResult {
    thumb_path: String,
    video_path: String,
}

#[tauri::command]
pub async fn save_captured_cover(
    app: AppHandle,
    video_id: String,
    video_path: String,
    frame_path: String,
) -> Result<SaveCapturedCoverResult, String> {
    // 确保视频在独立的同名目录中（避免多个视频共享 extrafanart 等资源目录）
    let actual_video_path = crate::video::service::ensure_video_in_own_dir_with_db(&app, &video_id)
        .unwrap_or_else(|e| {
            eprintln!("[目录规范化] 预检查失败，使用原路径: {}", e);
            video_path.clone()
        });

    // 保存帧作为封面资源（poster + thumb）
    let (poster_path, thumb_path) =
        super::assets::save_frame_as_cover_assets(&actual_video_path, &frame_path)?;

    // 更新数据库
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE videos SET poster = ?, thumb = ?, updated_at = datetime('now') WHERE id = ?",
        rusqlite::params![&poster_path, &thumb_path, &video_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(SaveCapturedCoverResult {
        thumb_path,
        video_path: actual_video_path,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCapturedThumbsResult {
    thumb_paths: Vec<String>,
    video_path: String,
}

#[tauri::command]
pub async fn save_captured_thumbs(
    app: AppHandle,
    video_id: String,
    video_path: String,
    frame_paths: Vec<String>,
) -> Result<SaveCapturedThumbsResult, String> {
    // 确保视频在独立的同名目录中（避免多个视频共享 extrafanart 目录）
    let actual_video_path = crate::video::service::ensure_video_in_own_dir_with_db(&app, &video_id)
        .unwrap_or_else(|e| {
            eprintln!("[目录规范化] 预检查失败，使用原路径: {}", e);
            video_path.clone()
        });

    // 保存多个帧作为预览图
    let thumb_paths =
        super::assets::save_frames_to_extrafanart(&actual_video_path, &frame_paths)?;

    Ok(SaveCapturedThumbsResult {
        thumb_paths,
        video_path: actual_video_path,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoPreviewImageSource {
    src: String,
    local_path: Option<String>,
    remote_url: Option<String>,
}

#[tauri::command]
pub async fn resolve_video_preview_images(video_path: String) -> Result<Vec<VideoPreviewImageSource>, String> {
    use std::collections::{BTreeMap, HashSet};
    use std::path::Path;

    if video_path.trim().is_empty() {
        return Ok(Vec::new());
    }

    let video_path_obj = Path::new(&video_path);
    let mut duration = None;
    let nfo_path = video_path_obj.with_extension("nfo");
    let remote_thumb_urls = if nfo_path.exists() {
        crate::nfo::parser::parse_nfo(&nfo_path, &mut duration)
            .map(|data| data.thumb_urls)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let extrafanart_map = crate::media::assets::collect_extrafanart_paths(video_path_obj)
        .into_iter()
        .collect::<BTreeMap<usize, String>>();
    let mut items = Vec::new();
    let mut used_local_paths = HashSet::new();
    let mut missing_remote_images = Vec::new();

    for (index, remote_url) in remote_thumb_urls.into_iter().enumerate() {
        let file_index = index + 1;
        if let Some(local_path) = extrafanart_map.get(&file_index) {
            used_local_paths.insert(local_path.clone());
            items.push(VideoPreviewImageSource {
                src: local_path.clone(),
                local_path: Some(local_path.clone()),
                remote_url: Some(remote_url),
            });
        } else {
            let remote_url = remote_url.trim().to_string();
            if remote_url.is_empty() {
                continue;
            }
            missing_remote_images.push((file_index, remote_url.clone()));
            items.push(VideoPreviewImageSource {
                src: remote_url.clone(),
                local_path: None,
                remote_url: Some(remote_url),
            });
        }
    }

    for (_, local_path) in extrafanart_map {
        if used_local_paths.insert(local_path.clone()) {
            items.push(VideoPreviewImageSource {
                src: local_path.clone(),
                local_path: Some(local_path),
                remote_url: None,
            });
        }
    }

    if !missing_remote_images.is_empty() {
        let background_video_path = video_path.clone();
        tauri::async_runtime::spawn(async move {
            let _ = crate::media::assets::sync_extrafanart_from_urls(
                &background_video_path,
                missing_remote_images,
            )
            .await;
        });
    }

    Ok(items)
}

/// 删除单个预览图文件
#[tauri::command]
pub async fn delete_thumb(
    _app: AppHandle,
    _video_id: String,
    thumb_path: String,
) -> Result<(), String> {
    // 删除本地截图文件
    let p = std::path::Path::new(&thumb_path);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn clear_thumbs(
    _app: AppHandle,
    _video_id: String,
    video_path: String,
) -> Result<(), String> {
    // 删除 extrafanart 中的预览图文件
    let video_path_obj = std::path::Path::new(&video_path);
    let extrafanart_dir = crate::media::assets::extrafanart_dir_for_video(video_path_obj)?;

    if extrafanart_dir.exists() && extrafanart_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&extrafanart_dir) {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.to_ascii_lowercase().starts_with("fanart") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        if let Ok(mut entries) = std::fs::read_dir(&extrafanart_dir) {
            if entries.next().is_none() {
                let _ = std::fs::remove_dir(&extrafanart_dir);
            }
        }
    }

    Ok(())
}
