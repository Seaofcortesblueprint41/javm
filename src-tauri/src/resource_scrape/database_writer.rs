//! 数据库写入器 - 负责将刮削的视频元数据写入数据库

use crate::db::Database;
use crate::resource_scrape::types::ScrapeMetadata;
use std::path::PathBuf;

/// 数据库写入器 - 负责将刮削的视频元数据写入数据库
///
/// 提供的功能：
/// - 更新视频元数据（标题、番号、发行日期等）
/// - 保存演员到关联表
/// - 保存标签到关联表
/// - 更新刮削状态和时间戳
pub struct DatabaseWriter {
    db_path: PathBuf,
}

impl DatabaseWriter {
    /// 创建新的数据库写入器实例
    pub fn new(db: &Database) -> Self {
        Self {
            db_path: db.get_database_path().clone(),
        }
    }

    /// 更新视频元数据
    ///
    /// 更新内容包括：标题、制作商、导演、发行日期、时长、评分、封面图、截图、番号等
    ///
    /// # 参数
    /// * `video_id` - 视频ID
    /// * `metadata` - 刮削得到的元数据
    /// * `local_cover_image` - 本地封面图路径
    /// * `remote_cover_image` - 远程封面图URL
    pub async fn update_video_metadata(
        &self,
        video_id: String,
        metadata: ScrapeMetadata,
        local_cover_image: String,
        remote_cover_image: String,
    ) -> Result<(), String> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;

            // 检查数据库中现有的时长
            let existing_duration: Option<i32> =
                Database::get_video_duration(&conn, &video_id).map_err(|e| e.to_string())?;

            // 如果数据库时长为0或NULL，则使用刮削得到的时长（刮削器返回分钟，数据库存储秒）
            let new_duration = if existing_duration.unwrap_or(0) == 0 {
                metadata.duration.map(|d| (d * 60) as i32)
            } else {
                existing_duration
            };
            // 序列化截图为JSON
            let screenshots_json =
                serde_json::to_string(&metadata.screenshots).unwrap_or_else(|_| "[]".to_string());

            Database::update_video_scrape_info(
                &conn,
                &video_id,
                &metadata.title,
                metadata.original_title.as_deref(),
                Some(metadata.studio.as_str()),
                Some(metadata.director.as_str()),
                Some(metadata.premiered.as_str()),
                new_duration,
                metadata.score,
                &local_cover_image,
                &remote_cover_image,
                &screenshots_json,
                Some(metadata.local_id.as_str()),
            )
            .map_err(|e| e.to_string())?;

            Ok(())
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// 保存演员到关联表
    ///
    /// 操作流程：
    /// 1. 删除该视频现有的演员关联
    /// 2. 为每个演员名创建或获取演员ID
    /// 3. 插入新的关联记录（按顺序设置优先级）
    pub async fn save_actors(&self, video_id: String, actors: Vec<String>) -> Result<(), String> {
        if actors.is_empty() {
            return Ok(());
        }

        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let mut conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;
            let transaction = conn.transaction().map_err(|e| e.to_string())?;

            Database::clear_video_actors(&transaction, &video_id).map_err(|e| e.to_string())?;

            for (idx, actor_name) in actors.iter().enumerate() {
                let actor_id = Database::get_or_create_actor(&transaction, actor_name)
                    .map_err(|e| e.to_string())?;
                Database::add_video_actor(&transaction, &video_id, actor_id, idx)
                    .map_err(|e| e.to_string())?;
            }

            transaction.commit().map_err(|e| e.to_string())?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// 保存标签到关联表
    ///
    /// 操作流程：
    /// 1. 删除该视频现有的标签关联
    /// 2. 为每个标签名创建或获取标签ID
    /// 3. 插入新的关联记录
    pub async fn save_tags(&self, video_id: String, tags: Vec<String>) -> Result<(), String> {
        if tags.is_empty() {
            return Ok(());
        }

        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let mut conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;
            let transaction = conn.transaction().map_err(|e| e.to_string())?;

            Database::clear_video_tags(&transaction, &video_id).map_err(|e| e.to_string())?;

            for tag_name in &tags {
                let tag_id = Database::get_or_create_tag(&transaction, tag_name)
                    .map_err(|e| e.to_string())?;
                Database::add_video_tag(&transaction, &video_id, tag_id)
                    .map_err(|e| e.to_string())?;
            }

            transaction.commit().map_err(|e| e.to_string())?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// 将所有刮削数据写入数据库
    ///
    /// 依次调用 update_video_metadata、save_actors、save_tags
    pub async fn write_all(
        &self,
        video_id: String,
        metadata: ScrapeMetadata,
        local_cover_image: String,
        remote_cover_image: String,
    ) -> Result<(), String> {
        let actors = metadata.actors.clone();
        let tags = metadata.tags.clone();

        self.update_video_metadata(
            video_id.clone(),
            metadata,
            local_cover_image,
            remote_cover_image,
        )
        .await?;

        self.save_actors(video_id.clone(), actors).await?;
        self.save_tags(video_id, tags).await?;

        Ok(())
    }
}
