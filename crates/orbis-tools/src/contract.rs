//! 契约 DTO 与组装（`docs/ipc-contract.md` §5 / §6 的 Rust 侧对应物）。
//!
//! # 为什么在 `orbis-tools` 而不是壳层
//!
//! `ToolDto` 是**四份数据的拼装结果**：Manifest（tools）+ 兼容性种子表（core）+
//! 工具启停状态（platform DB）+ 资产清单（tools）。壳层被明确要求「不承载业务逻辑」
//! （04 §4.1 / 00 §7.9），而 `orbis-core` 又不得知道 Manifest 的聚合结构 ——
//! 因此拼装点只能落在同时看得见三者的 `orbis-tools` 上。
//!
//! 本模块**不依赖 Tauri**：它只把领域数据投影成契约形状，壳层负责命令注册与错误码
//! 映射。这样 DTO 能被普通单测覆盖，不需要拉起一个 WebView。
//!
//! # 序列化口径（契约 §1）
//!
//! - 字段 **camelCase**（`#[serde(rename_all = "camelCase")]`）
//! - 枚举值 **snake_case 小写字面量** —— 直接复用各枚举的 `slug()` / `literal()`，
//!   不另造映射表：两份映射表必然会漂移
//! - 未知一律 `null`（`Option::None`），不用空字符串、不用 `"Unknown"` 字面量
//!
//! # 当前实现边界（不要在这里补不存在的能力）
//!
//! - **安装实例（A3）尚未落地** → 兼容查询的 `local` 版本必为 `None`，按契约 §3.7
//!   落到 `unknown` + `matchKind: 'none'`。这不是占位值，是规定行为。
//! - **资产下载器（B7）尚未落地** → 只会出现 `state: "missing"`。`ready` /
//!   `hash_mismatch` / `expected_sha256` / `installed_path` 随下载器一起到位；
//!   在那之前在这里「顺手填一个预期哈希」等于凭空造证据。
//! - **工具启停状态由调用方注入**（`is_enabled`）而非本模块直接读库：DB 不可用时
//!   的策略（降级为全部未启用 / 直接报错）属于壳层决定，因此这里只接受一个闭包。

use orbis_core::{query, CompatStatus, MatchKind, SeedTable, Version};
use serde::Serialize;

use crate::{AssetCatalog, BuiltinData, ToolManifest};

// ── DTO（契约 §6）──────────────────────────────────────────

/// 契约 §6 `CompatibilityDto`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityDto {
    pub game_id: String,
    pub tool_id: String,
    /// 五态之一：`verified` / `compatible` / `unknown` / `incompatible` / `deprecated`
    pub status: &'static str,
    /// `exact` / `prefix` / `none`；`none` 时 `status` 必为 `unknown`
    pub match_kind: &'static str,
    /// 如 `"3.x"`（prefix 命中时）
    pub matched_version_key: Option<String>,
    /// `YYYY-MM-DD`（seed 原文，非 epoch）
    pub verified_at: Option<String>,
    pub verified_by: Option<String>,
    pub notes: Option<String>,
    /// `null` = seed 缺失 / 损坏（02 C4 降级态）
    pub seed_schema_version: Option<String>,
}

/// 契约 §6 `ToolSource`（署名义务的载体，00 §8.6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSourceDto {
    /// `builtin` / `bundled` / `upstream`
    pub kind: &'static str,
    pub repo: Option<String>,
    pub license: Option<String>,
    pub version: Option<String>,
}

/// 契约 §6 `ToolAssetDto`。
///
/// `download_url_configured = false` → UI 显示「组件构建管道待定稿」，
/// **不作为错误呈现**（契约 §5 `TOOL_ASSET_NOT_CONFIGURED`，04 §11 Q1）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAssetDto {
    pub tool_id: String,
    pub asset_key: String,
    /// `not_required` / `missing` / `ready` / `hash_mismatch`
    pub state: &'static str,
    pub version: Option<String>,
    /// 已安装资产的实测哈希（无下载器 → 恒 `None`）
    pub sha256: Option<String>,
    /// 来自 `assets.json` 的期望哈希（无发行物 → 恒 `None`）
    pub expected_sha256: Option<String>,
    pub installed_path: Option<String>,
    pub download_url_configured: bool,
}

/// 契约 §6 `ToolDto`（`listTools` 的元素）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDto {
    /// `namespace/name`
    pub id: String,
    pub game_id: String,
    pub name: String,
    /// 面向用户的一句话（来自 Manifest，契约 §7.3 允许的例外之一）
    pub description: String,
    pub version: String,
    /// `config_modify` / `external_process`
    #[serde(rename = "type")]
    pub tool_type: &'static str,
    /// `L0`–`L3`
    pub risk_level: &'static str,
    pub permissions: Vec<&'static str>,
    pub requires_admin: bool,
    pub backup_required: bool,
    pub enabled: bool,
    /// 实时查询 seed，**不是** Manifest 静态值（04 §5.7 注②）
    pub compat: CompatibilityDto,
    /// 仅 `external_process` 类非空
    pub asset: Option<ToolAssetDto>,
    pub source: ToolSourceDto,
    /// 非空 → UI 显示「部分参数待实测」
    pub pending_verifications: Vec<String>,
}

