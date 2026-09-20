//! 兼容性种子表加载（`pm/04-技术设计.md` §5.6 / §6.2）。
//!
//! # 权威关系（不得引入第二个数据源）
//!
//! 兼容性的唯一权威是 `data/compatibility/seed.json`。Tool Manifest **不承载**
//! 兼容性字段（04 §5.7 注②）；本模块是它的唯一解析入口。
//!
//! # 打包方式
//!
//! 按 04 §6.2 用 `include_str!` 把数据烘进二进制（随发行包内置、只读加载）。
//! 这不违反「Core 零游戏知识」：游戏标识出现在**数据文件**里，不出现在本 crate
//! 的源码中 —— `scripts/check-architecture.sh` 的 [1] 只扫 `.rs` / `.toml` 源文本。
//!
//! # 降级契约（02 C4 验收）
//!
//! `schema_version` 不符 / JSON 损坏 / 条目非法 → **降级为空表**，于是所有查询
//! 落到 `CompatStatus::Unknown`（默认安全态），**绝不 panic、绝不猜测**。
//! 失败原因通过 [`SeedTable::builtin_or_degraded`] 交回调用方写入 D3 日志
//! （04 §8 总原则：任何降级必须显式可见，禁止静默失败）。

use std::fmt;

use serde::Deserialize;

use crate::compat::{CompatEntry, CompatRecord, CompatStatus, VersionMatch};
use crate::model::GameId;
use crate::tool::{is_valid_tool_id, RiskLevel};
use crate::version::{is_prefix_key, Version};

/// 本加载器支持的 seed schema 版本。
///
/// 必须与 `data/compatibility/seed.schema.json` 的 `schema_version.const` 一致；
/// 二者不等时文件会被降级为空表（宁可全部 Unknown，也不按错误的字段口径解析）。
pub const SUPPORTED_SCHEMA_VERSION: &str = "0.1.0";

/// seed 加载失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedError {
    /// JSON 语法错误或结构不符（含 schema 的 `additionalProperties: false` 违例 ——
    /// Rust 侧用 `deny_unknown_fields` 复现该严格性）
    Malformed { detail: String },
    /// `schema_version` 与当前加载器不符
    UnsupportedSchema {
        found: String,
        expected: &'static str,
    },
    /// 某条目字段非法（`index` 为 `entries` 数组下标，便于定位数据错误）
    InvalidEntry { index: usize, detail: String },
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { detail } => write!(f, "seed.json 结构非法：{detail}"),
            Self::UnsupportedSchema { found, expected } => write!(
                f,
                "seed schema_version 不符：文件为 {found:?}，加载器支持 {expected:?}"
            ),
            Self::InvalidEntry { index, detail } => {
                write!(f, "seed entries[{index}] 非法：{detail}")
            }
        }
    }
}

impl std::error::Error for SeedError {}

/// 内存中的种子表（只读索引，量级 < 100 条，MVP 不入库 —— 04 §6.2）。
///
/// `schema_version` 为 `None` 表示**降级态**（文件缺失 / 损坏 / 版本不符），
/// 与契约 `CompatibilityDto.seedSchemaVersion: string | null` 一一对应。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeedTable {
    schema_version: Option<String>,
    updated_at: Option<String>,
    maintainer: Option<String>,
    entries: Vec<CompatEntry>,
}

