//! 内置 Tool Manifest 加载（`pm/04-技术设计.md` §5.7，`pm/02-MVP-PRD.md` C1）。
//!
//! # 降级契约（02 C1 验收，不得放宽）
//!
//! **单个 Manifest 损坏 → 只让该工具不可见，其余照常可用。** 因此本模块刻意
//! 不返回 `Result<Vec<_>, _>`（那会诱使调用方在第一条坏数据上直接放弃全部工具），
//! 而是返回 [`ManifestSet`]：加载成功的在 `manifests`，被拒的在 `rejected` ——
//! 后者必须被写进 D3 日志并让 UI 可见（04 §8：降级必须显式可见）。
//!
//! # 校验口径
//!
//! 数据已由 `npm run validate:data` 做 JSON Schema + 跨文件一致性校验，Rust 侧
//! 按 04 §5.7 再做一次**加载期**校验。两边都做不是冗余：`validate:data` 只在 CI
//! 跑，而这里的校验随发行包一起走，能挡住「CI 通过后数据被本地改坏」。
//! `deny_unknown_fields` 复现 schema 的 `additionalProperties: false`。

use std::fmt;

use orbis_core::{
    is_valid_tool_id, GameId, RiskLevel, SourceKind, ToolPermission, ToolSource, ToolType,
};
use serde::Deserialize;

/// 随二进制打包的 Manifest 清单（04 §5.7 `include_str!`）。
///
/// **已知限制**：新增工具需要在本列表加一行（`include_str!` 要求字面量路径）。
/// 这不违反「新增工具不需要修改 Core」——本文件在 `orbis-tools`，不在 `orbis-core`
/// （架构不变量只约束 Core）。当工具数量增长到让这行维护变成负担时，改为
/// build script 扫描 `data/tools/manifests/*.json` 生成该列表。
const BUILTIN_MANIFEST_SOURCES: &[(&str, &str)] = &[
    (
        "wuwa-fps-120.json",
        include_str!("../../../data/tools/manifests/wuwa-fps-120.json"),
    ),
    (
        "genshin-fps-unlock.json",
        include_str!("../../../data/tools/manifests/genshin-fps-unlock.json"),
    ),
];

/// Manifest 被拒的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// JSON 语法 / 结构不符
    Malformed { detail: String },
    /// 字段取值非法
    InvalidField { field: &'static str, detail: String },
    /// 与已加载的其它 Manifest 冲突（如 id 重复）
    Conflict { detail: String },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { detail } => write!(f, "结构非法：{detail}"),
            Self::InvalidField { field, detail } => write!(f, "字段 {field} 非法：{detail}"),
            Self::Conflict { detail } => write!(f, "与其它条目冲突：{detail}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// 执行器分派入口（04 §5.7 `entry`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolEntry {
    /// 执行器标识（Rust 侧分派表 key）
    pub executor: String,
    /// `external_process` 专用：资产 key，须在 `data/tools/assets.json` 中有对应条目
    pub asset: Option<String>,
    /// `external_process` 专用：分发渠道
    pub channel: Option<String>,
}

/// 单个工具的完整数据声明（`ToolDto` 的静态部分）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolManifest {
    /// `namespace/name`，与 seed.json 的 `tool_id` 必须一致
    pub id: String,
    pub game_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub tool_type: ToolType,
    pub risk_level: RiskLevel,
    pub permissions: Vec<ToolPermission>,
    pub requires_admin: bool,
    /// true → 执行链路强制 Backup 先于 Modify（00 §12.3 规则 5）
    pub backup_required: bool,
    pub entry: ToolEntry,
    pub source: ToolSource,
    /// 决定本条目取值的待实测项（如 `["T4a","T4b"]`）；非空 → UI 显示「部分参数待实测」
    pub pending_verifications: Vec<String>,
}

impl ToolManifest {
    /// 解析并校验单个 Manifest。
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest =
            serde_json::from_str(json).map_err(|e| ManifestError::Malformed {
                detail: e.to_string(),
            })?;

        let invalid =
            |field: &'static str, detail: String| ManifestError::InvalidField { field, detail };

