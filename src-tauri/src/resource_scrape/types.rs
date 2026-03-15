//! 资源刮削系统类型定义

use serde::{Deserialize, Deserializer, Serialize};

/// 单个数据源的搜索结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    /// 番号
    pub code: String,
    /// 标题
    pub title: String,
    /// 演员（逗号分隔）
    pub actors: String,
    /// 时长（如 "120分钟"）
    pub duration: String,
    /// 制作商
    pub studio: String,
    /// 数据来源名称
    pub source: String,
    /// 封面图 URL
    #[serde(default, alias = "coverUrl")]
    pub cover_url: String,
    /// 导演
    #[serde(default)]
    pub director: String,
    /// 标签/分类（逗号分隔）
    #[serde(default)]
    pub tags: String,
    /// 发行日期
    #[serde(default)]
    pub premiered: String,
    /// 评分
    pub rating: Option<f64>,
    /// 预览截图 URL 列表
    #[serde(default)]
    pub screenshots: Vec<String>,
    /// 原始远程封面 URL（代理后保留，用于保存时下载）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_cover_url: Option<String>,
}

// ============================================================
// 从 scraper::types 迁移的类型
// ============================================================

/// 自定义反序列化：将 null 值转为空字符串
fn deserialize_null_as_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// 自定义反序列化：将 null 值转为空 Vec
fn deserialize_null_as_empty_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<Vec<String>>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// 刮削元数据 - 从网站提取的视频信息
///
/// 所有字段都有默认值，确保即使部分数据缺失也不会导致反序列化失败。
/// 这是爬虫系统的核心数据结构，被 HTTP 解析器、数据库写入器等模块共享。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapeMetadata {
    /// 视频标题
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub title: String,

    /// 番号（如 ABC-123）
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub local_id: String,

    /// 原始标题
    pub original_title: Option<String>,

    /// 制作商/片商
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub studio: String,

    /// 发行日期（YYYY-MM-DD 格式）
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub premiered: String,

    /// 时长（分钟）
    pub duration: Option<i64>,

    /// 封面图片 URL
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub poster_url: String,

    /// 演员列表
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub actors: Vec<String>,

    /// 导演
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub director: String,

    /// 评分（0-10）
    pub score: Option<f64>,

    /// 标签/类别列表
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub tags: Vec<String>,

    /// 预览截图 URL 列表
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub screenshots: Vec<String>,
}
