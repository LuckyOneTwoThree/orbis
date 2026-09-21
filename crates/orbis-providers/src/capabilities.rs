//! 游戏能力声明（`pm/04-技术设计.md` §4.2 MVP 装配表的机器可读形式）。
//!
//! # 为什么需要它，而不是直接把表写在文档里
//!
//! 契约的 `InstallationDto` 要求两个字段：`hasConfigSource` 与
//! `configUnsupportedReason`。它们**不是安装实例的属性**（同一款游戏在任何机器上
//! 答案都一样），而是「这个游戏的 Provider 声明了什么能力」——按 04 §4.2 的装配模型，
//! 那就是 `GameProvider.config: Option<Box<dyn ConfigSource>>` 的 `Some/None`。
//!
//! T1/T2/T3/T5/T7 未收敛前，`ConfigSource` 的**实现**（读写哪些键、怎么 upsert）
//! 无法落地；但「哪款游戏声明了配置源」是**已定稿的设计决定**（04 §4.2 明文列表），
//! 与实测项无关。因此这里先把**能力声明**落地，让 `configUnsupportedReason`
//! 有据可依；等实测收敛后再把 `Declared` 换成真正的 `ConfigSource` 实现。
//!
//! # 与前端参照实现的一致性
//!
//! `src/api/mock.ts` 的 `SEED_INSTALLATIONS` 是本表的第二份表达。两者必须一致 ——
//! mock 是契约的行为参照（契约 §7.2），不一致时**先确认哪个对**，不要各自修改。
//! `not_applicable` 与 `provider_not_declared` 的区别不是文字游戏：
//! 前者 UI 说「不适用」（该游戏只有 L3 内存级工具，无落盘配置可备份），
//! 后者说「暂不支持配置备份」（Provider 尚未声明）。03 §5.4 对这两句文案有明文规定。

/// 配置源能力（决定 A8 备份范围与 03 §5.4 的禁用态文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigCapability {
    /// Provider 声明了配置路径 → 可备份（MVP 只有鸣潮，L1 深度标杆）
    Declared,
    /// Provider 未声明 → UI「该游戏暂不支持配置备份」
    ProviderNotDeclared,
    /// 该游戏不适用配置备份 → UI「不适用」（只有 L3 内存级工具，不落盘）
    NotApplicable,
}

impl ConfigCapability {
    /// 契约 `InstallationDto.hasConfigSource`。
    pub const fn has_config_source(self) -> bool {
        matches!(self, Self::Declared)
    }

    /// 契约 `InstallationDto.configUnsupportedReason`。
    pub const fn unsupported_reason(self) -> Option<&'static str> {
        match self {
            Self::Declared => None,
            Self::ProviderNotDeclared => Some("provider_not_declared"),
            Self::NotApplicable => Some("not_applicable"),
        }
    }
}

/// 一款游戏的能力声明。字段与 04 §4.2 装配表的列一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameCapabilities {
    /// 契约 §2 的 `GameId` slug（必须存在于 [`crate::catalog`]）
    pub game_id: &'static str,
    pub config: ConfigCapability,
}

/// MVP 能力表（逐行取自 04 §4.2「MVP 装配表（5 款）」）。
///
/// | GameId | ConfigSource | 状态说明 |
/// |--------|--------------|----------|
/// | `genshin-impact` | —（L3 不落盘，A8 不适用） | E1 直连 |
/// | `honkai-star-rail` | — | E1 直连 |
/// | `zenless-zone-zero` | — | E1 直连 |
/// | `wuthering-waves` | `WuwaConfigProvider` | L1 深度标杆 |
/// | `arknights-endfield` | — | E1 降级 |
pub const GAME_CAPABILITIES: &[GameCapabilities] = &[
    GameCapabilities {
        game_id: "wuthering-waves",
        // L1 深度标杆：唯一有 ConfigSource 的游戏
        config: ConfigCapability::Declared,
    },
    GameCapabilities {
        game_id: "genshin-impact",
        // 表中特别标注「A8 不适用」：它的增强手段是 L3 内存级工具，无落盘配置
        config: ConfigCapability::NotApplicable,
    },
    GameCapabilities {
        game_id: "honkai-star-rail",
        config: ConfigCapability::ProviderNotDeclared,
    },
    GameCapabilities {
        game_id: "zenless-zone-zero",
        config: ConfigCapability::ProviderNotDeclared,
    },
    GameCapabilities {
        game_id: "arknights-endfield",
        config: ConfigCapability::ProviderNotDeclared,
    },
];

/// 取某款游戏的能力声明；未登记的游戏 → `None`。
///
/// **没有兜底默认值**：返回 `None` 而不是「按未声明处理」是刻意的 ——
/// 游戏目录与能力表必须成对维护（见下面的单测），缺一条说明有人只改了一半。
pub fn capabilities(game_id: &str) -> Option<&'static GameCapabilities> {
    GAME_CAPABILITIES.iter().find(|c| c.game_id == game_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::GAME_CATALOG;

    #[test]
    fn the_table_covers_exactly_the_catalog() {
        // 目录与能力表必须成对维护：只加 catalog 会让新游戏拿不到能力声明，
        // 于是 configUnsupportedReason 无从计算（而不是「恰好默认为未声明」）
        let mut capability_ids: Vec<&str> = GAME_CAPABILITIES.iter().map(|c| c.game_id).collect();
        let mut catalog_ids: Vec<&str> = GAME_CATALOG.iter().map(|e| e.id).collect();
        capability_ids.sort_unstable();
        catalog_ids.sort_unstable();
        assert_eq!(
            capability_ids, catalog_ids,
            "能力表与游戏目录必须一一对应（双源漂移在此暴露）"
        );
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = GAME_CAPABILITIES.iter().map(|c| c.game_id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "能力表存在重复条目");
    }

    #[test]
    fn only_one_game_declares_a_config_source_in_the_mvp() {
        // 04 §4.2：MVP 只有鸣潮有 ConfigSource。若将来新增，这条会红 ——
        // 那是提醒同步 04 §4.2 与 A8 范围，而不是随手改断言
        let declared: Vec<&str> = GAME_CAPABILITIES
            .iter()
            .filter(|c| c.config == ConfigCapability::Declared)
            .map(|c| c.game_id)
            .collect();
        assert_eq!(declared, vec!["wuthering-waves"]);
    }

    #[test]
    fn not_applicable_and_not_declared_are_distinct() {
        // 两者文案不同（03 §5.4）：「不适用」 vs 「暂不支持配置备份」。
        // 混淆会让原神页显示错误的禁用理由
        assert_eq!(
            ConfigCapability::NotApplicable.unsupported_reason(),
            Some("not_applicable")
        );
        assert_eq!(
            ConfigCapability::ProviderNotDeclared.unsupported_reason(),
            Some("provider_not_declared")
        );
        assert_eq!(ConfigCapability::Declared.unsupported_reason(), None);
        assert!(ConfigCapability::Declared.has_config_source());
        assert!(!ConfigCapability::NotApplicable.has_config_source());
    }

    #[test]
    fn lookup_is_exact_and_misses_stay_misses() {
        for entry in GAME_CAPABILITIES {
            assert!(capabilities(entry.game_id).is_some());
        }
        assert!(capabilities("no-such-game").is_none());
        assert!(capabilities("Wuthering-Waves").is_none(), "须区分大小写");
    }
}
