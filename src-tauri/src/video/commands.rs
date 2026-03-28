use crate::db::Database;
use std::path::Path;
use tauri::AppHandle;

use super::service::{
    clear_video_scrape_data, copy_dir_recursive, delete_video_and_files,
    enrich_videos_with_file_times, ensure_video_in_own_dir_with_db, move_file,
    update_all_directories_count, AdVideo, VideoUpdateContext, VideoUpdatePayload,
    VideoUpdateResult, build_nfo_metadata_for_update, load_video_relation_names,
    parse_name_list,
};

// ==================== 目录管理 ====================

#[tauri::command]
pub async fn get_directories(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    // 确保 directories 表存在
    conn.execute(
        "CREATE TABLE IF NOT EXISTS directories (
            id TEXT PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            video_count INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .map_err(|e| format!("创建 directories 表失败: {}", e))?;

    let mut stmt = conn
        .prepare("SELECT id, path, video_count, created_at, updated_at FROM directories ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let path: String = row.get(1)?;
            let count: i64 = row.get(2)?;
            let created_at: String = row.get(3)?;
            let updated_at: String = row.get(4)?;
            Ok(serde_json::json!({
                "id": id,
                "path": path,
                "videoCount": count,
                "createdAt": created_at,
                "updatedAt": updated_at
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut dirs = Vec::new();
    for r in rows {
        dirs.push(r.map_err(|e| e.to_string())?);
    }
    Ok(dirs)
}

#[tauri::command]
pub async fn add_directory(app: AppHandle, path: String) -> Result<String, String> {
    use uuid::Uuid;

    if crate::scanner::file_scanner::is_skipped_directory(Path::new(&path)) {
        return Err("该目录已被系统忽略，不能添加：behind the scenes / backdrops".to_string());
    }

    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM directories WHERE path = ?",
            [&path],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        return Err("目录已存在".to_string());
    }

    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO directories (id, path, video_count) VALUES (?, ?, 0)",
        rusqlite::params![&id, &path],
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub async fn delete_directory(app: AppHandle, id: String) -> Result<(), String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let path: String = conn
        .query_row("SELECT path FROM directories WHERE id = ?", [&id], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    let normalized_path = Path::new(&path).to_string_lossy().replace('\\', "/");

    let path_pattern = if normalized_path.ends_with('/') {
        format!("{}%", normalized_path)
    } else {
        format!("{}/%", normalized_path)
    };

    conn.execute(
        "DELETE FROM videos WHERE
            dir_path = ? OR
            dir_path = ? OR
            REPLACE(dir_path, '\\', '/') LIKE ? OR
            REPLACE(dir_path, '\\', '/') = ?",
        rusqlite::params![&path, &normalized_path, &path_pattern, &normalized_path],
    )
    .map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM directories WHERE id = ?", [&id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ==================== 视频管理 ====================

#[tauri::command]
pub async fn get_videos(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let videos = {
        let db = Database::new(&app).map_err(|e| e.to_string())?;
        let conn = db.get_connection().map_err(|e| e.to_string())?;

        let sql = r#"
            SELECT
                v.id,
                v.title,
                v.video_path,
                v.studio,
                v.premiered,
                v.rating,
                v.duration,
                v.created_at,
                v.scan_status,
                v.director,
                v.local_id,
                v.poster,
                v.thumb,
                v.fanart,
                v.original_title,
                (
                    SELECT GROUP_CONCAT(a.name, ', ')
                    FROM video_actors va
                    JOIN actors a ON va.actor_id = a.id
                    WHERE va.video_id = v.id
                    ORDER BY va.priority
                ) as actors,
                v.resolution,
                v.file_size,
                (
                    SELECT GROUP_CONCAT(t.name, ', ')
                    FROM video_tags vt
                    JOIN tags t ON vt.tag_id = t.id
                    WHERE vt.video_id = v.id
                ) as tags,
                (
                    SELECT GROUP_CONCAT(g.name, ', ')
                    FROM video_genres vg
                    JOIN genres g ON vg.genre_id = g.id
                    WHERE vg.video_id = v.id
                ) as genres,
                v.fast_hash
            FROM videos v
            ORDER BY v.created_at DESC
        "#;

        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

        let video_iter = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "title": row.get::<_, Option<String>>(1)?,
                    "videoPath": row.get::<_, String>(2)?,
                    "dirPath": std::path::Path::new(&row.get::<_, String>(2)?)
                        .parent()
                        .map(|path| path.to_string_lossy().to_string()),
                    "studio": row.get::<_, Option<String>>(3)?,
                    "premiered": row.get::<_, Option<String>>(4)?,
                    "rating": row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                    "duration": row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    "createdAt": row.get::<_, String>(7)?,
                    "scanStatus": row.get::<_, i32>(8)?,
                    "director": row.get::<_, Option<String>>(9)?,
                    "localId": row.get::<_, Option<String>>(10)?,
                    "poster": row.get::<_, Option<String>>(11)?,
                    "thumb": row.get::<_, Option<String>>(12)?,
                    "fanart": row.get::<_, Option<String>>(13)?,
                    "originalTitle": row.get::<_, Option<String>>(14)?,
                    "actors": row.get::<_, Option<String>>(15)?,
                    "resolution": row.get::<_, Option<String>>(16)?,
                    "fileSize": row.get::<_, Option<i64>>(17)?,
                    "tags": row.get::<_, Option<String>>(18)?,
                    "genres": row.get::<_, Option<String>>(19)?,
                    "fastHash": row.get::<_, Option<String>>(20)?,
                }))
            })
            .map_err(|e| e.to_string())?;

        let mut videos = Vec::new();
        for video in video_iter {
            videos.push(video.map_err(|e| e.to_string())?);
        }

        videos
    };

    Ok(enrich_videos_with_file_times(videos).await)
}

#[tauri::command]

pub async fn get_duplicate_videos(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    // 跨目录查找 fast_hash 重复或 local_id（番号）重复的视频
    let sql = r#"
        SELECT
            v.id,
            v.title,
            v.video_path,
            v.dir_path,
            v.local_id,
            v.resolution,
            v.file_size,
            v.fast_hash,
            v.scan_status
        FROM videos v
        WHERE (v.fast_hash IS NOT NULL AND v.fast_hash != '' AND v.fast_hash IN (
            SELECT fast_hash FROM videos WHERE fast_hash IS NOT NULL AND fast_hash != '' GROUP BY fast_hash HAVING COUNT(*) > 1
        ))
        OR (v.local_id IS NOT NULL AND v.local_id != '' AND v.local_id IN (
            SELECT local_id FROM videos WHERE local_id IS NOT NULL AND local_id != '' GROUP BY local_id HAVING COUNT(*) > 1
        ))
        ORDER BY v.local_id, v.fast_hash, v.created_at DESC
    "#;

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let video_iter = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, Option<String>>(1)?,
                "videoPath": row.get::<_, String>(2)?,
                "dirPath": row.get::<_, Option<String>>(3)?,
                "localId": row.get::<_, Option<String>>(4)?,
                "resolution": row.get::<_, Option<String>>(5)?,
                "fileSize": row.get::<_, Option<i64>>(6)?,
                "fastHash": row.get::<_, Option<String>>(7)?,
                "scanStatus": row.get::<_, i32>(8)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut videos = Vec::new();
    for video in video_iter {
        videos.push(video.map_err(|e| e.to_string())?);
    }

    Ok(videos)
}

#[tauri::command]
pub async fn delete_video_db(app: AppHandle, id: String) -> Result<(), String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM videos WHERE id = ?", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_video_file(
    app: AppHandle,
    id: String,
    delete_scrape_data_only: Option<bool>,
) -> Result<(), String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;
    if delete_scrape_data_only.unwrap_or(false) {
        clear_video_scrape_data(&conn, &id)?;
    } else {
        delete_video_and_files(&conn, &id)?;
    }
    let _ = update_all_directories_count(&conn);
    Ok(())
}

