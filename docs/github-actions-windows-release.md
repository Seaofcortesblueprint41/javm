# GitHub Actions 发版说明

## 当前行为

- 推送符合 semver 的 tag 时会触发线上构建，例如 `v0.1.5`、`v0.1.5-alpha.1`、`v0.1.5-beta.1`。
- 当前会在三个平台构建 Tauri 安装包：Windows、macOS、Linux。
- 当前已接入的架构矩阵：Windows `x64 / ARM64 / x86`，macOS `Intel(x64) / Apple Silicon(arm64)`，Linux `x64 / ARM64`。
- Windows 使用 `windows-latest` 和 `windows-11-arm`，macOS 使用 `macos-15-intel` 和 `macos-15`，Linux 使用 `ubuntu-22.04` 和 `ubuntu-22.04-arm`。
- 构建前会从 `https://github.com/nilaoda/N_m3u8DL-RE` 的最新非 draft Release 下载对应平台与架构的二进制，并放到 `src-tauri/bin` 后再参与打包。
- 当前下载映射为：Windows `win-x64.zip / win-arm64.zip / win-NT6.0-x86.zip`，macOS `osx-x64.tar.gz / osx-arm64.tar.gz`，Linux `linux-x64.tar.gz / linux-arm64.tar.gz`。
- 安装包图标直接复用 `src-tauri/icons/icon.ico`，当前项目配置已经包含它，无需额外处理。
- 发版前会校验 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三处版本号是否和 tag 去掉 `v` 后一致。
- Release 页面正文优先读取仓库中的版本日志文件：docs/releases/vx.y.z.md。
- 如果该文件不存在，工作流会生成一个最小兜底说明，但不会自动做 AI 总结。

当前默认产物方向：

- Windows：按架构分别输出安装包；预发布默认上传 `.exe`，正式版上传 `.exe` 和 `.msi`。当前 `x86` 预发布仍保留 `msi`，避免 32 位 NSIS 兼容性差异导致完全无产物。
- macOS：按 Intel 与 Apple Silicon 分别输出 `.dmg`，若 Tauri 产出 updater 包也会一并上传 `.app.tar.gz`。
- Linux：按 `x64` 与 `ARM64` 分别输出 `.deb` 和 `.AppImage`，如果生成 `.rpm` 也会一并上传。

当前未纳入此 workflow 的组合：

- Linux musl (`linux-musl-x64.tar.gz`、`linux-musl-arm64.tar.gz`)。
- Android / Termux (`android-bionic-arm64.tar.gz`、`android-bionic-x64.tar.gz`)。

原因：当前 workflow 是 Tauri 桌面端发布流，musl 和 Android 不属于同一套桌面安装包产物链路，需要单独设计发布任务。

## 很关键的一点

当前项目里的 Supabase 配置现在只保留“代码内置默认值”：

- 内置的 `SUPABASE_URL` / publishable key / table 默认值

这意味着：

- 安装后的应用始终使用代码里内置的 publishable 配置。
- GitHub Actions Secrets 不再参与应用内这条统计上报链路的配置注入。

## 如果你只是想保护密钥

推荐做法：

- 只给客户端使用 `SUPABASE_URL` 和 `SUPABASE_PUBLISHABLE_KEY`。
- 不要把 `SUPABASE_SECRET_KEY` 打进桌面安装包。

## 使用方式

1. 先把三个版本号同步成同一个版本，例如 `0.1.1`。
2. 提交代码并推送。
3. 打 tag：`git tag v0.1.1`
4. 推送 tag：`git push origin v0.1.1`
5. 等待 GitHub Actions 完成后，在 Releases 页面下载对应平台产物。

## 推荐做法

推荐在本地发布时，先由当前会话中的 AI 根据本次改动生成一个版本日志文件，再提交、打 tag、推送。这样 GitHub Actions 只负责读取文件内容，不需要额外配置 AI Secrets。

- 文件路径建议固定为：`docs/releases/vx.y.z.md`
- 例如：`docs/releases/v0.1.5.md`
- 预发布版本也支持，例如：`docs/releases/v0.1.5-alpha.1.md`
- 这个文件需要和版本升级提交一起进入仓库，这样 tag 对应的提交里就自带 Release 正文。
- 发布前先运行 `bun run release:collect -- v<version>` 生成完整上下文文件，再交给当前会话中的 AI 总结。
- 正式日志结构可参考 `docs/releases/TEMPLATE.md`。
- 更新日志的归纳范围建议固定为：从上一个 tag 到当前待发布版本之间的全部提交。
- 生成日志时不要只看提交标题，应同时读取提交正文，再让 AI 归纳总结。

当前 workflow 会优先读取对应 tag 的版本日志文件，并将其作为 GitHub Release 的正文。

建议：

- 版本日志里优先写用户可感知变更，其次再写工程或构建调整。
- 如果希望日志更准确，发布前先整理本次提交说明或 PR 标题。