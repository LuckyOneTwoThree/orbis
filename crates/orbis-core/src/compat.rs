//! 兼容性匹配引擎（`pm/Phase0-冲刺C-F2种子表定稿.md` §2.2 / `pm/04-技术设计.md` §5.6）。
//!
//! # 权威关系
//!
//! 兼容性的**唯一权威**是 `data/compatibility/seed.json`。Tool Manifest 不承载兼容性字段，
//! 避免双源冲突（04 §5.7 注②）。
//!
//! # 匹配规则（优先级自上而下）
//!
//! 1. `exact` 命中当前版本 → 返回该条目状态
//! 2. `prefix` 命中（`"3.x"` 覆盖 3.0–3.9）→ 返回该条目状态
//! 3. 无命中 / 本地版本未知 / 无条目 → `Unknown`（**默认安全态**）
//!
//! 第 3 条正是「游戏一更新就自动落入未验证」的实现机制，也是 B6 提示逻辑的触发点，
//! 因此**不得**为「避免 Unknown 拦截」而在匹配失败时降级为 Compatible。

use crate::version::Version;

/// 兼容五态（`pm/00-产品基石.md` §8.8）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatStatus {
    /// 指定版本有**直接验证证据**
    Verified,
    /// 原理 / 上游声明推断可用，**未实测**
    Compatible,
    /// 无条目或未验证 —— 默认安全态
    Unknown,
    /// 已知失效 → 硬阻止
    Incompatible,
    /// 工具被替代 → 硬阻止 + 迁移指引
    Deprecated,
}

impl CompatStatus {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Compatible => "compatible",
            Self::Unknown => "unknown",
            Self::Incompatible => "incompatible",
            Self::Deprecated => "deprecated",
        }
    }

    /// 从 seed.json 的首字母大写形式解析（`Verified` / `Compatible` / …）。
    pub fn from_seed_literal(literal: &str) -> Option<Self> {
        match literal {
            "Verified" => Some(Self::Verified),
            "Compatible" => Some(Self::Compatible),
            "Unknown" => Some(Self::Unknown),
            "Incompatible" => Some(Self::Incompatible),
            "Deprecated" => Some(Self::Deprecated),
            _ => None,
        }
    }

    /// 是否允许直接启用（04 §5.5 门控的输入）。
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Verified | Self::Compatible)
    }

    /// 是否为硬阻止态（Incompatible / Deprecated 一律阻止，无覆盖入口）。
    pub const fn is_hard_blocked(self) -> bool {
        matches!(self, Self::Incompatible | Self::Deprecated)
    }
}

/// 条目匹配方式（seed 的 `version_match` 字段，缺省 = exact）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionMatch {
    Exact,
    Prefix,
}

/// 查询结果命中的层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Exact,
    Prefix,
    /// 无命中 —— 必与 `CompatStatus::Unknown` 同时出现
    None,
}

/// seed 中的一条兼容记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatRecord {
    /// `exact` 时形如 `7.0`；`prefix` 时形如 `3.x`
    pub game_version: String,
    pub version_match: VersionMatch,
    pub status: CompatStatus,
    pub notes: Option<String>,
}

impl CompatRecord {
    pub fn exact(game_version: &str, status: CompatStatus) -> Self {
        Self {
            game_version: game_version.to_owned(),
            version_match: VersionMatch::Exact,
            status,
            notes: None,
        }
    }

    pub fn prefix(game_version: &str, status: CompatStatus) -> Self {
        Self {
            game_version: game_version.to_owned(),
            version_match: VersionMatch::Prefix,
            status,
            notes: None,
        }
    }

    pub fn with_notes(mut self, notes: &str) -> Self {
        self.notes = Some(notes.to_owned());
        self
    }

    fn matches(&self, local: Version) -> bool {
        match self.version_match {
            VersionMatch::Exact => Version::parse(&self.game_version) == Some(local),
            VersionMatch::Prefix => local.matches_prefix(&self.game_version),
        }
    }
}