#[tauri::command]

pub async fn move_video_file(app: AppHandle, id: String, target_dir: String) -> Result<(), String> {
    use std::fs;
    use std::path::Path;

    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    // 查询视频路径和同级图路径
    let (current_path, poster, thumb, fanart): (String, Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT video_path, poster, thumb, fanart FROM videos WHERE id = ?",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())?;

    let current_path_obj = Path::new(&current_path);
    if !current_path_obj.exists() {
        return Err("源视频文件不存在".to_string());
    }

    let file_name = current_path_obj.file_name().ok_or("无效的文件名")?;
    let new_path_obj = Path::new(&target_dir).join(file_name);

    if new_path_obj.exists() {
        return Err("目标目录已存在同名文件".to_string());
    }

    // 1. 移动视频文件
    move_file(current_path_obj, &new_path_obj).map_err(|e| format!("移动视频失败: {}", e))?;

    // 2. 移动 NFO 文件
    let current_nfo = current_path_obj.with_extension("nfo");
    if current_nfo.exists() {
        let new_nfo = new_path_obj.with_extension("nfo");
        let _ = move_file(&current_nfo, &new_nfo);
    }

    // 3. 移动同级图片资源
    let move_artwork = |path_opt: Option<String>, label: &str| -> Result<Option<String>, String> {
        if let Some(path) = path_opt {
            let source = Path::new(&path);
            if source.exists() {
                let file_name = source.file_name().ok_or_else(|| format!("无效的{}文件名", label))?;
                let target = Path::new(&target_dir).join(file_name);
                move_file(source, &target).map_err(|e| format!("移动{}失败: {}", label, e))?;
                return Ok(Some(target.to_string_lossy().to_string()));
            }
        }
        Ok(None)
    };
    let new_poster = move_artwork(poster.clone(), "poster")?;
    let new_thumb = move_artwork(thumb.clone(), "thumb")?;
    let new_fanart = move_artwork(fanart.clone(), "fanart")?;

    // 4. 移动 extrafanart 目录
    let old_parent = current_path_obj.parent().ok_or("无效的源路径")?;
    let extrafanart_dir = old_parent.join("extrafanart");
    if extrafanart_dir.exists() && extrafanart_dir.is_dir() {
        let target_extrafanart_dir = Path::new(&target_dir).join("extrafanart");
        copy_dir_recursive(&extrafanart_dir, &target_extrafanart_dir)
            .map_err(|e| format!("移动 extrafanart 目录失败: {}", e))?;
        let _ = fs::remove_dir_all(&extrafanart_dir);
    }

    // 5. 更新数据库
    let new_path_str = new_path_obj.to_string_lossy().to_string();
    conn.execute(
        "UPDATE videos SET video_path = ?, dir_path = ?, poster = ?, thumb = ?, fanart = ?, updated_at = datetime('now') WHERE id = ?",
        rusqlite::params![
            new_path_str,
            target_dir,
            new_poster.or(poster),
            new_thumb.or(thumb),
            new_fanart.or(fanart),
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_video(app: AppHandle, id: String, data: VideoUpdatePayload) -> Result<VideoUpdateResult, String> {
    // 确保视频在独立的同名目录中（避免重命名时影响其他视频的资源）
    if let Err(e) = ensure_video_in_own_dir_with_db(&app, &id) {
        eprintln!("[更新视频] 目录规范化失败: {}", e);
    }

    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let mut conn = db.get_connection().map_err(|e| e.to_string())?;

    let title_to_store = data.title.as_ref().map(|value| value.trim().to_string());

    if matches!(title_to_store.as_deref(), Some("")) {
        return Err("标题不能为空".to_string());
    }

    let current = conn
        .query_row(
            "SELECT title, original_title, local_id, studio, director, premiered, duration, rating, video_path, dir_path, poster, thumb, fanart FROM videos WHERE id = ?",
            [&id],
            |row| {
                Ok(VideoUpdateContext {
                    title: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    original_title: row.get(1)?,
                    local_id: row.get(2)?,
                    studio: row.get(3)?,
                    director: row.get(4)?,
                    premiered: row.get(5)?,
                    duration: row.get(6)?,
                    rating: row.get(7)?,
                    video_path: row.get(8)?,
                    dir_path: row.get(9)?,
                    poster: row.get(10)?,
                    thumb: row.get(11)?,
                    fanart: row.get(12)?,
                    actors: Vec::new(),
                    tags: Vec::new(),
                    genres: Vec::new(),
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let mut current = VideoUpdateContext {
        actors: load_video_relation_names(
            &conn,
            "SELECT a.name FROM video_actors va JOIN actors a ON va.actor_id = a.id WHERE va.video_id = ? ORDER BY va.priority",
            &id,
        )?,
        tags: load_video_relation_names(
            &conn,
            "SELECT t.name FROM video_tags vt JOIN tags t ON vt.tag_id = t.id WHERE vt.video_id = ? ORDER BY t.name",
            &id,
        )?,
        genres: load_video_relation_names(
            &conn,
            "SELECT g.name FROM video_genres vg JOIN genres g ON vg.genre_id = g.id WHERE vg.video_id = ? ORDER BY g.name",
            &id,
        )?,
        ..current
    };

    let mut parsed_duration = current.duration.map(|value| value as i32);
    let nfo_path = std::path::Path::new(&current.video_path).with_extension("nfo");
    let parsed_nfo = if nfo_path.exists() {
        crate::nfo::parser::parse_nfo(&nfo_path, &mut parsed_duration)
    } else {
        None
    };

    let updated_actors = data.actors.as_ref().map(|actors| parse_name_list(actors));
    let updated_tags = data.tags.as_ref().map(|tags| parse_name_list(tags));
    let rewritten_nfo_metadata = build_nfo_metadata_for_update(
        &current,
        &data,
        parsed_nfo.as_ref(),
        updated_actors.as_deref(),
        updated_tags.as_deref(),
    );
    let mut final_video_path = current.video_path.clone();
    let mut final_dir_path = current.dir_path.clone().or_else(|| {
        std::path::Path::new(&current.video_path)
            .parent()
            .map(|path| path.to_string_lossy().to_string())
    });
    let mut final_poster = current.poster.clone();
    let mut final_thumb = current.thumb.clone();
    let mut final_fanart = current.fanart.clone();

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 更新基本字段
    let mut sql_parts = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = &title_to_store {
        sql_parts.push("title = ?");
        params.push(Box::new(v.clone()) as Box<dyn rusqlite::ToSql>);
        current.title = v.clone();
    }
    if let Some(v) = &data.local_id {
        sql_parts.push("local_id = ?");
        params.push(Box::new(v.clone()) as Box<dyn rusqlite::ToSql>);
    }
    if let Some(v) = &data.duration {
        sql_parts.push("duration = ?");
        params.push(Box::new(*v as i64) as Box<dyn rusqlite::ToSql>);
    }
    if let Some(v) = &data.premiered {
        sql_parts.push("premiered = ?");
        params.push(Box::new(v.clone()) as Box<dyn rusqlite::ToSql>);
    }
    if let Some(v) = &data.rating {
        sql_parts.push("rating = ?");
        params.push(Box::new(*v) as Box<dyn rusqlite::ToSql>);
    }

    // 直接字符串字段（不再使用外键）
    if let Some(v) = &data.studio {
        sql_parts.push("studio = ?");
        params.push(Box::new(v.clone()) as Box<dyn rusqlite::ToSql>);
    }
    if let Some(v) = &data.director {
        sql_parts.push("director = ?");
        params.push(Box::new(v.clone()) as Box<dyn rusqlite::ToSql>);
    }
    if let Some(v) = &data.resolution {
        sql_parts.push("resolution = ?");
        params.push(Box::new(v.clone()) as Box<dyn rusqlite::ToSql>);
    }
    // maker 字段已不再使用

    sql_parts.push("updated_at = datetime('now')");

    if !sql_parts.is_empty() {
        let sql = format!("UPDATE videos SET {} WHERE id = ?", sql_parts.join(", "));
        params.push(Box::new(id.clone()));

        let mut stmt = tx.prepare(&sql).map_err(|e| e.to_string())?;
        stmt.execute(rusqlite::params_from_iter(params.iter()))
            .map_err(|e| e.to_string())?;
    }

    if let Some(actors) = &updated_actors {
        current.actors = actors.clone();

        tx.execute("DELETE FROM video_actors WHERE video_id = ?", [&id])
            .map_err(|e| e.to_string())?;

        for (idx, actor_name) in actors.iter().enumerate() {
            let actor_id = Database::get_or_create_actor(&tx, actor_name).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO video_actors (video_id, actor_id, priority) VALUES (?, ?, ?)",
                rusqlite::params![&id, actor_id, idx],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 处理标签（如果提供）
    if let Some(tags) = &updated_tags {
        current.tags = tags.clone();

        // 删除已有标签
        tx.execute("DELETE FROM video_tags WHERE video_id = ?", [&id])
            .map_err(|e| e.to_string())?;

        // 插入新标签
        for tag_name in tags.iter() {
            let tag_id = Database::get_or_create_tag(&tx, tag_name).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO video_tags (video_id, tag_id) VALUES (?, ?)",
                rusqlite::params![&id, tag_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some(title) = &title_to_store {
        if let Some(relocated) = crate::media::assets::rename_video_assets_with_title(
            &final_video_path,
            title,
            final_poster.as_deref(),
            final_thumb.as_deref(),
            final_fanart.as_deref(),
        )? {
            final_video_path = relocated.video_path;
            final_dir_path = Some(relocated.dir_path);
            final_poster = relocated.poster;
            final_thumb = relocated.thumb;
            final_fanart = relocated.fanart;

            Database::update_video_file_location_tx(
                &tx,
                &id,
                &relocated.original_video_path,
                &final_video_path,
                final_dir_path.as_deref().unwrap_or_default(),
                final_poster.as_deref(),
                final_thumb.as_deref(),
                final_fanart.as_deref(),
            )
            .map_err(|e| e.to_string())?;
        }
    }

    crate::media::assets::save_nfo_for_video(&final_video_path, &rewritten_nfo_metadata)?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(VideoUpdateResult {
        title: current.title,
        video_path: final_video_path,
        dir_path: final_dir_path,
        poster: final_poster,
        thumb: final_thumb,
        fanart: final_fanart,
    })
}

#[tauri::command]
pub async fn find_ad_videos(
    app: AppHandle,
    keywords: Option<Vec<String>>,
    check_duplicate: Option<bool>,
    exclude_keywords: Option<Vec<String>>,
) -> Result<Vec<AdVideo>, String> {
    use std::collections::HashMap;

    let check_duplicate = check_duplicate.unwrap_or(true);

    // 如果没有传入关键词，从设置中读取
    let settings = crate::settings::get_settings(app.clone()).await?;
    let keywords = keywords.unwrap_or(settings.ad_filter.keywords);
    let exclude_keywords = exclude_keywords.unwrap_or(settings.ad_filter.exclude_keywords);

    println!(
        "[find_ad_videos] 开始查找广告视频，关键词: {:?}, 排除关键词: {:?}, 检查重复: {}",
        keywords, exclude_keywords, check_duplicate
    );

    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;

    let mut ad_videos = Vec::new();

    // 第一步：查询所有视频（移除 50MB 限制）
    let mut stmt = conn
        .prepare("SELECT id, video_path, file_size FROM videos")
        .map_err(|e| e.to_string())?;

    let all_videos = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut video_list = Vec::new();
    for video in all_videos {
        let (id, path, size) = video.map_err(|e| e.to_string())?;
        video_list.push((id, path, size));
    }

    println!("[find_ad_videos] 找到 {} 个视频", video_list.len());

    // 第二步：统计文件名出现次数（在所有视频中统计）
    let mut filename_count: HashMap<String, Vec<String>> = HashMap::new();
    for (_, path, _) in &video_list {
        if let Some(filename) = std::path::Path::new(path).file_name() {
            let filename_str = filename.to_string_lossy().to_string();
            filename_count
                .entry(filename_str.clone())
                .or_insert_with(Vec::new)
                .push(path.clone());
        }
    }

    // 第三步：检查每个视频
    for (id, path, size) in video_list {
        let filename = std::path::Path::new(&path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut reasons = Vec::new();

        // 规则1: 文件大小为0（优先级最高）
        if size == 0 {
            reasons.push("文件大小为 0".to_string());
        } else {
            // 规则2: 文件名重复2次及以上
            if check_duplicate {
                if let Some(count) = filename_count.get(&filename) {
                    if count.len() >= 2 {
                        reasons.push(format!("文件名重复 {} 次", count.len()));
                    }
                }
            }

            // 规则3: 关键词过滤
            for keyword in &keywords {
                if filename.to_lowercase().contains(&keyword.to_lowercase()) {
                    reasons.push(format!("包含关键词: {}", keyword));
                    break;
                }
            }
        }

        // 如果有任何匹配的原因，添加到结果
        // 但如果文件名包含排除关键词，则跳过
        if !reasons.is_empty() {
            let filename_lower = filename.to_lowercase();
            let excluded = exclude_keywords
                .iter()
                .any(|ek| filename_lower.contains(&ek.to_lowercase()));
            if !excluded {
                ad_videos.push(AdVideo {
                    id,
                    path: path.clone(),
                    filename,
                    file_size: size,
                    reason: reasons.join(", "),
                });
            }
        }
    }

    println!("[find_ad_videos] 找到 {} 个疑似广告视频", ad_videos.len());
    Ok(ad_videos)
}

/// 下载远程图片到 extrafanart 目录
#[tauri::command]
pub async fn download_remote_image(
    _app: AppHandle,
    _video_id: String,
    video_path: String,
    url: String,
) -> Result<String, String> {
    let video_path_obj = std::path::Path::new(&video_path);
    let save_dir = crate::media::assets::extrafanart_dir_for_video(video_path_obj)?;
    std::fs::create_dir_all(&save_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let next_index = crate::media::assets::next_extrafanart_index(video_path_obj);
    let save_path = save_dir.join(format!("fanart{}.jpg", next_index));
    let client = crate::utils::proxy::apply_proxy_auto(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .use_rustls_tls()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"),
    )
    .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?
    .build()
    .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(&url)
        .header(
            "Accept",
            "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
        )
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Referer", "https://memojav.com/")
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("下载失败，HTTP 状态码: {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取数据失败: {}", e))?;
    if bytes.is_empty() {
        return Err("下载的数据为空".to_string());
    }

    std::fs::write(&save_path, &bytes).map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(save_path.to_string_lossy().to_string())
}

// 批量删除视频（复用 delete_video_and_files）
#[tauri::command]
pub async fn delete_videos(
    app: AppHandle,
    ids: Vec<String>,
    delete_scrape_data_only: Option<bool>,
) -> Result<(), String> {
    let db = Database::new(&app).map_err(|e| e.to_string())?;
    let conn = db.get_connection().map_err(|e| e.to_string())?;
    let delete_scrape_data_only = delete_scrape_data_only.unwrap_or(false);

    for id in ids {
        let result = if delete_scrape_data_only {
            clear_video_scrape_data(&conn, &id)
        } else {
            delete_video_and_files(&conn, &id)
        };

        if let Err(e) = result {
            eprintln!("删除视频 {} 失败: {}", id, e);
        }
    }

    let _ = update_all_directories_count(&conn);

    Ok(())
}
