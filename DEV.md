# DEV.md

## 项目定位

JAVM 是一个基于 Tauri 的桌面应用，前端使用 Vue 3，后端使用 Rust。
核心目标是把本地视频管理、资源刮削、下载管理和深度链接串成一个完整工作流。

## 技术与目录

- 前端：`src/`
- Tauri/Rust：`src-tauri/src/`
- 资源与文档：`docs/`
- 脚本：`scripts/`

关键模块：
- 路由入口：`src/router.ts`
- 下载管理：`src-tauri/src/download/`
- 资源刮削：`src-tauri/src/resource_scrape/`
- 扫描与入库：`src-tauri/src/scanner/`
- 数据统计：`src-tauri/src/analytics.rs`
- 深度链接：`src-tauri/src/deep_link.rs`

## 环境要求

- Bun >= 1.0
- Node.js >= 18
- Rust stable
- Windows 开发建议：安装 Visual Studio C++ Build Tools（按 Tauri 官方要求）
- macOS 开发建议：安装 Xcode Command Line Tools
- Linux 开发建议：按 Tauri 官方文档安装 WebKitGTK 与构建工具链

## 常用命令

安装依赖：
```bash
bun install
```

前端开发：
```bash
bun run dev
```

桌面开发：
```bash
bun run tauri dev
```

构建前端：
```bash
bun run build
```

版本号同步：
```bash
bun run vb -- patch
bun run vb -- minor
bun run vb -- major
bun run vb -- 1.2.3
```

## 开发约定

1. 包管理优先使用 Bun
- 统一使用 `bun install`、`bun run`、`bunx`
- 不建议混用 `npm/pnpm/yarn` 进行日常开发

2. 前后端接口
- 前端通过 Tauri `invoke` 调 Rust 命令
- 新增命令后，记得在 `src-tauri/src/lib.rs` 的 `invoke_handler` 注册

3. 数据库变更
- 当前使用 SQLite
- 任何表结构变更必须包含迁移逻辑，避免破坏老用户数据

4. 跨平台下载器资源
- `src-tauri/bin/` 中必须提供当前平台可执行下载器
- Windows 默认查找 `N_m3u8DL-RE.exe`
- macOS 默认查找 `N_m3u8DL-RE-macos`、`N_m3u8DL-RE-darwin`、`N_m3u8DL-RE`
- Linux 默认查找 `N_m3u8DL-RE-linux`、`N_m3u8DL-RE`
- 若打包资源不存在，后端会回退到系统 `PATH` 中查找同名命令

5. 下载与刮削联动
- 下载完成可触发自动刮削
- 调整下载状态码时，需同步检查前端状态展示和事件通知

6. 深度链接
- 协议格式：`javm://download?url=<...>&title=<...>`
- 修改 deep link 行为时，需同时验证：
  - 首次启动唤起
  - 单实例已运行时唤起
  - 参数编码（中文标题/特殊字符）

7. 统计上报
- 统计采用匿名 UUID，不依赖用户注册
- 涉及统计字段调整时，注意本地累计逻辑与远端 Supabase 表结构一致

## 发布前检查清单

- 能正常扫描目录并入库
- 下载任务可完整走完（开始/暂停/恢复/停止/完成）
- 资源刮削可写入标题、封面、NFO
- 播放与截图功能可用
- 深度链接可从浏览器唤起并入队下载
- 设置项可持久化
- 版本号已同步（`package.json`、`tauri.conf.json`、`Cargo.toml`）

## 调试建议

1. Rust 日志
- 在 `bun run tauri dev` 控制台观察后端日志
- 对关键路径（下载、刮削、深度链接）保留清晰日志

2. 前端调试
- Vite 开发服务器可直接查看页面与控制台输出
- 关注事件通信（`download-progress`、deep-link 事件）是否按预期触发

3. 常见问题
- deep link 在 Windows 不生效：重启 `tauri dev` 并确认协议注册
- 下载器不可用：确认 `src-tauri/bin/` 下存在当前平台对应的 `N_m3u8DL-RE` 可执行文件，或系统 `PATH` 中可直接运行该命令
- 迁移后数据异常：优先检查 SQLite 迁移 SQL 与主键/唯一键策略

## 后续可补充内容

- CI/CD 与自动打包流程
- 完整状态码表（下载/刮削）
- 站点刮削策略与限频规则
- 测试策略（单元测试、集成测试、回归测试）
