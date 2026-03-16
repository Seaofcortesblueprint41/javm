//! 下载管理模块
//!
//! 统一管理所有下载相关功能，包括：
//! - `manager` - 下载管理器（队列、并发控制、进度解析、执行逻辑）
//! - `commands` - Tauri 命令（任务增删改查、批量操作）
//! - `image` - 图片下载（单张、批量、封面下载）

use std::path::{Path, PathBuf};

pub mod manager;
pub mod commands;
pub mod image;

/// 解析任务实际下载目录。
///
/// 约定：当任务存在文件名时，实际下载目录为 `{save_path}/{filename}`。
/// 这样最终视频路径会变成 `{save_path}/{filename}/{filename}.mp4`。
pub fn resolve_task_save_dir(save_path: &str, filename: Option<&str>) -> PathBuf {
	let base_dir = PathBuf::from(save_path);
	match filename.map(str::trim).filter(|name| !name.is_empty()) {
		Some(name) => base_dir.join(name),
		None => base_dir,
	}
}

/// 根据任务信息查找已完成视频文件路径。
///
/// 同时兼容：
/// - 新结构：`{save_path}/{filename}/{filename}.ext`
/// - 旧结构：`{save_path}/{filename}.ext`
pub fn find_existing_video_path(save_path: &str, filename: &str) -> Option<PathBuf> {
	let candidate_dirs = [
		resolve_task_save_dir(save_path, Some(filename)),
		PathBuf::from(save_path),
	];
	let extensions = ["mp4", "mkv", "ts", "avi", "mov", "m4a"];

	for dir in candidate_dirs {
		let direct = dir.join(filename);
		if direct.exists() {
			return Some(direct);
		}

		for ext in extensions {
			let path = dir.join(format!("{}.{}", filename, ext));
			if path.exists() {
				return Some(path);
			}
		}
	}

	None
}

pub fn is_same_path(left: &Path, right: &Path) -> bool {
	left == right
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resolve_task_save_dir_uses_filename_subdirectory() {
		let path = resolve_task_save_dir("D:/download", Some("adb-123"));
		assert_eq!(path, PathBuf::from("D:/download").join("adb-123"));
	}

	#[test]
	fn resolve_task_save_dir_falls_back_to_root_without_filename() {
		let path = resolve_task_save_dir("D:/download", None);
		assert_eq!(path, PathBuf::from("D:/download"));
	}
}
