//! 视频媒体资源管理模块
//!
//! 统一处理视频相关的媒体资源操作，包括：
//! - NFO 元数据文件保存
//! - 封面图片下载/截取保存
//! - 预览缩略图下载
//! - 视频帧截取（ffmpeg）
//! - 预览截图保存
//! - 文件回滚

use std::fs;
use std::path::Path;

use crate::nfo::generator::NfoGenerator;
use crate::resource_scrape::types::ScrapeMetadata;

// ============================================================
// NFO 元数据
// ============================================================

/// 统一的 NFO 保存逻辑：检查本地封面是否存在，然后调用 NfoGenerator 生成 NFO 文件
///
/// 供 queue_manager、commands 等模块复用，避免重复实现。
pub fn save_nfo_for_video(video_path: &str, metadata: &ScrapeMetadata) -> Result<(), String> {
    let path = Path::new(video_path);
    let generator = NfoGenerator::new();

    let parent_dir = path.parent().ok_or("无效的视频路径")?;
    let file_stem = path
        .file_stem()
        .ok_or("无效的视频文件名")?
        .to_string_lossy();

    let poster_filename = format!("{}-poster.jpg", file_stem);
    let poster_path = parent_dir.join(&poster_filename);
    let local_poster = if poster_path.exists() {
        Some(poster_filename.as_str())
    } else {
        None
    };

    generator.save(metadata, path, local_poster).map(|_| ())
}

// ============================================================
// 封面图片
// ============================================================

/// 将截取的视频帧保存为封面图片
///
/// # 参数
/// * `video_path` - 视频文件路径
/// * `frame_path` - 截取的帧图片路径
///
/// # 返回
/// * `Ok(String)` - 保存的封面图片路径
/// * `Err(String)` - 保存失败的错误信息
pub fn save_frame_as_cover(video_path: &str, frame_path: &str) -> Result<String, String> {
    let video_path_obj = Path::new(video_path);
    let parent_dir = video_path_obj.parent().ok_or("无效的视频路径")?;
    let file_stem = video_path_obj
        .file_stem()
        .ok_or("无效的文件名")?
        .to_string_lossy();

    let cover_filename = format!("{}-poster.jpg", file_stem);
    let cover_path = parent_dir.join(&cover_filename);

    fs::copy(frame_path, &cover_path).map_err(|e| format!("保存封面失败: {}", e))?;

    Ok(cover_path.to_string_lossy().to_string())
}

// ============================================================
// 预览缩略图 / 截图
// ============================================================

/// 从 URL 列表下载预览缩略图并保存到以视频文件名命名的子目录
///
/// # 目录结构
/// 缩略图保存在以视频文件名（不含扩展名）命名的子目录中
/// 例如：视频为 `/path/to/ABC-123.mp4`，缩略图将保存在 `/path/to/ABC-123/thumb_001.jpg`
///
/// # 错误处理
/// 单个缩略图下载失败会记录日志但不会中断整个过程
pub async fn download_preview_thumbs(
    video_path: &str,
    thumb_urls: &[String],
) -> Result<Vec<String>, String> {
    if thumb_urls.is_empty() {
        return Ok(Vec::new());
    }

    if video_path.trim().is_empty() {
        return Err("视频路径不能为空".to_string());
    }

    let video_path = Path::new(video_path);
    let parent_dir = video_path.parent().ok_or("无效的视频路径")?;
    let file_stem = video_path
        .file_stem()
        .ok_or("无效的文件名")?
        .to_string_lossy();

    let thumbs_dir = parent_dir.join(file_stem.as_ref());

    crate::download::image::download_images_batch(thumb_urls, &thumbs_dir, "thumb", None, None)
        .await
}

/// 将截取的多个视频帧保存为预览截图
///
/// # 参数
/// * `video_path` - 视频文件路径
/// * `frame_paths` - 截取的帧图片路径列表
///
/// # 返回
/// * `Ok(Vec<String>)` - 保存的截图路径列表
/// * `Err(String)` - 保存失败的错误信息
pub fn save_frames_as_screenshots(
    video_path: &str,
    frame_paths: &[String],
) -> Result<Vec<String>, String> {
    let video_path_obj = Path::new(video_path);
    let parent_dir = video_path_obj.parent().ok_or("无效的视频路径")?;
    let file_stem = video_path_obj
        .file_stem()
        .ok_or("无效的文件名")?
        .to_string_lossy();

    // 保存到以视频文件名命名的子目录，与 download_preview_thumbs 保持一致
    let screenshots_dir = parent_dir.join(file_stem.as_ref());
    fs::create_dir_all(&screenshots_dir).map_err(|e| format!("创建截图目录失败: {}", e))?;

    let mut screenshot_paths = Vec::new();

    for (idx, frame_path) in frame_paths.iter().enumerate() {
        let screenshot_filename = format!("screenshot-{}.jpg", idx + 1);
        let screenshot_path = screenshots_dir.join(&screenshot_filename);

        fs::copy(frame_path, &screenshot_path)
            .map_err(|e| format!("保存预览图 {} 失败: {}", idx + 1, e))?;

        screenshot_paths.push(screenshot_path.to_string_lossy().to_string());
    }

    Ok(screenshot_paths)
}

// ============================================================
// 视频帧截取 (ffmpeg)
// ============================================================