        if !is_valid_tool_id(&raw.id) {
            return Err(invalid(
                "id",
                format!("须为 namespace/name 形态：{:?}", raw.id),
            ));
        }
        if GameId::new(&raw.game).is_none() {
            return Err(invalid("game", format!("须为合法 slug：{:?}", raw.game)));
        }
        if raw.name.trim().is_empty() {
            return Err(invalid("name", "不得为空".into()));
        }
        if raw.description.trim().is_empty() {
            return Err(invalid("description", "不得为空".into()));
        }
        if !is_semver_triplet(&raw.version) {
            return Err(invalid(
                "version",
                format!("须为 x.y.z 三段数字：{:?}", raw.version),
            ));
        }

        let tool_type = ToolType::from_slug(&raw.tool_type).ok_or_else(|| {
            invalid(
                "type",
                format!("仅 config_modify / external_process：{:?}", raw.tool_type),
            )
        })?;
        let risk_level = RiskLevel::from_literal(&raw.risk_level)
            .ok_or_else(|| invalid("risk_level", format!("非法等级：{:?}", raw.risk_level)))?;

        if raw.permissions.is_empty() {
            return Err(invalid(
                "permissions",
                "不得为空（schema minItems: 1）".into(),
            ));
        }
        let mut permissions = Vec::with_capacity(raw.permissions.len());
        for p in &raw.permissions {
            let parsed = ToolPermission::from_slug(p)
                .ok_or_else(|| invalid("permissions", format!("未知权限：{p:?}")))?;
            if permissions.contains(&parsed) {
                return Err(invalid(
                    "permissions",
                    format!("重复项：{p:?}（schema uniqueItems: true）"),
                ));
            }
            permissions.push(parsed);
        }

        // 00 §12.3 规则 5 的最后一道闸：会落盘改文件的工具，备份必须是强制项。
        // UI 侧的门禁（04 §7.4 安全模型不可关闭）依赖此值，故不得由数据决定成败。
        if tool_type.writes_game_files() && !raw.backup_required {
            return Err(invalid(
                "backup_required",
                "config_modify 必须为 true —— 否则等于允许「无备份修改」（00 §12.3 规则 5）".into(),
            ));
        }

        if raw.entry.executor.trim().is_empty() {
            return Err(invalid("entry.executor", "不得为空".into()));
        }
        // schema：executor = "unlocker" 时 asset 与 channel 为必填
        if raw.entry.executor == "unlocker"
            && (raw.entry.asset.is_none() || raw.entry.channel.is_none())
        {
            return Err(invalid(
                "entry",
                "executor = unlocker 时 asset 与 channel 为必填（发行包分离，B7）".into(),
            ));
        }

        let source_kind = SourceKind::from_slug(&raw.source.kind).ok_or_else(|| {
            invalid(
                "source.kind",
                format!("仅 builtin / bundled / upstream：{:?}", raw.source.kind),
            )
        })?;
        let source = ToolSource {
            kind: source_kind,
            repo: raw.source.repo,
            license: raw.source.license,
            version: raw.source.version,
        };
        // 00 §8.6：上游项目必须写明仓库与 License，否则无法在 NOTICE 中署名
        if !source.is_attribution_complete() {
            return Err(invalid(
                "source",
                "kind = upstream 时 repo 与 license 为必填且不得为空白（00 §8.6）".into(),
            ));
        }

        for id in &raw.pending_verifications {
            if !is_verification_id(id) {
                return Err(invalid(
                    "pending_verifications",
                    format!("须形如 T4a / T7：{id:?}"),
                ));
            }
        }

        Ok(Self {
            id: raw.id,
            game_id: raw.game,
            name: raw.name,
            description: raw.description,
            version: raw.version,
            tool_type,
            risk_level,
            permissions,
            requires_admin: raw.requires_admin,
            backup_required: raw.backup_required,
            entry: ToolEntry {
                executor: raw.entry.executor,
                asset: raw.entry.asset,
                channel: raw.entry.channel,
            },
            source,
            pending_verifications: raw.pending_verifications,
        })
    }

    /// 是否依赖独立下载的资产（`GameCatalogEntry.hasBundledComponent` 的依据）。
    pub fn requires_asset(&self) -> bool {
        self.entry.asset.is_some()
    }

    /// 参数是否尚未定稿（04 §5.7 注①：让「未定稿」对实现者显式可见）。
    pub fn has_pending_verifications(&self) -> bool {
        !self.pending_verifications.is_empty()
    }
}

