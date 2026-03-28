use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewWindow};

use super::cf_detection;

static WEBVIEW_EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfStatePayload {
    pub status: &'static str,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_id: Option<String>,
    pub active_count: usize,
}

pub fn next_event_name(prefix: &str) -> String {
    let id = WEBVIEW_EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}", prefix, id)
}

pub fn build_cf_probe_script(event_name: &str) -> String {
    let detector = cf_detection::build_cloudflare_detection_function();
    format!(
        r#"
            (function() {{
                try {{
                    {detector}
                    var detected = __javmDetectCloudflareChallenge();
                    window.__CF_CHALLENGE_ACTIVE__ = detected;
                    if (window.__TAURI__ && window.__TAURI__.event) {{
                        window.__TAURI__.event.emit({:?}, detected);
                    }}
                }} catch (e) {{}}
            }})();
        "#,
        event_name,
        detector = detector,
    )
}

pub fn build_html_extract_script(cf_event_name: &str, html_event_name: &str) -> String {
    let detector = cf_detection::build_cloudflare_detection_function();
    format!(
        r#"
            (function() {{
                try {{
                    if (document.readyState !== 'complete') return;
                    if (!document.body || document.body.innerHTML.length < 100) return;

                    {detector}
                    var html = document.documentElement ? document.documentElement.outerHTML : '';
                    var detected = __javmDetectCloudflareChallenge();

                    if (window.__TAURI__ && window.__TAURI__.event) {{
                        window.__TAURI__.event.emit({:?}, detected);
                        if (!detected) {{
                            window.__TAURI__.event.emit(
                                {:?},
                                html
                            );
                        }}
                    }}
                }} catch (e) {{}}
            }})();
        "#,
        cf_event_name, html_event_name,
        detector = detector,
    )
}

pub fn listen_cf_visibility(
    app: &AppHandle,
    window: &WebviewWindow,
    site: &super::sources::ResourceSite,
    event_name: &str,
    frontend_event_name: Option<&str>,
) -> tauri::EventId {
    let window = (*window).clone();
    let app_handle = app.clone();
    let site_id = site.id.clone();
    let frontend_event_name = frontend_event_name.map(str::to_string);
    let last_state = Arc::new(Mutex::new(None::<bool>));
    app.listen(event_name.to_string(), move |event| {
        let Ok(challenge_detected) = serde_json::from_str::<bool>(event.payload()) else {
            return;
        };

        let previous_state = {
            let mut guard = match last_state.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let previous = *guard;
            if previous != Some(challenge_detected) {
                *guard = Some(challenge_detected);
            }
            previous
        };

        if let Some(frontend_event_name) = &frontend_event_name {
            let snapshot = app_handle
                .state::<super::fetcher::WebviewPoolState>()
                .update_cf_state(window.label(), challenge_detected);
            match (previous_state, challenge_detected) {
                (Some(true), false) => {
                    // CF 验证通过后自动隐藏 WebView 窗口
                    sync_window_visibility(&window, false);
                    emit_cf_state(
                        &app_handle,
                        frontend_event_name,
                        "passed",
                        snapshot.site_id.or_else(|| Some(site_id.clone())),
                        snapshot.active_count,
                    );
                }
                (previous, true) if previous != Some(true) => {
                    // CF 验证触发时自动显示 WebView 窗口
                    sync_window_visibility(&window, true);
                    emit_cf_state(
                        &app_handle,
                        frontend_event_name,
                        "active",
                        snapshot.site_id.or_else(|| Some(site_id.clone())),
                        snapshot.active_count,
                    );
                }
                _ => {}
            }
        }
    })
}

pub fn sync_window_visibility(window: &WebviewWindow, visible: bool) {
    if visible {
        let _ = window.show();
    } else {
        let _ = window.hide();
    }
}

pub fn emit_cf_state(
    app: &AppHandle,
    frontend_event_name: &str,
    status: &'static str,
    site_id: Option<String>,
    active_count: usize,
) {
    let payload = CfStatePayload {
        status,
        active: active_count > 0,
        site_id,
        active_count,
    };
    let _ = app.emit(frontend_event_name, payload);
}

/// WebView 使用的 User-Agent，版本号需与 HTTP 客户端保持一致
pub const WEBVIEW_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

/// WebView2 额外启动参数，禁用自动化检测相关特征
#[cfg(target_os = "windows")]
pub const WEBVIEW_BROWSER_ARGS: &str =
    "--disable-blink-features=AutomationControlled --disable-features=msWebView2BrowserHitTransparent";