impl SeedTable {
    /// 空表 = 降级态：所有 [`crate::compat::query`] 都会落到 `Unknown`。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 解析种子表文本。失败一律 `Err`（由调用方决定如何降级）。
    pub fn parse(json: &str) -> Result<Self, SeedError> {
        let RawSeed {
            schema_version,
            updated_at,
            maintainer,
            entries: raw_entries,
        } = serde_json::from_str::<RawSeed>(json).map_err(|e| SeedError::Malformed {
            detail: e.to_string(),
        })?;

        if schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(SeedError::UnsupportedSchema {
                found: schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }

        let mut entries = Vec::with_capacity(raw_entries.len());
        for (index, entry) in raw_entries.into_iter().enumerate() {
            entries.push(convert_entry(index, entry)?);
        }

        Ok(Self {
            schema_version: Some(schema_version),
            updated_at: Some(updated_at),
            maintainer,
            entries,
        })
    }

    /// 加载随二进制打包的内置种子表（04 §6.2 `include_str!`）。
    pub fn builtin() -> Result<Self, SeedError> {
        Self::parse(include_str!("../../../data/compatibility/seed.json"))
    }

    /// **推荐入口**：加载内置种子表，失败则降级为空表并交回原因。
    ///
    /// 返回的 `Some(err)` 是「降级发生了」的信号，调用方必须把它写进日志并让 UI
    /// 可见 —— 返回类型刻意不是 `Self`，就是为了让静默降级写不出来。
    pub fn builtin_or_degraded() -> (Self, Option<SeedError>) {
        Self::parse_or_degraded(include_str!("../../../data/compatibility/seed.json"))
    }

    /// 降级组合：任何失败 → 空表 + 原因。`builtin_or_degraded` 与单测共用本实现，
    /// 保证「真实数据路径」与「被测路径」是同一条。
    fn parse_or_degraded(json: &str) -> (Self, Option<SeedError>) {
        match Self::parse(json) {
            Ok(table) => (table, None),
            Err(err) => (Self::empty(), Some(err)),
        }
    }

    /// 文件声明的 schema 版本；`None` = 降级态（契约 `seedSchemaVersion: null`）。
    pub fn schema_version(&self) -> Option<&str> {
        self.schema_version.as_deref()
    }

    /// 种子表维护者标注（`updated_at` 同理由 [`Self::updated_at`] 取）。
    pub fn maintainer(&self) -> Option<&str> {
        self.maintainer.as_deref()
    }

    pub fn updated_at(&self) -> Option<&str> {
        self.updated_at.as_deref()
    }

    pub fn entries(&self) -> &[CompatEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 是否处于降级态（版本元信息缺失即视为降级）。
    pub fn is_degraded(&self) -> bool {
        self.schema_version.is_none()
    }

    /// 按「游戏 × 工具」取条目。查不到 → `None`，`query()` 会把它转成 `Unknown`。
    pub fn entry(&self, game_id: &str, tool_id: &str) -> Option<&CompatEntry> {
        self.entries
            .iter()
            .find(|e| e.game_id == game_id && e.tool_id == tool_id)
    }
}

// ── 反序列化中间结构 ─────────────────────────────────────────
//
// 与 JSON 逐字对应，且 `deny_unknown_fields` 复现 schema 的 `additionalProperties: false`。
// 转换到领域类型时才做校验，这样报错能带上条目下标与具体字段。

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeed {
    schema_version: String,
    updated_at: String,
    #[serde(default)]
    maintainer: Option<String>,
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    game_id: String,
    tool_id: String,
    /// 展示用；**权威值在 Tool Manifest**（seed.schema.json 原文）。
    /// 这里解析但不持有 —— 校验形态可以挡住 `"L9"` 这类拼写错误，
    /// 而把它存进 `CompatEntry` 会制造一个「看起来也是权威」的副本。
    #[serde(default)]
    risk_level: Option<String>,
    compatibility: Vec<RawRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecord {
    game_version: String,
    /// 缺省 = `exact`（schema `default: "exact"`）
    #[serde(default)]
    version_match: Option<String>,
    status: String,
    #[serde(default)]
    verified_at: Option<String>,
    #[serde(default)]
    verified_by: Option<String>,
    #[serde(default)]
    evidence: Option<Vec<String>>,
    #[serde(default)]
    notes: Option<String>,
}

fn convert_entry(index: usize, entry: RawEntry) -> Result<CompatEntry, SeedError> {
    let invalid = |detail: String| SeedError::InvalidEntry { index, detail };

    // 只校验形态，不校验存在性 —— 游戏清单不在 Core（04 §9.1 规则 1）
    if GameId::new(&entry.game_id).is_none() {
        return Err(invalid(format!(
            "game_id 须为合法 slug：{:?}",
            entry.game_id
        )));
    }
    if !is_valid_tool_id(&entry.tool_id) {
        return Err(invalid(format!(
            "tool_id 须为 namespace/name 形态：{:?}",
            entry.tool_id
        )));
    }
    if let Some(level) = entry.risk_level.as_deref() {
        if RiskLevel::from_literal(level).is_none() {
            return Err(invalid(format!("risk_level 非法：{level:?}")));
        }
    }
    if entry.compatibility.is_empty() {
        return Err(invalid(
            "compatibility 不得为空（schema minItems: 1）".into(),
        ));
    }

    let mut records = Vec::with_capacity(entry.compatibility.len());
    for (pos, record) in entry.compatibility.into_iter().enumerate() {
        records.push(convert_record(index, pos, record)?);
    }

    Ok(CompatEntry::new(&entry.game_id, &entry.tool_id, records))
}

fn convert_record(index: usize, pos: usize, record: RawRecord) -> Result<CompatRecord, SeedError> {
    let invalid = |detail: String| SeedError::InvalidEntry {
        index,
        detail: format!("compatibility[{pos}] {detail}"),
    };

    let version_match = match record.version_match.as_deref() {
        None | Some("exact") => VersionMatch::Exact,
        Some("prefix") => VersionMatch::Prefix,
        Some(other) => {
            return Err(invalid(format!(
                "version_match 非法：{other:?}（仅 exact / prefix）"
            )))
        }
    };

    let status = CompatStatus::from_seed_literal(&record.status)
        .ok_or_else(|| invalid(format!("status 非法：{:?}（区分大小写）", record.status)))?;

    // 版本键形态与匹配方式必须自洽：exact 走 major.minor，prefix 走 N.x。
    // 不校验的话，写成 prefix 却是 "3.5" 的条目会永远匹配不上而**静默失效**。
    match version_match {
        VersionMatch::Exact => {
            if Version::parse(&record.game_version).is_none() {
                return Err(invalid(format!(
                    "exact 条目的 game_version 须为 major.minor：{:?}",
                    record.game_version
                )));
            }
        }
        VersionMatch::Prefix => {
            if !is_prefix_key(&record.game_version) {
                return Err(invalid(format!(
                    "prefix 条目的 game_version 须为 N.x：{:?}",
                    record.game_version
                )));
            }
        }
    }

    let record = match version_match {
        VersionMatch::Exact => CompatRecord::exact(&record.game_version, status),
        VersionMatch::Prefix => CompatRecord::prefix(&record.game_version, status),
    }
    .with_evidence(record.evidence.unwrap_or_default())
    .with_notes_opt(record.notes)
    .with_verification(record.verified_at, record.verified_by);

    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{query, MatchKind};
    use crate::version::normalize;

    // 与 compat.rs 同一纪律：Core 的单测只验证**规则**，不验证某款游戏的数据。
    // 真实 seed.json 由 `builtin_seed_is_loadable` 断言「可加载」，其内容正确性
    // 由 `npm run validate:data`（跨文件一致性）守护。

    const SAMPLE: &str = r#"{
      "schema_version": "0.1.0",
      "updated_at": "2026-09-18",
      "maintainer": "orbis-maintainer",
      "entries": [
        {
          "game_id": "sample-game",
          "tool_id": "sample-ns/sample-tool",
          "risk_level": "L1",
          "compatibility": [
            {
              "game_version": "2.7",
              "status": "Verified",
              "verified_at": "2026-09-18",
              "verified_by": "P0-X §6.3",
              "evidence": ["source-a", "source-b"],
              "notes": "直接写数值"
            },
            {
              "game_version": "3.x",
              "version_match": "prefix",
              "status": "Unknown",
              "notes": "未实测"
            }
          ]
        }
      ]
    }"#;

    #[test]
    fn parses_a_well_formed_table_and_preserves_verification_metadata() {
        let table = SeedTable::parse(SAMPLE).expect("示例表应可解析");
        assert_eq!(table.schema_version(), Some("0.1.0"));
        assert_eq!(table.maintainer(), Some("orbis-maintainer"));
        assert_eq!(table.updated_at(), Some("2026-09-18"));
        assert!(!table.is_degraded());

        let entry = table
            .entry("sample-game", "sample-ns/sample-tool")
            .expect("条目应可取出");
        assert_eq!(entry.compatibility.len(), 2);

        let exact = &entry.compatibility[0];
        assert_eq!(exact.version_match, VersionMatch::Exact);
        assert_eq!(exact.status, CompatStatus::Verified);
        assert_eq!(exact.verified_at.as_deref(), Some("2026-09-18"));
        assert_eq!(exact.verified_by.as_deref(), Some("P0-X §6.3"));
        assert_eq!(exact.evidence.len(), 2);

        let prefix = &entry.compatibility[1];
        assert_eq!(prefix.version_match, VersionMatch::Prefix);
        assert!(prefix.evidence.is_empty(), "缺省 evidence 应为空列表");
        assert!(prefix.verified_at.is_none(), "缺省 verified_at 应为 None");
    }

    #[test]
    fn loaded_records_feed_the_compat_engine_end_to_end() {
        let table = SeedTable::parse(SAMPLE).unwrap();
        let entry = table.entry("sample-game", "sample-ns/sample-tool");

        // exact 命中，且验证元信息一路透传到查询结果（契约 CompatibilityDto 需要）
        let hit = query(entry, normalize("2.7.1.9"));
        assert_eq!(hit.status, CompatStatus::Verified);
        assert_eq!(hit.match_kind, MatchKind::Exact);
        assert_eq!(hit.verified_at.as_deref(), Some("2026-09-18"));
        assert_eq!(hit.verified_by.as_deref(), Some("P0-X §6.3"));

        // prefix 兜底 → Unknown（B6 门控触发点）
        assert_eq!(query(entry, normalize("3.4")).status, CompatStatus::Unknown);
        assert_eq!(query(entry, normalize("3.4")).match_kind, MatchKind::Prefix);
    }

    #[test]
    fn degraded_table_makes_every_query_unknown() {
        // 02 C4 验收：种子表缺失 / 损坏 → 全部 Unknown，**不崩溃**
        let table = SeedTable::empty();
        assert!(table.is_degraded());
        assert_eq!(table.schema_version(), None);
        assert!(table.entries().is_empty());

        let hit = query(
            table.entry("sample-game", "sample-ns/sample-tool"),
            normalize("2.7"),
        );
        assert_eq!(hit.status, CompatStatus::Unknown);
        assert_eq!(hit.match_kind, MatchKind::None);
        assert!(hit.verified_at.is_none());
    }

    #[test]
    fn unsupported_schema_version_is_rejected_not_guessed() {
        let json = SAMPLE.replace("\"0.1.0\"", "\"0.2.0\"");
        match SeedTable::parse(&json) {
            Err(SeedError::UnsupportedSchema { found, expected }) => {
                assert_eq!(found, "0.2.0");
                assert_eq!(expected, SUPPORTED_SCHEMA_VERSION);
            }
            other => panic!("应按版本不符拒绝，实际：{other:?}"),
        }
    }

    #[test]
    fn malformed_json_is_rejected() {
        assert!(matches!(
            SeedTable::parse("{ not json"),
            Err(SeedError::Malformed { .. })
        ));
        // schema 的 additionalProperties: false 必须同样生效
        let with_extra =
            SAMPLE.replace("\"maintainer\"", "\"unexpected_field\": 1, \"maintainer\"");
        assert!(matches!(
            SeedTable::parse(&with_extra),
            Err(SeedError::Malformed { .. })
        ));
    }

    #[test]
    fn invalid_entries_report_their_index() {
        // 非法工具标识
        let bad_tool = SAMPLE.replace("sample-ns/sample-tool", "no-namespace");
        match SeedTable::parse(&bad_tool) {
            Err(SeedError::InvalidEntry { index, detail }) => {
                assert_eq!(index, 0);
                assert!(detail.contains("tool_id"), "detail={detail}");
            }
            other => panic!("应拒绝非法 tool_id，实际：{other:?}"),
        }

        // 大小写错误的 status 不得静默通过（否则数据错误会被掩盖）
        let bad_status = SAMPLE.replace("\"Verified\"", "\"verified\"");
        assert!(matches!(
            SeedTable::parse(&bad_status),
            Err(SeedError::InvalidEntry { .. })
        ));

        // 非法风险等级拼写
        let bad_risk = SAMPLE.replace("\"L1\"", "\"L9\"");
        assert!(matches!(
            SeedTable::parse(&bad_risk),
            Err(SeedError::InvalidEntry { .. })
        ));
    }

    #[test]
    fn version_key_shape_must_match_the_match_kind() {
        // prefix 条目写成 "3.5" → 永远匹配不上，属静默失效，必须在加载期拦下
        let bad_prefix = SAMPLE.replace("\"game_version\": \"3.x\"", "\"game_version\": \"3.5\"");
        assert!(matches!(
            SeedTable::parse(&bad_prefix),
            Err(SeedError::InvalidEntry { .. })
        ));

        // exact 条目写成 "2.x" → 同样拦下
        let bad_exact = SAMPLE.replace("\"game_version\": \"2.7\"", "\"game_version\": \"2.x\"");
        assert!(matches!(
            SeedTable::parse(&bad_exact),
            Err(SeedError::InvalidEntry { .. })
        ));
    }

    #[test]
    fn empty_compatibility_list_is_rejected() {
        let json = r#"{
          "schema_version": "0.1.0",
          "updated_at": "2026-09-18",
          "entries": [
            { "game_id": "sample-game", "tool_id": "sample-ns/sample-tool", "compatibility": [] }
          ]
        }"#;
        assert!(matches!(
            SeedTable::parse(json),
            Err(SeedError::InvalidEntry { .. })
        ));
    }

    #[test]
    fn version_match_defaults_to_exact_when_absent() {
        // schema `default: "exact"` —— 缺省必须是 exact，且此时版本键须为 major.minor
        let json = r#"{
          "schema_version": "0.1.0",
          "updated_at": "2026-09-18",
          "entries": [
            {
              "game_id": "sample-game",
              "tool_id": "sample-ns/sample-tool",
              "compatibility": [
                { "game_version": "2.7", "status": "Verified" },
                { "game_version": "4.1", "version_match": "exact", "status": "Unknown" }
              ]
            }
          ]
        }"#;
        let table = SeedTable::parse(json).expect("缺省 version_match 应可解析");
        let entry = table.entry("sample-game", "sample-ns/sample-tool").unwrap();
        assert_eq!(entry.compatibility[0].version_match, VersionMatch::Exact);
        assert_eq!(entry.compatibility[1].version_match, VersionMatch::Exact);
    }

