// 深度链接解析模块
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDeepLink {
    pub action: String,
    pub url: String,
    pub title: String,
}

/// 解析深度链接 URL。
///
/// 支持的格式：
/// - javm://download?url=<video_url>&title=<title>
/// - javm://download?url=<video_url>
/// - javm://download/?url=<video_url>&title=<title>
pub fn parse_url(url: &str) -> Result<ParsedDeepLink, String> {
    let parsed = Url::parse(url).map_err(|e| format!("URL 无效: {e}"))?;
    process_single_url(&parsed)
}

fn process_single_url(url: &Url) -> Result<ParsedDeepLink, String> {
    // 检查协议
    if url.scheme() != "javm" {
        return Err(format!("不支持的协议: {}", url.scheme()));
    }

    // 获取主机名（命令类型）
    // 注意：URL 可能是 javm://download?... 或 javm://download/?...
    let host = url.host_str().unwrap_or("");
    let path = url.path();
    
    // 如果 host 为空但 path 以 /download 开头，说明是 javm://download/?... 格式
    let command = if !host.is_empty() {
        host
    } else if path.starts_with("/download") {
        "download"
    } else {
        ""
    };

    match command {
        "download" => handle_download_command(url),
        _ => Err(format!("未知命令: {}", command)),
    }
}

fn handle_download_command(url: &Url) -> Result<ParsedDeepLink, String> {
    // 解析查询参数
    let query_pairs: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();

    let video_url = query_pairs
        .get("url")
        .ok_or("缺少 url 参数")?
        .to_string();

    let title = query_pairs
        .get("title")
        .cloned()
        .unwrap_or_else(|| "未命名视频".to_string());

    Ok(ParsedDeepLink {
        action: "download".to_string(),
        url: video_url,
        title,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_url;

    #[test]
    fn supports_download_link() {
        let parsed = parse_url("javm://download?url=https%3A%2F%2Fexample.com%2Fvideo.mp4&title=test")
            .expect("should parse download link");

        assert_eq!(parsed.action, "download");
        assert_eq!(parsed.url, "https://example.com/video.mp4");
        assert_eq!(parsed.title, "test");
    }

    #[test]
    fn supports_download_link_with_slash_path() {
        let parsed = parse_url("javm://download/?url=https%3A%2F%2Fexample.com%2Fvideo.mp4")
            .expect("should parse path style download link");

        assert_eq!(parsed.action, "download");
        assert_eq!(parsed.url, "https://example.com/video.mp4");
        assert_eq!(parsed.title, "未命名视频");
    }
}
