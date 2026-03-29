use serde::{Deserialize, Serialize};
use std::fs;
use tauri::AppHandle;
use tauri::Manager;

use super::encryption::{encrypt_settings, decrypt_settings};
use super::{AppSettings, normalize_scrape_settings, get_settings_path};

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let path = get_settings_path(&app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut settings: AppSettings = serde_json::from_str(&content).unwrap_or_default();

    // 解密API Key
    decrypt_settings(&mut settings);
    normalize_scrape_settings(&mut settings.scrape);

    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, mut settings: AppSettings) -> Result<(), String> {
    let path = get_settings_path(&app)?;
    let dir = path.parent()
        .ok_or_else(|| "设置文件路径无效".to_string())?;
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }

    // 加密API Key后再保存
    encrypt_settings(&mut settings);
    normalize_scrape_settings(&mut settings.scrape);

    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&path, &content).map_err(|e| e.to_string())?;

    // 刷新全局代理缓存
    if let Ok(config_dir) = app.path().app_config_dir() {
        crate::utils::proxy::refresh(&config_dir);
    }

    if let Some(manager) = app.try_state::<crate::download::manager::DownloadManager>() {
        manager
            .set_max_concurrent(settings.download.concurrent.max(1) as usize)
            .await;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestApiRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestApiResponse {
    pub success: bool,
    pub message: String,
}

/// 测试AI API连接
#[tauri::command]
pub async fn test_ai_api(request: TestApiRequest) -> Result<TestApiResponse, String> {
    let client = crate::utils::proxy::apply_proxy_auto(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15)),
    )
    .map_err(|e| e.to_string())?
    .build()
    .map_err(|e| e.to_string())?;

    // 构建测试端点URL
    let base_url = request
        .endpoint
        .unwrap_or_else(|| match request.provider.as_str() {
            "openai" => "https://api.openai.com/v1".to_string(),
            "deepseek" => "https://api.deepseek.com/v1".to_string(),
            "claude" => "https://api.anthropic.com/v1".to_string(),
            _ => String::new(),
        });

    if base_url.is_empty() {
        return Ok(TestApiResponse {
            success: false,
            message: "请提供有效的API端点".to_string(),
        });
    }

    // 根据provider构建不同的测试请求
    if request.provider == "claude" {
        // Claude使用messages端点
        let endpoint = format!("{}/messages", base_url.trim_end_matches('/'));

        let test_payload = serde_json::json!({
            "model": request.model,
            "max_tokens": 1,
            "messages": [{
                "role": "user",
                "content": "test"
            }]
        });

        let response = client
            .post(&endpoint)
            .header("x-api-key", &request.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&test_payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    Ok(TestApiResponse {
                        success: true,
                        message: "API连接成功！".to_string(),
                    })
                } else {
                    let error_text = resp.text().await.unwrap_or_else(|_| "未知错误".to_string());
                    Ok(TestApiResponse {
                        success: false,
                        message: format!("API返回错误 ({}): {}", status.as_u16(), error_text),
                    })
                }
            }
            Err(e) => Ok(TestApiResponse {
                success: false,
                message: format!("连接失败: {}", e),
            }),
        }
    } else {
        // OpenAI兼容API使用chat/completions端点
        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        let test_payload = serde_json::json!({
            "model": request.model,
            "messages": [{
                "role": "user",
                "content": "test"
            }],
            "max_tokens": 1
        });

        let response = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", request.api_key))
            .header("content-type", "application/json")
            .json(&test_payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    Ok(TestApiResponse {
                        success: true,
                        message: "API连接成功！".to_string(),
                    })
                } else {
                    let error_text = resp.text().await.unwrap_or_else(|_| "未知错误".to_string());
                    // 尝试解析JSON错误信息
                    if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                        let error_msg = error_json["error"]["message"]
                            .as_str()
                            .or_else(|| error_json["message"].as_str())
                            .unwrap_or(&error_text);
                        Ok(TestApiResponse {
                            success: false,
                            message: format!("API返回错误 ({}): {}", status.as_u16(), error_msg),
                        })
                    } else {
                        Ok(TestApiResponse {
                            success: false,
                            message: format!("API返回错误 ({}): {}", status.as_u16(), error_text),
                        })
                    }
                }
            }
            Err(e) => {
                let error_msg = if e.is_timeout() {
                    "连接超时，请检查网络或API端点".to_string()
                } else if e.is_connect() {
                    "无法连接到服务器，请检查API端点是否正确".to_string()
                } else {
                    format!("连接失败: {}", e)
                };

                Ok(TestApiResponse {
                    success: false,
                    message: error_msg,
                })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecognizeDesignationResponse {
    pub success: bool,
    pub designation: Option<String>,
    pub method: String, // "regex" | "ai" | "failed"
    pub message: String,
}

/// 使用AI识别视频标题中的番号
#[tauri::command]
pub async fn recognize_designation_with_ai(
    app: AppHandle,
    title: String,
    force_ai: Option<bool>, // 新增参数：是否强制使用 AI
) -> Result<RecognizeDesignationResponse, String> {
    use crate::utils::designation_recognizer::{
        AIProvider as RecognizerAIProvider, DesignationRecognizer, RecognitionMethod,
    };

    let force_ai = force_ai.unwrap_or(false);

    // 获取设置
    let settings = get_settings(app).await?;

    // 找到第一个启用的AI提供商
    let ai_provider = settings
        .ai
        .providers
        .iter()
        .filter(|p| p.active)
        .min_by_key(|p| p.priority)
        .map(|p| RecognizerAIProvider {
            provider: p.provider.clone(),
            model: p.model.clone(),
            api_key: p.api_key.clone(),
            endpoint: p.endpoint.clone(),
        });

    // 创建识别器
    let recognizer = if let Some(provider) = ai_provider {
        DesignationRecognizer::with_ai_provider(provider)
    } else {
        DesignationRecognizer::new()
    };

    // 执行识别
    let result = recognizer.recognize(&title, force_ai).await?;

    // 转换结果格式
    Ok(RecognizeDesignationResponse {
        success: result.success,
        designation: result.designation,
        method: match result.method {
            RecognitionMethod::Regex => "regex".to_string(),
            RecognitionMethod::AI => "ai".to_string(),
            RecognitionMethod::Failed => "failed".to_string(),
        },
        message: match result.method {
            RecognitionMethod::Regex => format!("智能识别成功（正则匹配）"),
            RecognitionMethod::AI => format!("智能识别成功（AI）"),
            RecognitionMethod::Failed => {
                if force_ai && !recognizer.has_ai_provider() {
                    "没有可用的AI提供商，请在设置中配置".to_string()
                } else {
                    result.message
                }
            }
        },
    })
}