    #[test]
    fn exact_record_with_prefix_shaped_key_is_rejected() {
        // 反向用例：删掉 version_match 却留下 "3.x"，会变成「永远匹配不上的 exact 条目」，
        // 属于静默失效，必须在加载期拦下（此用例由一次真实的测试失败暴露出来）。
        let json = r#"{
          "schema_version": "0.1.0",
          "updated_at": "2026-09-18",
          "entries": [
            {
              "game_id": "sample-game",
              "tool_id": "sample-ns/sample-tool",
              "compatibility": [ { "game_version": "3.x", "status": "Unknown" } ]
            }
          ]
        }"#;
        assert!(matches!(
            SeedTable::parse(json),
            Err(SeedError::InvalidEntry { .. })
        ));
    }

    #[test]
    fn degraded_load_returns_empty_table_plus_reason() {
        // 降级路径与真实数据路径共用同一实现，因此这里测的就是生产代码走的路
        let (table, err) = SeedTable::parse_or_degraded("{ broken");
        assert!(matches!(err, Some(SeedError::Malformed { .. })));
        assert!(table.is_degraded());
        assert!(table.entries().is_empty());
        assert_eq!(table.schema_version(), None);
    }

    #[test]
    fn builtin_seed_is_loadable() {
        // 断言的是「随包数据能被加载器接受」，不是它的内容 ——
        // 内容正确性（含 5 个 game slug 与工具条目）由 npm run validate:data 守护，
        // 此处刻意不出现任何游戏标识（架构不变量，check-architecture.sh [1]）。
        let table = SeedTable::builtin().expect("内置 seed.json 必须能被加载器解析");
        assert_eq!(table.schema_version(), Some(SUPPORTED_SCHEMA_VERSION));
        assert!(!table.is_empty());
        assert!(!table.is_degraded());

        let (_, err) = SeedTable::builtin_or_degraded();
        assert!(err.is_none(), "内置数据不应触发降级：{err:?}");
    }
}
