//! Orbis Platform —— OS 落地层。
//!
//! # 职责（pm/04-技术设计.md §4.1）
//!
//! - SQLite 持久化（`rusqlite` bundled）：installation / launch_profile / playtime_session /
//!   tool_state / backup / app_setting（04 §6.1）
//! - 备份与恢复：整目录快照 + 逐文件 SHA-256（04 §5.4）
//! - 进程快照与时长追踪：`sysinfo` 轮询（04 §5.10）
//! - 统一日志：`tracing` + 自定义 JSONL Layer（04 §5.11，字段 schema 见 00 §9.1）
//! - 只读版本探测：`reqwest`（04 §5.2）
//! - 应用级约束：单实例互斥 / 按实例串行化 / 备份空间检查（04 §5.12）
//!
//! # 平台策略（双轨开发，docs/05 §1.1）
//!
//! Windows 专有代码（注册表、进程枚举、写权限检查）必须用 `#[cfg(windows)]` 门控，
//! 并为非 Windows 提供 stub —— 目的是让 macOS 上也能 `cargo test` 整个 workspace，
//! 而真机行为只在 Windows 上验收。
//!
//! 依赖方向：`platform → core`（不得反向，不得依赖 providers / tools）。

#![cfg_attr(not(windows), allow(unused))]

/// SQLite 持久化（04 §6.1 schema / §6.3 布局）。**已落地**：建库 + 迁移 + 设置白名单。
pub mod db;

/// 安装实例与启动参数的仓储（04 §6.1 / 契约 §3.1、§3.3）。**已落地**：读 + 删除 + 启动参数写入。
///
/// `addInstallation`（A2）落地前，写入路径仅测试覆盖 —— 它被实测项 T5/T7 阻塞，
/// 见模块文档。
pub mod installation;

/// D3 统一日志（00 §9.1 schema / 04 §5.11）。**已落地**。
pub mod log;

/// 应用数据目录布局（04 §6.3）。
pub mod paths;

/// 游戏时长（04 §5.10，A6）。**已落地**：会话跟踪 + 30s 检查点 + 崩溃封存 + 区间聚合。
///
/// 依赖 Q11 的裁决（本地时区依赖 `chrono`），见模块文档。
pub mod playtime;

/// 进程快照（04 §5.10，A5 运行状态）。**已落地**：进程表采集 + exe 路径前缀匹配。
///
/// 这是唯一不依赖任何实测项就能做对的 A 域能力 —— 匹配只用库里已有的
/// `executable_path`，不需要按游戏的进程名常量。
pub mod process;

/// 测试辅助（仅 `cfg(test)` 编译）。
#[cfg(test)]
mod test_support;

/// 当前构建目标是否为受支持的平台。
///
/// 兼容性口径见 pm/02 §5：Windows 10 19041+ / Windows 11 x64。
/// 这里只判断**编译目标是否 Windows** —— 具体版本下限由安装包的 supportedOS
/// 声明与运行期检查负责，这一层不假装能判断系统版本。
pub const fn is_supported_target() -> bool {
    cfg!(windows)
}

/// Windows 专有实现（注册表发现 / 进程查询 / 写权限检查）。
#[cfg(windows)]
pub mod windows_impl {}

/// 非 Windows stub：让跨平台编译与单测成立；真机行为不在此验收。
#[cfg(not(windows))]
pub mod windows_impl {}

#[cfg(test)]
mod tests {
    #[test]
    fn target_report_matches_cfg() {
        assert_eq!(super::is_supported_target(), cfg!(windows));
    }
}