// ── 组装 ──────────────────────────────────────────────────

/// 兼容性查询 → DTO。
///
/// `local` 为**该游戏最新安装实例的归一化版本**；无安装实例 / 版本未知时传 `None`。
/// 按契约 §3.7，此时结果必为 `unknown` + `matchKind: 'none'`（02 A3：不猜版本）。
pub fn compatibility(
    seed: &SeedTable,
    game_id: &str,
    tool_id: &str,
    local: Option<Version>,
) -> CompatibilityDto {
    let hit = query(seed.entry(game_id, tool_id), local);
    CompatibilityDto {
        game_id: game_id.to_owned(),
        tool_id: tool_id.to_owned(),
        status: hit.status.slug(),
        match_kind: match_kind_slug(hit.match_kind),
        matched_version_key: hit.matched_version_key,
        verified_at: hit.verified_at,
        verified_by: hit.verified_by,
        notes: hit.notes,
        seed_schema_version: seed.schema_version().map(str::to_owned),
    }
}

const fn match_kind_slug(kind: MatchKind) -> &'static str {
    match kind {
        MatchKind::Exact => "exact",
        MatchKind::Prefix => "prefix",
        MatchKind::None => "none",
    }
}

/// 某款游戏**全部工具**的兼容状态（04 §6.4.3 判定式的 `tool_compat` 入参）。
///
/// 由本模块提供而不是让壳层自己循环：04 §6.4.3 的判定式要求
/// 「该游戏任一工具 compat ∈ {unknown, incompatible, deprecated}」，
/// 而「哪些工具属于这款游戏」只有 Manifest 知道、「每个工具是什么状态」只有种子表答案 ——
/// 把两件事合起来需要一个同时看得见两者的地方，那就是这里。
/// Core 只接受一个状态列表（它不认识 Manifest，这正是 Core 零游戏知识的代价与价值）。
///
/// 无工具的游戏返回空列表 → 判定式里工具维恒不命中（原神以外的广度层游戏即如此）。
pub fn game_tool_compat(
    data: &BuiltinData,
    game_id: &str,
    local: Option<Version>,
) -> Vec<CompatStatus> {
    data.manifests
        .for_game(game_id)
        .map(|manifest| query(data.seed.entry(game_id, &manifest.id), local).status)
        .collect()
}

/// `listTools(gameId?)` 的组装（契约 §3.6）。
///
/// - `game_id = None` → 返回全部工具；顺序 = Manifest 装载顺序（`include_str!` 清单
///   的字面量顺序），**确定且稳定**，UI 需要别的顺序请自行排序
/// - `is_enabled` 由调用方注入（见模块文档：DB 不可用时的策略归壳层）
/// - 兼容性一律以 `local = None` 查询：安装实例未落地，契约 §3.7 规定此时为 `unknown`
pub fn list_tools(
    data: &BuiltinData,
    game_id: Option<&str>,
    is_enabled: &dyn Fn(&str) -> bool,
) -> Vec<ToolDto> {
    data.manifests
        .iter()
        .filter(|manifest| game_id.map_or(true, |game| manifest.game_id == game))
        .map(|manifest| tool_dto(data, manifest, None, is_enabled(manifest.id.as_str())))
        .collect()
}

