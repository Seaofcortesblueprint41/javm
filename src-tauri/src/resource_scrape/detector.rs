//! 已刮削视频检测器

use crate::db::Database;

/// 已刮削视频检测器
///
/// 提供以下功能：
/// - 检查视频是否已刮削（scan_status = 2）
/// - 批量过滤已刮削视频
/// - 判断刮削时是否应跳过某视频
pub struct ScrapedVideoDetector<'a> {
    db: &'a Database,
}

impl<'a> ScrapedVideoDetector<'a> {
    /// 创建新的已刮削视频检测器
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// 检查视频是否已刮削（scan_status = 2）
    pub fn is_video_scraped(&self, video_path: &str) -> Result<bool, String> {
        let conn = self.db.get_connection().map_err(|e| e.to_string())?;

        let scan_status: Option<i32> = Database::get_video_scan_status_by_path(&conn, video_path)
            .map_err(|e| e.to_string())?;

        Ok(scan_status == Some(2))
    }
}