/// seed 中的一个「游戏 × 工具」条目。
///
/// `game_id` / `tool_id` 保留为字符串：它们**逐字镜像 seed.json 的字段**，
/// 由加载器负责校验（工具标识形如 `namespace/name`，游戏标识须存在于 providers 的
/// catalog 中）。匹配引擎本身不做存在性判断，也不持有任何具体游戏知识。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatEntry {
    pub game_id: String,
    pub tool_id: String,
    pub compatibility: Vec<CompatRecord>,
}

impl CompatEntry {
    pub fn new(game_id: &str, tool_id: &str, compatibility: Vec<CompatRecord>) -> Self {
        Self {
            game_id: game_id.to_owned(),
            tool_id: tool_id.to_owned(),
            compatibility,
        }
    }
}

/// 查询结果（对应契约 §6 的 `CompatibilityDto`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compatibility {
    pub status: CompatStatus,
    pub match_kind: MatchKind,
    pub matched_version_key: Option<String>,
    pub notes: Option<String>,
}

impl Compatibility {
    fn unknown(notes: &str) -> Self {
        Self {
            status: CompatStatus::Unknown,
            match_kind: MatchKind::None,
            matched_version_key: None,
            notes: Some(notes.to_owned()),
        }
    }
}

/// 兼容性查询：`exact → prefix → Unknown`。
///
/// `local` 为 `None` 表示本地版本未知（02 A3）——此时**不得**猜一个版本去匹配，
/// 必须直接落到 `Unknown`。
pub fn query(entry: Option<&CompatEntry>, local: Option<Version>) -> Compatibility {
    let Some(entry) = entry else {
        return Compatibility::unknown("种子表无该工具条目");
    };

    let Some(local) = local else {
        return Compatibility::unknown("本地版本未知，无法判定");
    };

    // 1. exact 优先
    if let Some(hit) = entry
        .compatibility
        .iter()
        .find(|r| r.version_match == VersionMatch::Exact && r.matches(local))
    {
        return Compatibility {
            status: hit.status,
            match_kind: MatchKind::Exact,
            matched_version_key: Some(hit.game_version.clone()),
            notes: hit.notes.clone(),
        };
    }

    // 2. prefix 兜底
    if let Some(hit) = entry
        .compatibility
        .iter()
        .find(|r| r.version_match == VersionMatch::Prefix && r.matches(local))
    {
        return Compatibility {
            status: hit.status,
            match_kind: MatchKind::Prefix,
            matched_version_key: Some(hit.game_version.clone()),
            notes: hit.notes.clone(),
        };
    }

    // 3. 默认安全态
    Compatibility::unknown(&format!("种子表无 {local} 条目，落到默认安全态"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::normalize;

    // 刻意使用中性标识：Core 的单测应验证**规则**，不是验证某款游戏的数据。
    // 真实数据（含 5 个 game slug）在 data/compatibility/seed.json 与 orbis-providers，
    // 由 `npm run validate:data` 与 providers 侧的集成测试守护。

    /// 形态 A：exact 有证据 + prefix 兜底为 Unknown（对应 P0-C §3 的「L1 标杆」形态）
    fn entry_exact_then_prefix_unknown() -> CompatEntry {
        CompatEntry::new(
            "sample-game",
            "sample-ns/sample-tool",
            vec![
                CompatRecord::exact("2.7", CompatStatus::Verified),
                CompatRecord::prefix("3.x", CompatStatus::Unknown)
                    .with_notes("3.x 未实测；待首测后升级"),
            ],
        )
    }

    /// 形态 B：exact + prefix 双 Compatible（对应 P0-C v0.2 的 prefix 兜底层）
    fn entry_prefix_backstop() -> CompatEntry {
        CompatEntry::new(
            "other-game",
            "sample-ns/other-tool",
            vec![
                CompatRecord::exact("7.0", CompatStatus::Compatible),
                CompatRecord::prefix("7.x", CompatStatus::Compatible),
            ],
        )
    }

    #[test]
    fn exact_hit_wins_over_prefix() {
        let e = entry_exact_then_prefix_unknown();
        let r = query(Some(&e), normalize("2.7.1.9"));
        assert_eq!(r.status, CompatStatus::Verified);
        assert_eq!(r.match_kind, MatchKind::Exact);
        assert_eq!(r.matched_version_key.as_deref(), Some("2.7"));
    }

    #[test]
    fn prefix_fallback_covers_whole_minor_range() {
        let e = entry_exact_then_prefix_unknown();
        for raw in ["3.0", "3.5.0.128940", "3.9"] {
            let r = query(Some(&e), normalize(raw));
            assert_eq!(r.status, CompatStatus::Unknown, "raw={raw}");
            assert_eq!(r.match_kind, MatchKind::Prefix, "raw={raw}");
            assert_eq!(r.matched_version_key.as_deref(), Some("3.x"), "raw={raw}");
        }
    }

    #[test]
    fn outside_all_records_falls_to_unknown() {
        let e = entry_exact_then_prefix_unknown();
        let r = query(Some(&e), normalize("4.0"));
        assert_eq!(r.status, CompatStatus::Unknown);
        assert_eq!(r.match_kind, MatchKind::None);
        assert!(r.matched_version_key.is_none());
    }

    #[test]
    fn unknown_if_local_version_unknown() {
        // 02 A3：不猜版本，直接 Unknown（而不是拿某个条目硬套）
        let e = entry_prefix_backstop();
        let r = query(Some(&e), None);
        assert_eq!(r.status, CompatStatus::Unknown);
        assert_eq!(r.match_kind, MatchKind::None);
    }

    #[test]
    fn missing_entry_never_panics() {
        let r = query(None, normalize("1.0"));
        assert_eq!(r.status, CompatStatus::Unknown);
        assert_eq!(r.match_kind, MatchKind::None);
    }

    #[test]
    fn prefix_backstop_keeps_live_versions_usable() {
        // P0-C v0.2 的教训：只有 exact 条目时，live 7.1+ 会常态 Unknown 而拦死工具
        let e = entry_prefix_backstop();
        for raw in ["7.0", "7.1", "7.9.3"] {
            let r = query(Some(&e), normalize(raw));
            assert_eq!(r.status, CompatStatus::Compatible, "raw={raw}");
        }
        // 走出区间 → 回到默认安全态
        assert_eq!(query(Some(&e), normalize("8.0")).status, CompatStatus::Unknown);
    }

    #[test]
    fn exact_record_does_not_leak_into_neighbouring_minor() {
        // 2.7 的 Verified 不得覆盖 2.8（exact 语义必须严格）
        let e = entry_exact_then_prefix_unknown();
        let r = query(Some(&e), normalize("2.8"));
        assert_eq!(r.status, CompatStatus::Unknown);
        assert_eq!(r.match_kind, MatchKind::None);
    }

    #[test]
    fn status_gating_semantics() {
        assert!(CompatStatus::Verified.is_usable());
        assert!(CompatStatus::Compatible.is_usable());
        assert!(
            !CompatStatus::Unknown.is_usable(),
            "Unknown 需门控（L1 可逐次覆盖 / L3 硬阻止）"
        );
        assert!(CompatStatus::Incompatible.is_hard_blocked());
        assert!(CompatStatus::Deprecated.is_hard_blocked());
    }

    #[test]
    fn seed_literal_parsing_is_exact() {
        assert_eq!(
            CompatStatus::from_seed_literal("Verified"),
            Some(CompatStatus::Verified)
        );
        // 大小写 / 空白错误不得静默通过（否则数据错误会被掩盖）
        assert_eq!(CompatStatus::from_seed_literal("verified"), None);
        assert_eq!(CompatStatus::from_seed_literal("Compatible "), None);
        assert_eq!(CompatStatus::from_seed_literal(""), None);
    }

    #[test]
    fn slugs_match_contract_literals() {
        // 契约 §2：IPC / 入库一律 snake_case 小写
        assert_eq!(CompatStatus::Verified.slug(), "verified");
        assert_eq!(CompatStatus::Compatible.slug(), "compatible");
        assert_eq!(CompatStatus::Unknown.slug(), "unknown");
        assert_eq!(CompatStatus::Incompatible.slug(), "incompatible");
        assert_eq!(CompatStatus::Deprecated.slug(), "deprecated");
    }
}
