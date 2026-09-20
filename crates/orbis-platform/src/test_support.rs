//! 测试辅助（仅 `cfg(test)` 编译）。
//!
//! # 为什么析构里的删除失败要吞掉
//!
//! 本机环境的安全删除钩子会拦下删除（尤其批量删除），`fs::remove_dir_all` 因此可能
//! 返回错误。那属于**环境噪音**，不该让一个本来通过的测试变红 —— 所以这里显式忽略，
//! 而不是 `expect`。临时目录留在系统 temp 下无副作用。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 自清理的临时目录；同一进程内多次调用不会碰撞。
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("orbis-test-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("应能创建临时目录");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
