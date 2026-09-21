//! 游戏目录（编译期静态，`pm/00-产品基石.md` §7.10 / 契约 §3.1 `listGames`）。
//!
//! # 为什么目录在这里
//!
//! `Game` 实体**不入库**（00 §7.10）：它不随扫描结果变化，也没有用户可编辑的字段。
//! 但 `GameId` 也**不能是 Core 的闭集枚举** —— 那会让「新增游戏」必须改 Core，
//! 直接违反 00 §12.3 规则 1（Core 零游戏知识）。于是目录落在本 crate：
//!
//! - Core 只认识「一个合法的 slug 字符串」（[`orbis_core::GameId`]）
//! - providers 是唯一允许出现具体游戏知识的地方（04 §4.2）
//! - UI 通过 `listGames()` 取得显示名，**不得自带一份目录**（架构断言 [8]）
//!
//! # 目录里没有什么
//!
//! - **`hasBundledComponent` 不在这里**。它由「该游戏是否有依赖独立下载资产的工具」
//!   派生（契约 §6 `GameCatalogEntry`），而资产依赖是 Manifest 的事实。本 crate
//!   禁止依赖 `orbis-tools`（04 §9.1 不变量 [3]），因此这个字段**只能在同时看得见
//!   两边的地方（壳层）拼装** —— 见 `src-tauri/src/dto.rs`。若在此处写死一个副本，
//!   新增/删除 L3 工具时就会与 Manifest 漂移且无人发现。
//! - **版本、安装状态、区服可用性**：全都不属于静态目录。
//!
//! # 待实测项
//!
//! `official_url` 的取值由实测项 **T8** 核实（契约 §3.1 / 附录 v1.1）。在 T8 收敛前
//! 它们按厂商公开入口填写，不据此做任何自动行为（不给「一键前往」以外的用途）。

use orbis_core::Region;

/// 一个游戏目录条目（`GameCatalogEntry` 的静态部分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameEntry {
    /// 契约 §2 的 `GameId` slug
    pub id: &'static str,
    /// 中文名（UI 主标题）
    pub name: &'static str,
    /// 英文名（UI 副标题 / 检索）
    pub en_name: &'static str,
    /// 发行商（展示用；**不用来做任何判定**）
    pub publisher: &'static str,
    /// 该游戏已知区服。同款多区服 = 多个 GameInstallation（02 A1）
    pub regions: &'static [Region],
    /// 官方入口（A1 空状态引导）。`None` = 暂无公开入口，UI 不显示该入口
    pub official_url: Option<&'static str>,
}

/// MVP 的 5 款游戏（04 §4.2 装配表）。
///
/// 顺序即 UI 默认展示顺序（首页「全部游戏」网格），改动它属于产品决定。
/// 区服口径与 `src/api/mock.ts` 的参照实现一致 —— mock 是契约的行为参照，
/// 两者不一致时**先确认哪个对**，不要各自修改。
pub const GAME_CATALOG: &[GameEntry] = &[
    GameEntry {
        id: "wuthering-waves",
        name: "鸣潮",
        en_name: "Wuthering Waves",
        publisher: "库洛游戏",
        regions: &[Region::Cn, Region::Global],
        official_url: Some("https://mc.kurogames.com/"),
    },
    GameEntry {
        id: "genshin-impact",
        name: "原神",
        en_name: "Genshin Impact",
        publisher: "米哈游",
        regions: &[Region::Cn, Region::Global, Region::Bili],
        official_url: Some("https://ys.mihoyo.com/"),
    },
    GameEntry {
        id: "honkai-star-rail",
        name: "崩坏：星穹铁道",
        en_name: "Honkai: Star Rail",
        publisher: "米哈游",
        regions: &[Region::Cn, Region::Global],
        official_url: Some("https://sr.mihoyo.com/"),
    },
    GameEntry {
        id: "zenless-zone-zero",
        name: "绝区零",
        en_name: "Zenless Zone Zero",
        publisher: "米哈游",
        regions: &[Region::Cn, Region::Global],
        official_url: Some("https://zzz.mihoyo.com/"),
    },
    GameEntry {
        id: "arknights-endfield",
        name: "明日方舟：终末地",
        en_name: "Arknights: Endfield",
        publisher: "鹰角网络",
        regions: &[Region::Cn],
        official_url: Some("https://endfield.hypergryph.com/"),
    },
];

