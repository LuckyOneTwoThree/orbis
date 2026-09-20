//! Orbis Core —— 领域模型、能力 trait 与兼容引擎。
//!
//! # 边界（架构不变量，`pm/04-技术设计.md` §9.1）
//!
//! - **零游戏知识**：任何具体游戏标识都不得出现在本 crate（规则 1）
//! - **零平台依赖**：不含 Windows API、文件系统实现或网络实现，因此可在任意平台跑单测
//! - **不依赖任何其它 orbis crate**：依赖方向为 `providers/tools → core`、`platform → core`
//!
//! # 当前实现范围
//!
//! 本 crate 先落地**平台无关的纯逻辑切片**（版本口径、兼容匹配、需处理判定）；
//! 自 2026-09-20 起接入 `serde`，补上**内置数据加载**（种子表）。
//! 能力 trait（`VersionSource` / `ConfigSource` / `LaunchSpec` / `DetectRule`）
//! 随 providers 落地再定义 —— 它们的实现者全在 `orbis-providers`。

pub mod attention;
pub mod compat;
pub mod model;
pub mod seed;
pub mod tool;
pub mod version;

pub use attention::{attention_reasons, needs_attention, AttentionInput, AttentionReason};
pub use compat::{
    query, CompatEntry, CompatRecord, CompatStatus, Compatibility, MatchKind, VersionMatch,
};
pub use model::{GameId, GameRuntimeStatus, Region};
pub use seed::{SeedError, SeedTable, SUPPORTED_SCHEMA_VERSION};
pub use tool::{is_valid_tool_id, RiskLevel, SourceKind, ToolPermission, ToolSource, ToolType};
pub use version::{compare, is_prefix_key, is_update_available, normalize, Version};
