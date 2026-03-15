//! NFO 文件解析器
//!
//! 使用 quick_xml 事件驱动解析 Kodi/Emby/Jellyfin 兼容的 NFO 文件，
//! 提取视频元数据（标题、演员、标签等）。

use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;

/// 从 NFO 文件中解析出的元数据
pub struct NfoData {
    pub title: Option<String>,
    pub original_title: Option<String>,
    pub local_id: Option<String>,
    pub studio: Option<String>,
    pub director: Option<String>,
    pub premiered: Option<String>,
    pub rating: Option<f64>,
    pub remote_cover_url: Option<String>,
    pub actor_names: Vec<String>,
    pub tag_names: Vec<String>,
}

/// 使用 quick_xml 解析 NFO 文件内容，返回结构化元数据
///
/// `duration` 参数为可变引用：如果 NFO 中包含 runtime，会覆盖已有的时长值
pub fn parse_nfo(nfo_path: &Path, duration: &mut Option<i32>) -> Option<NfoData> {
    let content = match std::fs::read(nfo_path) {
        Ok(bytes) => {
            // 跳过 UTF-8 BOM（如果存在）
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                String::from_utf8_lossy(&bytes[3..]).to_string()
            } else {
                String::from_utf8_lossy(&bytes).to_string()
            }
        }
        Err(e) => {
            eprintln!("读取 NFO 文件失败 '{}': {}", nfo_path.display(), e);
            return None;
        }
    };

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut title: Option<String> = None;
    let mut original_title: Option<String> = None;
    let mut local_id: Option<String> = None;
    let mut studio: Option<String> = None;
    let mut director: Option<String> = None;
    let mut premiered: Option<String> = None;
    let mut year: Option<String> = None;
    let mut rating: Option<f64> = None;
    let mut remote_cover_url: Option<String> = None;
    let mut actor_names: Vec<String> = Vec::new();
    let mut tag_names: Vec<String> = Vec::new();

    // 当前标签名，用于跟踪嵌套（主要是 <actor><name>）
    let mut current_tag: Option<String> = None;
    let mut in_actor = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                match tag.as_str() {
                    "actor" => {
                        in_actor = true;
                        current_tag = None;
                    }
                    _ => {
                        current_tag = Some(tag);
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref tag) = current_tag {
                    let text = match e.xml10_content() {
                        Ok(cow) => cow.trim().to_string(),
                        Err(_) => continue,
                    };
                    if text.is_empty() {
                        continue;
                    }
                    match tag.as_str() {
                        "title" if !in_actor && title.is_none() => {
                            title = Some(text);
                        }
                        "originaltitle" if original_title.is_none() => {
                            original_title = Some(text);
                        }
                        // 番号从 uniqueid 标签获取
                        "uniqueid" => {
                            local_id = Some(text);
                        }
                        "studio" if studio.is_none() => {
                            studio = Some(text);
                        }
                        "premiered" if premiered.is_none() => {
                            premiered = Some(text);
                        }
                        "year" if year.is_none() => {
                            year = Some(text);
                        }
                        "director" if !in_actor && director.is_none() => {
                            director = Some(text);
                        }
                        "rating" if rating.is_none() => {
                            if let Ok(v) = text.parse::<f64>() {
                                rating = Some(v);
                            }
                        }
                        "runtime" => {
                            if let Ok(minutes) = text.parse::<i32>() {
                                *duration = Some(minutes * 60);
                            }
                        }
                        "thumb" if remote_cover_url.is_none() => {
                            remote_cover_url = Some(text);
                        }
                        "name" if in_actor => {
                            actor_names.push(text);
                        }
                        "tag" => {
                            tag_names.push(text);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                if tag == "actor" {
                    in_actor = false;
                }
                current_tag = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("解析 NFO 文件失败 '{}': {}", nfo_path.display(), e);
                return None;
            }
            _ => {}
        }
    }

    // 如果没有 premiered 但有 year，用 year 构造日期
    if premiered.is_none() {
        if let Some(y) = year {
            if !y.is_empty() {
                premiered = Some(format!("{}-01-01", y));
            }
        }
    }

    Some(NfoData {
        title,
        original_title,
        local_id,
        studio,
        director,
        premiered,
        rating,
        remote_cover_url,
        actor_names,
        tag_names,
    })
}
