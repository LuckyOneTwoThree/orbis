//! 枚举与标识口径（`docs/ipc-contract.md` §2 的 Rust 侧对应物）。
//!
//! # 一个刻意的设计选择：`GameId` 是不透明字符串，不是闭集枚举
//!
//! 直觉上会把 `GameId` 写成 `enum { Genshin, StarRail, ... }`。但那样做等于把
//! **具体游戏清单硬编码进 Core**，于是「新增游戏」必须修改 Core —— 直接违反
//! `pm/00-产品基石.md` §12.3 规则 1 与 §12.4 B 组不变量，也让插件化叙事失真。
//!
//! 因此 Core 只认识「一个合法的游戏标识字符串」；**游戏清单归 `orbis-providers` 的
//! 编译期 catalog**（00 §7.10：`Game` 实体为静态目录、不入库）。CI 有 grep 断言
//! 守护这条约束（`scripts/check-architecture.sh`）。
//!
//! 代价是无法对游戏做穷尽匹配 —— 但这个代价换来的正是可扩展性：
//! 新游戏的接入面 = providers 加模块 + catalog 条目 + seed 数据，Core 一行不动。

use std::fmt;
use std::str::FromStr;

/// 游戏标识（slug）。构造即校验，因此拿到本类型就代表「标识合法」。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GameId(String);

/// 标识长度上限：防止把描述性文本塞进标识位。
const MAX_SLUG_LEN: usize = 64;

impl GameId {
    /// 校验并构造。规则：非空、长度 ≤ 64、仅 ASCII 小写字母 / 数字 / `-`，
    /// 且首尾必须是字母或数字（不允许前导 / 尾随 `-`）。
    pub fn new(slug: impl AsRef<str>) -> Option<Self> {
        let slug = slug.as_ref();
        if slug.is_empty() || slug.len() > MAX_SLUG_LEN {
            return None;
        }
        if !slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return None;
        }
        if slug.starts_with('-') || slug.ends_with('-') {
            return None;
        }
        Some(Self(slug.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 兼容契约 §2 的 `namespace/name` 工具标识：校验形式（不校验是否存在）。
    pub fn is_valid_slug(slug: &str) -> bool {
        Self::new(slug).is_some()
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for GameId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for GameId {
    type Err = InvalidGameId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s).ok_or(InvalidGameId)
    }
}

/// 标识不合法（对应契约 §5 的 `GAME_NOT_FOUND` 家族中「形态非法」的情况）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidGameId;

impl fmt::Display for InvalidGameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("game id must be a non-empty lowercase slug (a-z 0-9 -)")
    }
}

impl std::error::Error for InvalidGameId {}

/// 区服。留作枚举是合理的：区服是**产品级通用概念**（3 个固定取值），
/// 不是某款游戏的知识，新增游戏不会新增区服。
///
/// 库内与跨 IPC 一律小写；「国服」这类展示文案只能在 UI 层出现（契约 §2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    Cn,
    Global,
    Bili,
}

impl Region {
    pub const ALL: [Region; 3] = [Region::Cn, Region::Global, Region::Bili];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Cn => "cn",
            Self::Global => "global",
            Self::Bili => "bili",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "cn" => Some(Self::Cn),
            "global" => Some(Self::Global),
            "bili" => Some(Self::Bili),
            _ => None,
        }
    }
}

/// 游戏运行态：**互斥主状态**（04 §6.4.1 ①）。
///
/// 与 `update_available` / `version_unknown` 是**正交**关系，不可合并为单一枚举——
/// 合并会导致「已安装且可更新」这类组合无法表达，首页聚合也会算错
/// （原前端 mock 正是这种错误建模）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRuntimeStatus {
    Installed,
    Running,
    Updating,
    Repairing,
    Broken,
}

impl GameRuntimeStatus {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Running => "running",
            Self::Updating => "updating",
            Self::Repairing => "repairing",
            Self::Broken => "broken",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "installed" => Some(Self::Installed),
            "running" => Some(Self::Running),
            "updating" => Some(Self::Updating),
            "repairing" => Some(Self::Repairing),
            "broken" => Some(Self::Broken),
            _ => None,
        }
    }
}