/// 从视频中随机截取指定数量的帧
///
/// 将视频时长均匀分段，在每段内随机选择时间点，覆盖 0%~100% 范围。
/// 需要系统安装 ffmpeg。
///
/// # 参数
/// * `video_path` - 视频文件路径
/// * `count` - 要截取的帧数量
// 已抽离至 crate::utils::ffmpeg

// ============================================================
// 文件回滚
// ============================================================

/// 回滚文件操作，删除已创建的文件
///
/// 当数据库操作失败时调用此函数，以确保文件系统和数据库之间的数据一致性
#[allow(dead_code)]
pub fn rollback_files(
    nfo_path: Option<&std::path::PathBuf>,
    cover_path: Option<&str>,
    thumbs_dir: Option<&std::path::PathBuf>,
) {
    if let Some(nfo) = nfo_path {
        if nfo.exists() {
            match fs::remove_file(nfo) {
                Ok(_) => println!("回滚: 已删除 NFO 文件: {:?}", nfo),
                Err(e) => eprintln!("回滚: 删除 NFO 文件失败 {:?}: {}", nfo, e),
            }
        }
    }

    if let Some(cover) = cover_path {
        if !cover.trim().is_empty() {
            let cover_path_obj = Path::new(cover);
            if cover_path_obj.exists() {
                match fs::remove_file(cover_path_obj) {
                    Ok(_) => println!("回滚: 已删除封面图片: {}", cover),
                    Err(e) => eprintln!("回滚: 删除封面图片失败 {}: {}", cover, e),
                }
            }
        }
    }

    if let Some(thumbs) = thumbs_dir {
        if thumbs.exists() {
            match fs::remove_dir_all(thumbs) {
                Ok(_) => println!("回滚: 已删除缩略图目录: {:?}", thumbs),
                Err(e) => eprintln!("回滚: 删除缩略图目录失败 {:?}: {}", thumbs, e),
            }
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_rollback_files_deletes_nfo() {
        let temp_dir = std::env::temp_dir();
        let nfo_path = temp_dir.join("test_video.nfo");

        let mut file = fs::File::create(&nfo_path).unwrap();
        file.write_all(b"test nfo content").unwrap();
        drop(file);

        assert!(nfo_path.exists());
        rollback_files(Some(&nfo_path), None, None);
        assert!(!nfo_path.exists());
    }

    #[test]
    fn test_rollback_files_deletes_cover() {
        let temp_dir = std::env::temp_dir();
        let cover_path = temp_dir.join("test_video-poster.jpg");

        let mut file = fs::File::create(&cover_path).unwrap();
        file.write_all(b"fake image data").unwrap();
        drop(file);

        assert!(cover_path.exists());
        let cover_str = cover_path.to_string_lossy().to_string();
        rollback_files(None, Some(&cover_str), None);
        assert!(!cover_path.exists());
    }

    #[test]
    fn test_rollback_files_deletes_thumbs_directory() {
        let temp_dir = std::env::temp_dir();
        let thumbs_dir = temp_dir.join("test_thumbs");
        fs::create_dir_all(&thumbs_dir).unwrap();

        for i in 1..=3 {
            let thumb_path = thumbs_dir.join(format!("thumb_{:03}.jpg", i));
            let mut file = fs::File::create(&thumb_path).unwrap();
            file.write_all(b"fake thumb data").unwrap();
        }

        assert!(thumbs_dir.exists());
        assert_eq!(fs::read_dir(&thumbs_dir).unwrap().count(), 3);

        rollback_files(None, None, Some(&thumbs_dir));
        assert!(!thumbs_dir.exists());
    }

    #[test]
    fn test_rollback_files_deletes_all() {
        let temp_dir = std::env::temp_dir();
        let nfo_path = temp_dir.join("test_all.nfo");
        let cover_path = temp_dir.join("test_all-poster.jpg");
        let thumbs_dir = temp_dir.join("test_all_thumbs");

        fs::File::create(&nfo_path)
            .unwrap()
            .write_all(b"nfo")
            .unwrap();
        fs::File::create(&cover_path)
            .unwrap()
            .write_all(b"cover")
            .unwrap();
        fs::create_dir_all(&thumbs_dir).unwrap();
        fs::File::create(thumbs_dir.join("thumb_001.jpg"))
            .unwrap()
            .write_all(b"thumb")
            .unwrap();

        assert!(nfo_path.exists());
        assert!(cover_path.exists());
        assert!(thumbs_dir.exists());

        let cover_str = cover_path.to_string_lossy().to_string();
        rollback_files(Some(&nfo_path), Some(&cover_str), Some(&thumbs_dir));

        assert!(!nfo_path.exists());
        assert!(!cover_path.exists());
        assert!(!thumbs_dir.exists());
    }

    #[test]
    fn test_rollback_files_handles_nonexistent_files() {
        let temp_dir = std::env::temp_dir();
        let nonexistent_nfo = temp_dir.join("nonexistent.nfo");
        let nonexistent_cover = temp_dir.join("nonexistent-poster.jpg");
        let nonexistent_thumbs = temp_dir.join("nonexistent_thumbs");

        assert!(!nonexistent_nfo.exists());
        assert!(!nonexistent_cover.exists());
        assert!(!nonexistent_thumbs.exists());

        let cover_str = nonexistent_cover.to_string_lossy().to_string();
        rollback_files(
            Some(&nonexistent_nfo),
            Some(&cover_str),
            Some(&nonexistent_thumbs),
        );

        assert!(!nonexistent_nfo.exists());
        assert!(!nonexistent_cover.exists());
        assert!(!nonexistent_thumbs.exists());
    }
}