/// 构建反自动化检测的初始化脚本，在页面 JS 执行前注入
pub fn build_anti_detection_script() -> String {
    r#"
        (function() {
            'use strict';

            // ── 1. 隐藏 navigator.webdriver ──
            Object.defineProperty(navigator, 'webdriver', {
                get: function() { return undefined; },
                configurable: true,
            });

            // ── 2. 伪造 navigator.userAgentData（Client Hints，CF 重点检测） ──
            if (!navigator.userAgentData || /WebView/i.test(JSON.stringify(navigator.userAgentData.brands || []))) {
                var fakeUAData = {
                    brands: [
                        { brand: 'Chromium', version: '134' },
                        { brand: 'Google Chrome', version: '134' },
                        { brand: 'Not:A-Brand', version: '24' }
                    ],
                    mobile: false,
                    platform: 'Windows',
                    getHighEntropyValues: function(hints) {
                        return Promise.resolve({
                            brands: this.brands,
                            mobile: false,
                            platform: 'Windows',
                            platformVersion: '15.0.0',
                            architecture: 'x86',
                            bitness: '64',
                            model: '',
                            uaFullVersion: '134.0.6998.89',
                            fullVersionList: this.brands
                        });
                    }
                };
                Object.defineProperty(navigator, 'userAgentData', {
                    get: function() { return fakeUAData; },
                    configurable: true,
                });
            }

            // ── 3. 伪造 navigator.plugins（需要模拟 PluginArray 接口） ──
            if (navigator.plugins.length === 0) {
                var fakePlugins = [
                    { name: 'PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format', length: 1 },
                    { name: 'Chrome PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format', length: 1 },
                    { name: 'Chromium PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format', length: 1 },
                    { name: 'Microsoft Edge PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format', length: 1 },
                    { name: 'WebKit built-in PDF', filename: 'internal-pdf-viewer', description: 'Portable Document Format', length: 1 }
                ];
                fakePlugins.item = function(i) { return this[i] || null; };
                fakePlugins.namedItem = function(n) {
                    for (var j = 0; j < this.length; j++) {
                        if (this[j].name === n) return this[j];
                    }
                    return null;
                };
                fakePlugins.refresh = function() {};
                Object.defineProperty(navigator, 'plugins', {
                    get: function() { return fakePlugins; },
                    configurable: true,
                });
            }

            // ── 4. 确保 navigator.languages 正常 ──
            if (!navigator.languages || navigator.languages.length === 0) {
                Object.defineProperty(navigator, 'languages', {
                    get: function() { return ['zh-CN', 'zh', 'en']; },
                    configurable: true,
                });
            }

            // ── 5. 伪造 Notification 权限（WebView 通常缺失） ──
            if (typeof Notification === 'undefined') {
                window.Notification = function() {};
                window.Notification.permission = 'default';
                window.Notification.requestPermission = function() { return Promise.resolve('default'); };
            }

            // ── 6. 伪造 chrome.runtime ──
            if (!window.chrome) window.chrome = {};
            if (!window.chrome.runtime) {
                window.chrome.runtime = {
                    connect: function() { return { onMessage: { addListener: function() {} }, postMessage: function() {} }; },
                    sendMessage: function() {},
                    id: undefined
                };
            }
            // chrome.app（真实 Chrome 存在此对象）
            if (!window.chrome.app) {
                window.chrome.app = {
                    isInstalled: false,
                    InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
                    RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
                    getDetails: function() { return null; },
                    getIsInstalled: function() { return false; }
                };
            }

            // ── 7. 移除 Chromium DevTools 协议残留属性 ──
            var cdcKeys = Object.getOwnPropertyNames(window).filter(function(k) {
                return /^cdc_/i.test(k) || /^\$cdc_/i.test(k);
            });
            cdcKeys.forEach(function(k) { try { delete window[k]; } catch(e) {} });

            // ── 8. 屏幕尺寸保护（隐藏窗口可能 0×0 被 CF 检测） ──
            if (window.innerWidth === 0 || window.innerHeight === 0) {
                Object.defineProperty(window, 'innerWidth', { get: function() { return 1920; }, configurable: true });
                Object.defineProperty(window, 'innerHeight', { get: function() { return 1080; }, configurable: true });
                Object.defineProperty(window, 'outerWidth', { get: function() { return 1920; }, configurable: true });
                Object.defineProperty(window, 'outerHeight', { get: function() { return 1080; }, configurable: true });
            }

            // ── 9. 修正 permissions API 行为（更接近真实浏览器） ──
            if (navigator.permissions) {
                var origQuery = navigator.permissions.query.bind(navigator.permissions);
                navigator.permissions.query = function(desc) {
                    if (desc && desc.name === 'notifications') {
                        return Promise.resolve({ state: 'prompt', onchange: null });
                    }
                    return origQuery(desc);
                };
            }
        })();
    "#.to_string()
}

pub fn persistent_data_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("解析 WebView 数据目录失败: {}", e))?;

    let dir = app_data_dir.join("webview").join("external-profile");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建 WebView 数据目录失败: {}", e))?;

    Ok(dir)
}
