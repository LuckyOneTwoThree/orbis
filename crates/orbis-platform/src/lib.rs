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

/// Windows 专有实现（注册表发现 / 进程查询 / 写权限检查）。
#[cfg(windows)]
pub mod windows_impl {}

/// 非 Windows stub：让跨平台编译与单测成立；真机行为不在此验收。
#[cfg(not(windows))]
pub mod windows_impl {}