/// 被拒的 Manifest —— 保留来源与原因，供 D3 日志与 UI 提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedManifest {
    /// 来源文件名（include_str 的清单键）
    pub source: String,
    pub error: ManifestError,
}

/// 一次加载的结果集：**部分成功**是正常状态，不是异常。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestSet {
    manifests: Vec<ToolManifest>,
    rejected: Vec<RejectedManifest>,
}

impl ManifestSet {
    /// 从 `(来源名, JSON 文本)` 列表加载。解析失败只跳过该条，并记入 `rejected`。
    pub fn from_sources(sources: &[(&str, &str)]) -> Self {
        let mut set = Self::default();
        for (name, json) in sources {
            match ToolManifest::parse(json) {
                Ok(manifest) => {
                    if set.get(&manifest.id).is_some() {
                        set.rejected.push(RejectedManifest {
                            source: (*name).to_owned(),
                            error: ManifestError::Conflict {
                                detail: format!("工具标识重复：{}", manifest.id),
                            },
                        });
                        continue;
                    }
                    set.manifests.push(manifest);
                }
                Err(error) => set.rejected.push(RejectedManifest {
                    source: (*name).to_owned(),
                    error,
                }),
            }
        }
        set
    }

    /// 加载随二进制打包的内置 Manifest。
    pub fn builtin() -> Self {
        Self::from_sources(BUILTIN_MANIFEST_SOURCES)
    }

    pub fn manifests(&self) -> &[ToolManifest] {
        &self.manifests
    }

    pub fn rejected(&self) -> &[RejectedManifest] {
        &self.rejected
    }

    pub fn get(&self, tool_id: &str) -> Option<&ToolManifest> {
        self.manifests.iter().find(|m| m.id == tool_id)
    }

    /// 某款游戏的工具（`listTools(gameId)` 的数据源）。
    pub fn for_game<'a>(&'a self, game_id: &'a str) -> impl Iterator<Item = &'a ToolManifest> {
        self.manifests.iter().filter(move |m| m.game_id == game_id)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ToolManifest> {
        self.manifests.iter()
    }

    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }

    /// 是否发生了任何降级（有工具因数据问题不可见）。
    pub fn is_degraded(&self) -> bool {
        !self.rejected.is_empty()
    }
}

/// `x.y.z` 三段数字（对应 schema 的 `^\d+\.\d+\.\d+$`）。
fn is_semver_triplet(version: &str) -> bool {
    let mut parts = version.split('.');
    let all_numeric = |n: &str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), Some(c), None) => all_numeric(a) && all_numeric(b) && all_numeric(c),
        _ => false,
    }
}

/// 待实测项 ID：`T` + 数字 + （可选）`a`/`b`/`c`（对应 schema `^T[0-9]+[a-c]?$`）。
fn is_verification_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix('T') else {
        return false;
    };
    let (digits, suffix) = match rest.as_bytes().last() {
        Some(b'a' | b'b' | b'c') => (&rest[..rest.len() - 1], &rest[rest.len() - 1..]),
        _ => (rest, ""),
    };
    if !digits.is_empty() && !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if digits.is_empty() {
        return false;
    }
    suffix.is_empty() || matches!(suffix, "a" | "b" | "c")
}

