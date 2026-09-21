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
//! - **T8**：5 条官方入口 URL 未核实 → [`catalog::GameEntry::official_url`] 暂按公开入口填写
//!
//! 留 `todo!()` 比写一个未经实测的常量更容易被发现 —— 后者会静默产生错误行为。
//!
//! 依赖方向：`providers → core, platform`；**禁止** providers ↔ tools 横向依赖。
//!
//! # 当前实现范围
//!
//! 已落地：[`catalog`] —— 编译期游戏目录，`listGames()`（契约 §3.1）的唯一数据源。
//!
//! 未落地：能力横切 trait（`DetectRule` / `VersionSource` / `ConfigSource` / `LaunchSpec`）
//! 与 5 个装配条目本身。它们**全部**被上表的实测项阻塞，先写出来的只会是猜测常量。

pub mod catalog;

#[cfg(test)]
mod tests {
    /// 装配表规模自检：MVP 恰好 5 款游戏（04 §4.2）。
    /// 具体内容断言（id 并集、slug 形态、区服、入口）在 [`catalog`] 的单测里。
    #[test]
    fn catalog_size_matches_the_mvp_scope() {
        assert_eq!(super::catalog::GAME_CATALOG.len(), 5);
    }
}