/// 按 slug 取目录条目。未知 slug → `None`（调用方据此返回 `GAME_NOT_FOUND`）。
pub fn find(id: &str) -> Option<&'static GameEntry> {
    GAME_CATALOG.iter().find(|entry| entry.id == id)
}

/// 目录中是否存在该 slug（用于入参校验，比 `find(..).is_some()` 意图更清楚）。
pub fn contains(id: &str) -> bool {
    find(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbis_core::GameId;

    /// 契约 §2 的 `GameId` 并集 —— 目录必须**恰好**覆盖它。
    /// 若将来新增游戏，这条断言会先红，提醒同步契约 §2 的 TS 联合类型。
    const CONTRACT_GAME_IDS: [&str; 5] = [
        "genshin-impact",
        "honkai-star-rail",
        "zenless-zone-zero",
        "wuthering-waves",
        "arknights-endfield",
    ];

    #[test]
    fn catalog_covers_exactly_the_contract_game_ids() {
        let mut catalog_ids: Vec<&str> = GAME_CATALOG.iter().map(|e| e.id).collect();
        let mut expected: Vec<&str> = CONTRACT_GAME_IDS.to_vec();
        catalog_ids.sort_unstable();
        expected.sort_unstable();
        assert_eq!(
            catalog_ids, expected,
            "目录与契约 §2 的 GameId 并集必须一致（双源漂移在此暴露）"
        );
    }

    #[test]
    fn ids_are_valid_slugs_and_unique() {
        for entry in GAME_CATALOG {
            assert!(
                GameId::new(entry.id).is_some(),
                "目录出现非法 slug：{:?}",
                entry.id
            );
        }
        let mut ids: Vec<&str> = GAME_CATALOG.iter().map(|e| e.id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "目录存在重复 slug");
    }

    #[test]
    fn every_entry_has_display_text_and_regions() {
        for entry in GAME_CATALOG {
            assert!(!entry.name.trim().is_empty(), "{} 缺少中文名", entry.id);
            assert!(!entry.en_name.trim().is_empty(), "{} 缺少英文名", entry.id);
            assert!(
                !entry.publisher.trim().is_empty(),
                "{} 缺少发行商",
                entry.id
            );
            assert!(
                !entry.regions.is_empty(),
                "{} 的区服不得为空（A1 需要至少一个已知区服）",
                entry.id
            );
            // 区服不得重复 —— 重复会让「同款多区服」被算成同一实例
            let mut regions: Vec<&str> = entry.regions.iter().map(|r| r.slug()).collect();
            let before = regions.len();
            regions.sort_unstable();
            regions.dedup();
            assert_eq!(regions.len(), before, "{} 的区服出现重复", entry.id);
        }
    }

    #[test]
    fn official_urls_are_absolute_https_or_absent() {
        for entry in GAME_CATALOG {
            if let Some(url) = entry.official_url {
                assert!(
                    url.starts_with("https://"),
                    "{} 的官方入口必须是 https 绝对地址：{url:?}",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn find_is_exhaustive_over_the_catalog() {
        for entry in GAME_CATALOG {
            let found = find(entry.id).expect("目录内条目应可取出");
            assert_eq!(found.id, entry.id);
            assert!(contains(entry.id));
        }
        // 未知 slug 不得返回 Some —— 否则入参校验会失效
        assert!(find("no-such-game").is_none());
        assert!(!contains("no-such-game"));
        assert!(!contains("Genshin-Impact"), "校验须区分大小写");
    }
}
