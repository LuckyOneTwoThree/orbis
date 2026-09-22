//! 应用数据目录布局（`pm/04-技术设计.md` §6.3）。
//!
//! ```text
//! %APPDATA%\orbis\
//! ├── orbis.db                              # SQLite（WAL）
//! ├── backups\<installation_id>\<backup_id>\
//! ├── tools\genshin-fps-unlock\<version>\   # 发行包分离的解锁器资产
//! └── logs\orbis.YYYY-MM-DD.log             # JSONL，保留 14 天
//! ```
//!
//! # 为什么单独成模块
//!
//! 目录解析是**应用级**关注点，不是日志或数据库的私有实现：`log` 要 `logs/`，
//! `db` 要 `orbis.db`，将来的 `backup` 要 `backups/`。若把它留在 `log` 里，
//! `db` 就得为了一个路径去依赖日志模块 —— 那会让「谁依赖谁」变得毫无道理。
//!
//! # 平台策略
//!
//! Windows 用 `%APPDATA%\orbis`（02 §5 Windows 首发）；其它平台按 XDG 惯例。
//! 非 Windows 分支的存在是为了**让跨平台单测可跑**（docs/05 §1.1 双轨策略），
//! 不代表支持非 Windows 发行。

use std::path::PathBuf;

/// 应用数据根目录。
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("orbis"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|base| base.join("orbis"))
    }
}

/// 日志目录：`<data_dir>/logs`（04 §6.3）。
pub fn logs_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("logs"))
}

/// 数据库文件：`<data_dir>/orbis.db`（04 §6.3）。
pub fn db_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("orbis.db"))
}

/// 备份根目录：`<data_dir>/backups`（04 §5.4 / §6.3）。
pub fn backups_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("backups"))
}

/// 单个备份的目录：`<data_dir>/backups/<installation_id>/<backup_id>`（04 §5.4 存储布局）。
///
/// 分两层是刻意的：移除安装实例时按 `installation_id` 整目录清理即可，
/// 不必逐条遍历 `backup_id`。
///
/// 两个 id 都是我们自己生成并落库的（契约 §1 规定 `installationId` / `backupId` 为 UUID v4），
/// 因此可直接作为路径组件 —— 但**调用方不得**把来自 IPC 的任意字符串传进来当 id 用。
pub fn backup_dir(installation_id: &str, backup_id: &str) -> Option<PathBuf> {
    backups_dir().map(|d| d.join(installation_id).join(backup_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_hangs_off_the_app_data_dir() {
        let data = data_dir().expect("测试环境应能解析应用数据目录");
        assert!(data.ends_with("orbis"), "数据目录应以 orbis 结尾：{data:?}");

        assert_eq!(logs_dir().unwrap(), data.join("logs"));
        assert_eq!(db_path().unwrap(), data.join("orbis.db"));
        assert_eq!(backups_dir().unwrap(), data.join("backups"));
        assert_eq!(
            backup_dir("inst-1", "bk-2").unwrap(),
            data.join("backups").join("inst-1").join("bk-2"),
            "备份目录必须按 installation → backup 两级组织（04 §5.4）"
        );
    }
}
