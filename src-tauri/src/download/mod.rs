//! 下载管理模块
//!
//! 统一管理所有下载相关功能，包括：
//! - `manager` - 下载管理器（队列、并发控制、进度解析、执行逻辑）
//! - `commands` - Tauri 命令（任务增删改查、批量操作）
//! - `image` - 图片下载（单张、批量、封面下载）

pub mod manager;
pub mod commands;
pub mod image;
