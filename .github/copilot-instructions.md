# Project AI Assistant Rules

## 1. 语言与沟通 (Language & Communication)
- **绝对要求**：所有的对话、代码注释、Git Commit Message 以及文档编写，必须完全使用中文（zh-CN）。
- 回答要简明扼要，直接给出代码或解决方案，拒绝冗长的废话。

## 2. 工具链与包管理 (Toolchain & Package Manager)
- **绝对要求**：在前端生态中，严格且唯一地使用 `bun` 作为包管理器和任务运行器。
- 永远**不要**在建议中使用 `npm`、`yarn` 或 `pnpm`。
- 举例：安装依赖使用 `bun add`，运行脚本使用 `bun run`。
- **版本升级要求**：项目版本号升级必须使用 `bun run vb -- <patch|minor|major>` 或 `bun run version:bump -- <patch|minor|major>`，禁止手动编辑 `package.json`、`src-tauri/tauri.conf.json` 和 `src-tauri/Cargo.toml` 中的版本号。

## 3. 技术栈规范 (Tech Stack Specifications)

### 3.1 前端 (Frontend)
- **核心框架**：Vite + Vue 3 + TypeScript。
- **Vue 规范**：严格使用 Vue 3 的 Composition API 和 `<script setup>` 语法糖。避免使用 Options API。
- **样式与 UI**：使用 Tailwind CSS 进行样式编写，UI 组件库使用 Reka UI。优先使用 Tailwind 的原子类，避免编写冗长的自定义 CSS。

### 3.2 后端 (Backend)
- **核心框架**：Rust + Tauri 2.0。
- **Rust 规范**：编写符合惯用法的、内存安全的 Rust 代码。优先考虑性能和跨平台兼容性。
- **Tauri 规范**：注意 Tauri 2.0 的 API 变化（如插件系统和移动端支持机制），前后端通信严格使用 Tauri 的 IPC 机制（前端 `invoke`，后端 `#[tauri::command]`）。

## 4. 验证与质量控制 (Verification & Quality Control)
- **编译检查**：如果在对话中对前端或后端代码进行了大量修改或重构，你必须在回答的末尾主动验证代码的编译状态。
  - Rust 后端修改：运行 `cargo check` 或 `cargo clippy`。
  - Vue 前端修改：运行 `bun run build`。
- **错误处理**：Rust 中避免滥用 `.unwrap()`，必须进行优雅的错误处理并返回给前端；前端在调用 `invoke` 时必须使用 `try-catch` 捕获后端抛出的异常。