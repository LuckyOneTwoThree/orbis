//! Orbis Providers —— 游戏装配层（**唯一**允许出现具体游戏知识的地方）。
//!
//! # 职责（pm/04-技术设计.md §4.2）
//!
//! 每个游戏一个装配条目（`GameProvider` struct），由能力横切的 trait 组合而成：
//! `DetectRule` / `VersionSource` / `ConfigSource` / `LaunchSpec`。
//!
//! MVP 装配表（04 §4.2）：
//!
//! | game slug | VersionSource | ConfigSource |
//! |-----------|---------------|--------------|
//! | `genshin-impact` | HoyoPlayAdapter | —（L3 不落盘，A8 不适用） |
//! | `honkai-star-rail` | HoyoPlayAdapter | — |
//! | `zenless-zone-zero` | HoyoPlayAdapter | — |
//! | `wuthering-waves` | KuroAdapter | WuwaConfigProvider（L1 深度标杆） |
//! | `arknights-endfield` | DegradedAdapter | — |
//!
//! # 待实测约束（04 §10，未收敛前不要凭推测填常量）
//!
//! - **T1 / T2**：米哈游国服 API base、鸣潮国服索引常量未实测 → adapter 常量表未定稿
//! - **T5 / T7**：exe 直启参数与注册表 / 路径发现规则未实测 → `LaunchSpec` / `DetectRule` 未定稿
//! - **T3**：鸣潮 3.x 双键写入行为未实测 → `WuwaConfigProvider` 的写入器增强项待定
//!
//! 留 `todo!()` 比写一个未经实测的常量更容易被发现 —— 后者会静默产生错误行为。
//!
//! 依赖方向：`providers → core, platform`；**禁止** providers ↔ tools 横向依赖。

/// 装配条目注册表：每个游戏一条（00 §7.1「每个游戏都是一个 Provider」）。
///
/// 待 04 §4.2 的 trait 定义落地后返回 5 个 `GameProvider`。
pub fn catalog() -> &'static [&'static str] {
    // TODO(实现期)：实现 5 个装配条目（04 §4.2 MVP 装配表）
    &[]
}

#[cfg(test)]
mod tests {
    /// 架构不变量自检：装配表数量必须等于 MVP 游戏数（5）。
    /// 真正的不变量断言由 CI grep 承担（04 §9.1），此处仅防止注册表被写空。
    #[test]
    fn catalog_is_placeholder_for_now() {
        assert!(super::catalog().is_empty());
    }
}