// ── 反序列化中间结构（与 manifests.schema.json 逐字对应）────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    id: String,
    game: String,
    name: String,
    description: String,
    version: String,
    #[serde(rename = "type")]
    tool_type: String,
    risk_level: String,
    permissions: Vec<String>,
    requires_admin: bool,
    backup_required: bool,
    entry: RawEntry,
    source: RawSource,
    #[serde(default)]
    pending_verifications: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    executor: String,
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    channel: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    kind: String,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // 与 core 同一纪律：不把真实游戏 slug 写进测试数据（虽不受架构断言约束，
    // 但保持一致的意图 —— 单测验证规则，数据正确性由 validate:data 守护）。

    fn valid_config_modify() -> String {
        r#"{
          "id": "sample-ns/sample-tool",
          "game": "sample-game",
          "name": "示例工具",
          "description": "面向用户的一句话说明。",
          "version": "0.1.0",
          "type": "config_modify",
          "risk_level": "L1",
          "permissions": ["read_config", "write_config"],
          "requires_admin": false,
          "backup_required": true,
          "entry": { "executor": "sample_executor" },
          "source": { "kind": "builtin", "repo": null, "license": null, "version": null },
          "pending_verifications": []
        }"#
        .to_owned()
    }

    fn valid_external_process() -> String {
        r#"{
          "id": "sample-ns/sample-unlocker",
          "game": "sample-game",
          "name": "示例解锁",
          "description": "不落盘的运行时工具。",
          "version": "0.1.0",
          "type": "external_process",
          "risk_level": "L3",
          "permissions": ["launch_external", "process_attach"],
          "requires_admin": false,
          "backup_required": false,
          "entry": { "executor": "unlocker", "asset": "sample-asset", "channel": "releases" },
          "source": { "kind": "upstream", "repo": "owner/repo", "license": "MIT", "version": "v1.0.0" },
          "pending_verifications": ["T4a", "T4b"]
        }"#
        .to_owned()
    }

    #[test]
    fn parses_a_config_modify_manifest() {
        let m = ToolManifest::parse(&valid_config_modify()).expect("示例应可解析");
        assert_eq!(m.id, "sample-ns/sample-tool");
        assert_eq!(m.tool_type, ToolType::ConfigModify);
        assert_eq!(m.risk_level, RiskLevel::L1);
        assert!(m.backup_required);
        assert!(!m.requires_asset());
        assert!(!m.has_pending_verifications());
        assert_eq!(
            m.permissions,
            vec![ToolPermission::ReadConfig, ToolPermission::WriteConfig]
        );
    }

    #[test]
    fn parses_an_external_process_manifest_with_attribution() {
        let m = ToolManifest::parse(&valid_external_process()).expect("示例应可解析");
        assert_eq!(m.tool_type, ToolType::ExternalProcess);
        assert_eq!(m.risk_level, RiskLevel::L3);
        assert!(!m.backup_required, "L3 不落盘，备份不适用");
        assert!(m.requires_asset());
        assert_eq!(m.entry.asset.as_deref(), Some("sample-asset"));
        assert!(m.source.is_attribution_complete());
        assert!(m.has_pending_verifications());
    }

    #[test]
    fn rejects_config_modify_without_mandatory_backup() {
        // 00 §12.3 规则 5：落盘修改必须强制备份。放过这条 = 允许「无备份修改」
        let json = valid_config_modify()
            .replace("\"backup_required\": true", "\"backup_required\": false");
        match ToolManifest::parse(&json) {
            Err(ManifestError::InvalidField { field, .. }) => assert_eq!(field, "backup_required"),
            other => panic!("应拒绝无备份的 config_modify，实际：{other:?}"),
        }
    }

    #[test]
    fn rejects_upstream_source_without_attribution() {
        let json = valid_external_process().replace("\"license\": \"MIT\"", "\"license\": null");
        match ToolManifest::parse(&json) {
            Err(ManifestError::InvalidField { field, .. }) => assert_eq!(field, "source"),
            other => panic!("应拒绝缺失 License 的上游来源，实际：{other:?}"),
        }
    }

    #[test]
    fn rejects_unlocker_without_asset() {
        // 发行包分离（B7）要求 unlocker 必须指向一个资产 key
        let json = valid_external_process().replace(
            ", \"asset\": \"sample-asset\", \"channel\": \"releases\"",
            "",
        );
        match ToolManifest::parse(&json) {
            Err(ManifestError::InvalidField { field, .. }) => assert_eq!(field, "entry"),
            other => panic!("应拒绝缺少 asset 的 unlocker，实际：{other:?}"),
        }
    }

    #[test]
    fn rejects_bad_identifiers_and_strictness_violations() {
        // id 形态
        let bad_id = valid_config_modify().replace("sample-ns/sample-tool", "no-namespace");
        assert!(matches!(
            ToolManifest::parse(&bad_id),
            Err(ManifestError::InvalidField { field: "id", .. })
        ));

        // game 形态
        let bad_game =
            valid_config_modify().replace("\"game\": \"sample-game\"", "\"game\": \"Sample Game\"");
        assert!(matches!(
            ToolManifest::parse(&bad_game),
            Err(ManifestError::InvalidField { field: "game", .. })
        ));

        // type / risk_level / permissions 严格解析
        for (json, field) in [
            (
                valid_config_modify().replace("\"config_modify\"", "\"config\""),
                "type",
            ),
            (
                valid_config_modify().replace("\"L1\"", "\"l1\""),
                "risk_level",
            ),
            (
                valid_config_modify().replace("\"read_config\"", "\"read\""),
                "permissions",
            ),
        ] {
            match ToolManifest::parse(&json) {
                Err(ManifestError::InvalidField { field: f, .. }) => assert_eq!(f, field),
                other => panic!("字段 {field} 应被拒绝，实际：{other:?}"),
            }
        }

        // version 必须三段
        let bad_version =
            valid_config_modify().replace("\"version\": \"0.1.0\"", "\"version\": \"0.1\"");
        assert!(matches!(
            ToolManifest::parse(&bad_version),
            Err(ManifestError::InvalidField {
                field: "version",
                ..
            })
        ));

        // pending_verifications 形态
        let bad_pending = valid_external_process().replace("\"T4a\"", "\"T4z\"");
        assert!(matches!(
            ToolManifest::parse(&bad_pending),
            Err(ManifestError::InvalidField {
                field: "pending_verifications",
                ..
            })
        ));

        // additionalProperties: false
        let extra = valid_config_modify().replace("\"name\"", "\"unexpected\": 1, \"name\"");
        assert!(matches!(
            ToolManifest::parse(&extra),
            Err(ManifestError::Malformed { .. })
        ));
    }

    #[test]
    fn one_broken_manifest_does_not_hide_the_others() {
        // 02 C1 验收：单个损坏只让该工具不可见
        let good_a = valid_config_modify();
        let good_b = valid_external_process();
        let broken = "{ this is not json";

        let set = ManifestSet::from_sources(&[
            ("a.json", good_a.as_str()),
            ("broken.json", broken),
            ("b.json", good_b.as_str()),
        ]);

        assert_eq!(set.len(), 2, "其余工具必须照常可用");
        assert_eq!(set.rejected().len(), 1);
        assert_eq!(set.rejected()[0].source, "broken.json");
        assert!(set.is_degraded());
        assert!(set.get("sample-ns/sample-tool").is_some());
        assert!(set.get("sample-ns/sample-unlocker").is_some());
    }

    #[test]
    fn duplicate_tool_id_is_rejected_as_conflict() {
        let json = valid_config_modify();
        let set =
            ManifestSet::from_sources(&[("a.json", json.as_str()), ("b.json", json.as_str())]);
        assert_eq!(set.len(), 1, "重复 id 只保留第一条");
        assert!(matches!(
            set.rejected()[0].error,
            ManifestError::Conflict { .. }
        ));
    }

    #[test]
    fn empty_source_list_yields_an_empty_set_without_degradation() {
        let set = ManifestSet::from_sources(&[]);
        assert!(set.is_empty());
        assert!(!set.is_degraded(), "没有数据 ≠ 降级");
    }

    #[test]
    fn builtin_manifests_are_loadable() {
        // 断言内置数据能被加载器接受；内容正确性由 npm run validate:data 守护
        let set = ManifestSet::builtin();
        assert!(!set.is_empty(), "内置 Manifest 不应为空");
        assert!(
            !set.is_degraded(),
            "内置 Manifest 不应有降级项：{:?}",
            set.rejected()
        );
        // 数据不变量：每条的 game_id 都是合法 slug（形态层，不涉及具体游戏）
        for m in set.iter() {
            assert!(
                GameId::new(&m.game_id).is_some(),
                "game_id 非法：{}",
                m.game_id
            );
        }
    }
}