/// 单个 Manifest → `ToolDto`。`getTool`（ToolDetail）落地时会复用本函数。
fn tool_dto(
    data: &BuiltinData,
    manifest: &ToolManifest,
    local: Option<Version>,
    enabled: bool,
) -> ToolDto {
    ToolDto {
        id: manifest.id.clone(),
        game_id: manifest.game_id.clone(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        version: manifest.version.clone(),
        tool_type: manifest.tool_type.slug(),
        risk_level: manifest.risk_level.literal(),
        permissions: manifest.permissions.iter().map(|p| p.slug()).collect(),
        requires_admin: manifest.requires_admin,
        backup_required: manifest.backup_required,
        enabled,
        compat: compatibility(&data.seed, &manifest.game_id, &manifest.id, local),
        asset: asset_dto(manifest, &data.assets),
        source: ToolSourceDto {
            kind: manifest.source.kind.slug(),
            repo: manifest.source.repo.clone(),
            license: manifest.source.license.clone(),
            version: manifest.source.version.clone(),
        },
        pending_verifications: manifest.pending_verifications.clone(),
    }
}

/// 资产状态 → DTO。Manifest 未声明资产 → `None`（契约要求 `external_process` 才非空）。
///
/// 资产清单里**找不到条目**时仍然返回 `Some`：工具必须照常可见，只是不可下载
/// （发行包分离的设计红利，02 C7 / B7）。`download_url_configured` 是唯一需要
/// UI 关心的信号。
fn asset_dto(manifest: &ToolManifest, assets: &AssetCatalog) -> Option<ToolAssetDto> {
    let asset_key = manifest.entry.asset.as_deref()?;
    Some(ToolAssetDto {
        tool_id: manifest.id.clone(),
        asset_key: asset_key.to_owned(),
        // 下载器（B7）未落地 → 不可能处于 ready / hash_mismatch。
        // `not_required` 属于「工具不需要资产」的情形，此时本函数已在上面返回 None。
        state: "missing",
        version: None,
        sha256: None,
        expected_sha256: None,
        installed_path: None,
        download_url_configured: assets.download_url_configured(asset_key),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestSet;

    // 与 core / manifest 同一纪律：单测只验证**规则与投影**，不验证某款游戏的数据。
    // 真实数据的正确性由 `npm run validate:data` 与壳层的端到端路径守护。

    const SEED: &str = r#"{
      "schema_version": "0.1.0",
      "updated_at": "2026-09-18",
      "entries": [
        {
          "game_id": "sample-game",
          "tool_id": "sample-ns/sample-tool",
          "risk_level": "L1",
          "compatibility": [
            { "game_version": "2.7", "status": "Verified", "verified_at": "2026-09-18", "verified_by": "P0-X" },
            { "game_version": "3.x", "version_match": "prefix", "status": "Unknown" }
          ]
        }
      ]
    }"#;

    const CONFIG_MODIFY: &str = r#"{
      "id": "sample-ns/sample-tool",
      "game": "sample-game",
      "name": "示例修改",
      "description": "示例说明。",
      "version": "0.1.0",
      "type": "config_modify",
      "risk_level": "L1",
      "permissions": ["read_config", "write_config"],
      "requires_admin": false,
      "backup_required": true,
      "entry": { "executor": "sample_executor" },
      "source": { "kind": "builtin" },
      "pending_verifications": ["T4a"]
    }"#;

    const EXTERNAL_PROCESS: &str = r#"{
      "id": "sample-ns/sample-unlocker",
      "game": "other-game",
      "name": "示例解锁",
      "description": "运行时工具。",
      "version": "0.1.0",
      "type": "external_process",
      "risk_level": "L3",
      "permissions": ["launch_external", "process_attach"],
      "requires_admin": false,
      "backup_required": false,
      "entry": { "executor": "unlocker", "asset": "sample-asset", "channel": "releases" },
      "source": { "kind": "upstream", "repo": "owner/repo", "license": "MIT", "version": "v1.0.0" },
      "pending_verifications": []
    }"#;

    const ASSETS: &str = r#"{
      "schema_version": "0.1.0",
      "updated_at": "2026-09-18",
      "assets": [
        {
          "asset_key": "sample-asset",
          "tool_id": "sample-ns/sample-unlocker",
          "channel": "releases",
          "upstream": { "repo": "owner/repo", "license": "MIT", "version": "v1.0.0" },
          "artifacts": []
        }
      ]
    }"#;

    /// 合成装载结果（`issues` / `degradations` 与投影无关，留空）。
    fn data() -> BuiltinData {
        BuiltinData {
            seed: SeedTable::parse(SEED).expect("合成种子表应可解析"),
            manifests: ManifestSet::from_sources(&[
                ("tool.json", CONFIG_MODIFY),
                ("unlocker.json", EXTERNAL_PROCESS),
            ]),
            assets: AssetCatalog::parse(ASSETS).expect("合成资产清单应可解析"),
            issues: Vec::new(),
            degradations: Vec::new(),
        }
    }

    fn always_enabled(_: &str) -> bool {
        true
    }

    #[test]
    fn maps_the_static_manifest_fields_verbatim() {
        let tools = list_tools(&data(), Some("sample-game"), &always_enabled);
        assert_eq!(tools.len(), 1);
        let tool = &tools[0];

        assert_eq!(tool.id, "sample-ns/sample-tool");
        assert_eq!(tool.game_id, "sample-game");
        assert_eq!(tool.name, "示例修改");
        assert_eq!(tool.tool_type, "config_modify");
        assert_eq!(tool.risk_level, "L1");
        assert_eq!(tool.permissions, vec!["read_config", "write_config"]);
        assert!(tool.backup_required);
        assert_eq!(tool.pending_verifications, vec!["T4a"]);
        assert_eq!(tool.source.kind, "builtin");
        // config_modify 不依赖资产 → 契约要求 asset = null
        assert!(tool.asset.is_none());
    }

    #[test]
    fn game_filter_returns_only_that_game() {
        let data = data();
        assert_eq!(list_tools(&data, None, &always_enabled).len(), 2);
        assert_eq!(
            list_tools(&data, Some("other-game"), &always_enabled).len(),
            1
        );
        assert!(list_tools(&data, Some("absent-game"), &always_enabled).is_empty());
    }

    #[test]
    fn enabled_flag_comes_from_the_caller() {
        let data = data();
        let only_second = |id: &str| id == "sample-ns/sample-unlocker";
        let tools = list_tools(&data, None, &only_second);
        let by_id = |id: &str| tools.iter().find(|t| t.id == id).expect("工具应在结果中");
        assert!(!by_id("sample-ns/sample-tool").enabled);
        assert!(by_id("sample-ns/sample-unlocker").enabled);
    }

    #[test]
    fn compat_uses_the_seed_and_falls_to_unknown_without_an_installation() {
        let data = data();

        // 无安装实例（本切片的规定路径）→ unknown + none
        let no_local = list_tools(&data, Some("sample-game"), &always_enabled)
            .remove(0)
            .compat;
        assert_eq!(no_local.status, "unknown");
        assert_eq!(no_local.match_kind, "none");
        assert!(no_local.matched_version_key.is_none());
        assert!(no_local.verified_at.is_none(), "未命中不得残留验证元信息");
        assert_eq!(no_local.seed_schema_version.as_deref(), Some("0.1.0"));

        // 有版本时管线确实按 exact → prefix → unknown 工作
        let exact = compatibility(
            &data.seed,
            "sample-game",
            "sample-ns/sample-tool",
            Version::parse("2.7"),
        );
        assert_eq!(exact.status, "verified");
        assert_eq!(exact.match_kind, "exact");
        assert_eq!(exact.verified_at.as_deref(), Some("2026-09-18"));

        let prefix = compatibility(
            &data.seed,
            "sample-game",
            "sample-ns/sample-tool",
            Version::parse("3.4"),
        );
        assert_eq!(prefix.status, "unknown");
        assert_eq!(prefix.match_kind, "prefix");
        assert_eq!(prefix.matched_version_key.as_deref(), Some("3.x"));
    }

    #[test]
    fn degraded_seed_reports_unknown_and_a_null_schema_version() {
        let mut data = data();
        data.seed = SeedTable::empty();
        let compat = compatibility(&data.seed, "sample-game", "sample-ns/sample-tool", None);
        assert_eq!(compat.status, "unknown");
        assert_eq!(compat.match_kind, "none");
        assert!(compat.seed_schema_version.is_none());
    }

    #[test]
    fn external_process_carries_an_asset_that_is_not_yet_downloadable() {
        let tools = list_tools(&data(), Some("other-game"), &always_enabled);
        let asset = tools[0]
            .asset
            .as_ref()
            .expect("external_process 必须带资产");
        assert_eq!(asset.asset_key, "sample-asset");
        assert_eq!(asset.state, "missing");
        // artifacts 为空 = 构建管道未定稿（Q1）：工具可见、不可下载、**不是错误**
        assert!(!asset.download_url_configured);
        assert!(asset.expected_sha256.is_none());
        assert!(asset.sha256.is_none());
        assert!(asset.installed_path.is_none());
    }

    #[test]
    fn a_tool_whose_asset_is_absent_from_the_catalog_stays_visible() {
        // 资产清单损坏 / 缺条目 → 工具仍在列表里，只是不可下载（发行包分离的红利）
        let mut data = data();
        data.assets = AssetCatalog::empty();
        let tools = list_tools(&data, Some("other-game"), &always_enabled);
        assert_eq!(tools.len(), 1, "资产缺失不得让工具消失");
        assert!(!tools[0].asset.as_ref().unwrap().download_url_configured);
    }

    #[test]
    fn json_field_names_match_the_contract() {
        // 防回归：字段名一旦漂移，前端会静默拿到 undefined
        let tool = list_tools(&data(), Some("other-game"), &always_enabled).remove(0);
        let json = serde_json::to_value(&tool).expect("DTO 应可序列化");

        assert!(json.get("gameId").is_some());
        assert!(json.get("riskLevel").is_some());
        assert!(json.get("backupRequired").is_some());
        assert!(json.get("pendingVerifications").is_some());
        assert_eq!(json["type"], "external_process", "type 不得被写成 toolType");
        assert_eq!(json["compat"]["matchKind"], "none");
        assert!(json["compat"].get("seedSchemaVersion").is_some());
        assert!(json["asset"].get("assetKey").is_some());
        assert!(json["asset"].get("downloadUrlConfigured").is_some());
        assert!(json["source"].get("license").is_some());
    }
}
