//! Orbis Tools —— 工具运行时。
//!
//! # 职责（pm/04-技术设计.md §5.7 / §5.8）
//!
//! - **Manifest 加载**：`data/tools/manifests/*.json` 经 `include_str!` 打包进二进制，
//!   启动时解析 + schema 校验；**单个损坏只让该工具不可见**，不影响其它工具（02 C1）
//! - **执行器分派**：`config_modify` → EnhancementExecutor（04 §5.5）；
//!   `external_process` → UnlockerRunner（04 §5.8）
//! - **工具资产**：`data/tools/assets.json` 的 URL + SHA-256；哈希不符**拒绝加载**（防篡改）
//! - **panic 隔离**：执行器内 `catch_unwind`，任何 panic 转入 RollingBack，
//!   **不影响主进程**（00 §12.3 规则 9）。根 `Cargo.toml` 因此刻意**未**启用 `panic = "abort"`
//!
//! # 架构不变量
//!
//! 新增工具 = 新 Manifest +（必要时）providers 扩展，**Core 不动**（04 §9.1）。
//! 本 crate 不得依赖 `orbis-providers`。
//!
//! # 数据校验
//!
//! Manifest 与 assets 的结构由 `data/tools/*.schema.json` 定义，跨文件一致性
//! （manifest.id ↔ seed.tool_id ↔ assets.asset_key）由 `npm run validate:data` 守护，
//! Rust 侧只需再做一次加载期 schema 校验。

/// 加载内置 Manifest（待实现：需 serde + `manifests.schema.json` 校验）。
pub fn load_builtin_manifests() -> Vec<&'static str> {
    // TODO(实现期)：include_str! 引入 data/tools/manifests/*.json 并逐个解析；
    // 解析失败的那一条跳过并记日志，其余照常可用（02 C1 验收）
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_loading_is_placeholder_for_now() {
        assert!(super::load_builtin_manifests().is_empty());
    }
}