/// UI 主标签的展示优先级（04 §6.4.2）：数值越小越优先。
///
/// 由 Core 给出，避免 UI 各自实现一套优先级导致首页与卡片不一致。
pub const fn primary_status_rank(status: GameRuntimeStatus, update_available: bool) -> u8 {
    match status {
        GameRuntimeStatus::Running => 1,
        GameRuntimeStatus::Broken => 2,
        GameRuntimeStatus::Updating | GameRuntimeStatus::Repairing => 3,
        GameRuntimeStatus::Installed => {
            if update_available {
                4
            } else {
                5
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：本模块刻意**不出现任何真实游戏 slug** —— 那会破坏架构不变量，
    // 且 Core 的单测本就应该只验证规则本身，而不是验证数据（数据在 data/ 与 providers）。
    const SAMPLE: &str = "sample-game";

    #[test]
    fn accepts_well_formed_slugs() {
        assert_eq!(GameId::new(SAMPLE).unwrap().as_str(), SAMPLE);
        assert_eq!(GameId::new("a").unwrap().as_str(), "a");
        assert_eq!(GameId::new("game-2-x").unwrap().as_str(), "game-2-x");
    }

    #[test]
    fn rejects_malformed_slugs() {
        for bad in ["", "-lead", "trail-", "Upper", "with space", "with/slash", "中文", "a_b"] {
            assert!(GameId::new(bad).is_none(), "should reject: {bad:?}");
        }
        assert!(GameId::new("x".repeat(65)).is_none(), "超长标识应被拒绝");
    }

    #[test]
    fn from_str_reports_typed_error() {
        assert!(SAMPLE.parse::<GameId>().is_ok());
        assert_eq!("Bad".parse::<GameId>(), Err(InvalidGameId));
    }

    #[test]
    fn display_roundtrips_through_parse() {
        let id = GameId::new(SAMPLE).unwrap();
        assert_eq!(id.to_string().parse::<GameId>().unwrap(), id);
    }

    #[test]
    fn region_literals_are_lowercase_and_display_text_is_rejected() {
        assert_eq!(Region::Cn.slug(), "cn");
        assert_eq!(Region::from_slug("global"), Some(Region::Global));
        assert_eq!(Region::from_slug("bili"), Some(Region::Bili));
        // 展示文案不得作为库内值（契约 §2）
        assert_eq!(Region::from_slug("国服"), None);
        assert_eq!(Region::from_slug("CN"), None);
        assert_eq!(Region::ALL.len(), 3);
    }

    #[test]
    fn runtime_status_slug_roundtrip() {
        for s in [
            GameRuntimeStatus::Installed,
            GameRuntimeStatus::Running,
            GameRuntimeStatus::Updating,
            GameRuntimeStatus::Repairing,
            GameRuntimeStatus::Broken,
        ] {
            assert_eq!(GameRuntimeStatus::from_slug(s.slug()), Some(s));
        }
        assert_eq!(GameRuntimeStatus::from_slug("ready"), None);
    }

    #[test]
    fn running_outranks_update_available() {
        // 「正在运行 + 有更新」时，主标签必须是「运行中」（04 §6.4.2）
        assert!(
            primary_status_rank(GameRuntimeStatus::Running, true)
                < primary_status_rank(GameRuntimeStatus::Installed, true)
        );
    }

    #[test]
    fn broken_outranks_installed() {
        assert!(
            primary_status_rank(GameRuntimeStatus::Broken, false)
                < primary_status_rank(GameRuntimeStatus::Installed, false)
        );
    }

    #[test]
    fn update_available_ranks_below_running_and_broken() {
        let upd = primary_status_rank(GameRuntimeStatus::Installed, true);
        assert!(upd > primary_status_rank(GameRuntimeStatus::Broken, false));
        assert!(upd > primary_status_rank(GameRuntimeStatus::Running, false));
        assert!(upd < primary_status_rank(GameRuntimeStatus::Installed, false));
    }
}
