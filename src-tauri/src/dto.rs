//! 壳层投影的契约 DTO（`docs/ipc-contract.md` §6）。
//!
//! 大部分 DTO 的组装在领域 crate 里（见 `orbis_tools::contract`）；这里只放**必须由
//! 壳层拼装**的那几个 —— 理由是它们需要同时看见别的领域 crate：
//!
//! - 游戏目录 / 安装实例 / 摘要：需要 `providers`（目录 + 能力声明）与
//!   `tools`（工具兼容状态），而这两个 crate 之间禁止横向依赖（04 §9.1 不变量 [3]）
//! - 设置：`platform` 的类型直接就是契约形状
//!
//! 壳层是唯一组装根，因此这类跨域投影只能落在这里。
//!
//! 这里做的事是**投影**（把已有事实摆成契约形状）与**凑齐判定的输入**，
//! 不是**决策** —— 后者的例子是「需处理」判定式（在 Core，04 §6.4.3）与
//! 「该不该允许启用这个工具」（在 Core，04 §5.5 门控）。

use orbis_core::{attention_reasons, AttentionInput};
use orbis_platform::db::AppSettings;
use orbis_platform::installation::{InstallationRecord, LaunchProfileRecord};
use orbis_providers::capabilities::{capabilities, ConfigCapability};
use orbis_tools::{game_tool_compat, BuiltinData};
use serde::Serialize;

/// 契约 §6 `GameCatalogEntry`（`listGames` 的元素）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCatalogEntryDto {
    pub id: String,
    /// 中文名
    pub name: String,
    pub en_name: String,
    pub publisher: String,
    /// 小写 slug（`cn` / `global` / `bili`）；「国服」这类展示文案归 UI
    pub regions: Vec<&'static str>,
    /// 官方入口（A1 空状态引导）；`None` → UI 不显示该入口
    pub official_url: Option<String>,
    /// 是否存在依赖独立下载资产的组件（B7 发行包分离，空状态需提前告知）
    pub has_bundled_component: bool,
}

/// 游戏目录 → DTO。
///
/// `has_bundled_component` **由 Manifest 派生**而不是在目录里写死一个副本：
/// 写死会在「新增/删除一个 external_process 工具」时与 Manifest 静默漂移。
pub fn game_catalog_entries(data: &BuiltinData) -> Vec<GameCatalogEntryDto> {
    orbis_providers::catalog::GAME_CATALOG
        .iter()
        .map(|entry| GameCatalogEntryDto {
            id: entry.id.to_owned(),
            name: entry.name.to_owned(),
            en_name: entry.en_name.to_owned(),
            publisher: entry.publisher.to_owned(),
            regions: entry.regions.iter().map(|region| region.slug()).collect(),
            official_url: entry.official_url.map(str::to_owned),
            has_bundled_component: data
                .manifests
                .for_game(entry.id)
                .any(|manifest| manifest.requires_asset()),
        })
        .collect()
}

/// 契约 §6 `AppSettings`。
///
/// 字段名带点（`version_check.enabled`）—— 这是契约明文规定的键名，会被 UI 直接
/// 当作设置项 key 使用，因此**不能**套用 camelCase 转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AppSettingsDto {
    #[serde(rename = "version_check.enabled")]
    pub version_check_enabled: bool,
    #[serde(rename = "log.retention_days")]
    pub log_retention_days: u32,
    #[serde(rename = "playtime.checkpoint_sec")]
    pub playtime_checkpoint_sec: u32,
}

impl From<AppSettings> for AppSettingsDto {
    fn from(settings: AppSettings) -> Self {
        Self {
            version_check_enabled: settings.version_check_enabled,
            log_retention_days: settings.log_retention_days,
            playtime_checkpoint_sec: settings.playtime_checkpoint_sec,
        }
    }
}

// ── 安装实例（契约 §3.1 / §6）────────────────────────────────

/// 契约 §6 `InstallationDto`。
///
/// # 三个「当前只能是某个固定值」的字段
///
/// 它们不是占位符，而是**该能力未落地时的唯一诚实取值**，落地后自然替换：
///
/// | 字段 | 当前值 | 依据 |
/// |------|--------|------|
/// | `updateAvailable` | 恒 `false` | 远程版本（E1）未落地 → 02 E1「不误报」 |
/// | `pid` | 恒 `null` | 进程快照（A5）未落地 → 只有 `running` 时才该非空 |
/// | `playtime` | 恒 0 | 会话记录（A6）未落地 → 「没有记录」与「0 秒」在 UI 上一致 |
///
/// 时间一律 **Unix epoch 毫秒**（契约 §1：IPC 层为毫秒；DB 的 `*_at` 是直接喂给 IPC 的
/// INTEGER，因此同样存毫秒）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationDto {
    pub id: String,
    pub game_id: String,
    pub region: &'static str,
    pub install_path: String,
    pub executable_path: String,
    pub local_version: Option<String>,
    pub version_norm: Option<String>,
    pub version_source: Option<&'static str>,
    /// 运行态。A5 未落地 → 只可能是落库的四态，不会是 `running`
    pub status: &'static str,
    pub update_available: bool,
    pub version_unknown: bool,
    pub needs_attention: bool,
    pub attention_reasons: Vec<&'static str>,
    pub added_via: &'static str,
    pub pid: Option<u32>,
    pub playtime: PlaytimeDto,
    pub has_config_source: bool,
    /// `provider_not_declared`（暂不支持配置备份）/ `not_applicable`（不适用）/ `null`
    pub config_unsupported_reason: Option<&'static str>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 契约 §6 `InstallationDto.playtime`（单位：秒）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeDto {
    pub today_sec: i64,
    pub week_sec: i64,
    pub total_sec: i64,
}

/// 契约 §6 `AttentionSummary`（首页摘要条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionSummaryDto {
    pub total: usize,
    pub needs_attention: usize,
    pub update_available: usize,
    pub broken: usize,
    /// 「工具维」需处理的数量：任一工具 `unknown` 或硬阻止
    pub tool_attention: usize,
}

/// 契约 §6 `InstallationListResult`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationListResultDto {
    pub installations: Vec<InstallationDto>,
    pub summary: AttentionSummaryDto,
}

/// 契约 §6 `LaunchProfileDto`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfileDto {
    pub installation_id: String,
    pub args: String,
    pub updated_at: i64,
}

/// 一条安装实例 → DTO。
///
/// 「需处理」判定在 Core 算（04 §6.4.3：`attention_reasons`），本函数只负责凑齐输入：
/// 状态来自落库记录、`updateAvailable` 来自 E1（未落地 → false）、
/// 工具兼容状态来自 [`orbis_tools::game_tool_compat`]。
fn installation_dto(data: &BuiltinData, record: &InstallationRecord) -> InstallationDto {
    // 未登记的游戏 → 视为「未声明配置源」：目录与能力表必须成对维护
    // （`capabilities` 的单测守着），这里只是漂移时的降级，不是猜测。
    let config = capabilities(&record.game_id)
        .map(|c| c.config)
        .unwrap_or(ConfigCapability::ProviderNotDeclared);

    let runtime_status = record.status.to_runtime();
    // E1 未落地 → 不误报（02 E1 验收）
    let update_available = false;
    let tool_compat = game_tool_compat(data, &record.game_id, record.version_norm);
    let input = AttentionInput::new(
        runtime_status,
        update_available,
        record.version_norm,
        tool_compat,
    );
    let reasons = attention_reasons(&input);

    InstallationDto {
        id: record.id.clone(),
        game_id: record.game_id.clone(),
        region: record.region.slug(),
        install_path: record.install_path.clone(),
        executable_path: record.executable_path.clone(),
        local_version: record.local_version.clone(),
        version_norm: record.version_norm.map(|v| v.to_string()),
        version_source: record.version_source.map(|s| s.slug()),
        status: runtime_status.slug(),
        update_available,
        version_unknown: record.version_unknown(),
        needs_attention: !reasons.is_empty(),
        attention_reasons: reasons.iter().map(|r| r.slug()).collect(),
        added_via: record.added_via.slug(),
        pid: None,
        playtime: PlaytimeDto {
            today_sec: 0,
            week_sec: 0,
            total_sec: 0,
        },
        has_config_source: config.has_config_source(),
        config_unsupported_reason: config.unsupported_reason(),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

/// 实例列表 + 摘要。摘要口径与 `src/api/mock.ts` 逐字一致（契约 §6 只列字段名）。
pub fn installation_list(
    data: &BuiltinData,
    records: Vec<InstallationRecord>,
) -> InstallationListResultDto {
    let installations: Vec<InstallationDto> = records
        .iter()
        .map(|record| installation_dto(data, record))
        .collect();

    let summary = AttentionSummaryDto {
        total: installations.len(),
        needs_attention: installations.iter().filter(|i| i.needs_attention).count(),
        update_available: installations.iter().filter(|i| i.update_available).count(),
        broken: installations
            .iter()
            .filter(|i| i.status == "broken")
            .count(),
        tool_attention: installations
            .iter()
            .filter(|i| {
                i.attention_reasons
                    .iter()
                    .any(|r| *r == "tool_unknown" || *r == "tool_incompatible")
            })
            .count(),
    };

    InstallationListResultDto {
        installations,
        summary,
    }
}

/// 启动参数 → DTO。
///
/// 没有自定义参数时 `args = ""`，`updatedAt` 取**实例的创建时间**：
/// 契约把 `updatedAt` 定为非空 `number`，而「自实例建立以来一直是默认参数」正是事实。
/// 编造一个 `0` 或当前时间都会让 UI 显示错误的时间点。
pub fn launch_profile_dto(
    record: &InstallationRecord,
    profile: Option<LaunchProfileRecord>,
) -> LaunchProfileDto {
    match profile {
        Some(profile) => LaunchProfileDto {
            installation_id: profile.installation_id,
            args: profile.args,
            updated_at: profile.updated_at,
        },
        None => LaunchProfileDto {
            installation_id: record.id.clone(),
            args: String::new(),
            updated_at: record.created_at,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbis_core::SeedTable;
    use orbis_tools::{AssetCatalog, ManifestSet};

    const CONFIG_MODIFY: &str = r#"{
      "id": "sample-ns/sample-tool",
      "game": "sample-game",
      "name": "示例",
      "description": "示例说明。",
      "version": "0.1.0",
      "type": "config_modify",
      "risk_level": "L1",
      "permissions": ["write_config"],
      "requires_admin": false,
      "backup_required": true,
      "entry": { "executor": "sample_executor" },
      "source": { "kind": "builtin" },
      "pending_verifications": []
    }"#;

    fn data_with(manifests: ManifestSet) -> BuiltinData {
        BuiltinData {
            seed: SeedTable::empty(),
            manifests,
            assets: AssetCatalog::empty(),
            issues: Vec::new(),
            degradations: Vec::new(),
        }
    }

    #[test]
    fn catalog_entries_carry_every_static_field() {
        let entries = game_catalog_entries(&data_with(ManifestSet::from_sources(&[])));
        assert_eq!(
            entries.len(),
            orbis_providers::catalog::GAME_CATALOG.len(),
            "目录条目不得被投影丢弃"
        );

        // 逐条对照：投影不得漏字段、不得改名
        for (entry, dto) in orbis_providers::catalog::GAME_CATALOG.iter().zip(&entries) {
            assert_eq!(dto.id, entry.id);
            assert_eq!(dto.name, entry.name);
            assert_eq!(dto.en_name, entry.en_name);
            assert_eq!(dto.publisher, entry.publisher);
            assert_eq!(
                dto.regions,
                entry.regions.iter().map(|r| r.slug()).collect::<Vec<_>>()
            );
            assert!(!dto.regions.is_empty());
        }
    }

    #[test]
    fn bundled_component_is_derived_from_manifests_not_hardcoded() {
        // 合成数据：给目录里**第一个**游戏挂一个依赖资产的工具
        let target = orbis_providers::catalog::GAME_CATALOG[0].id;
        let external = CONFIG_MODIFY
            .replace("sample-game", target)
            .replace("config_modify", "external_process")
            .replace("\"write_config\"", "\"launch_external\"")
            .replace(
                "\"entry\": { \"executor\": \"sample_executor\" }",
                "\"entry\": { \"executor\": \"unlocker\", \"asset\": \"sample-asset\", \"channel\": \"releases\" }",
            )
            .replace("\"backup_required\": true", "\"backup_required\": false")
            .replace("\"risk_level\": \"L1\"", "\"risk_level\": \"L3\"");

        let without = game_catalog_entries(&data_with(ManifestSet::from_sources(&[])));
        assert!(
            without.iter().all(|entry| !entry.has_bundled_component),
            "没有任何工具时不得报告随包组件"
        );

        let with = game_catalog_entries(&data_with(ManifestSet::from_sources(&[(
            "external.json",
            external.as_str(),
        )])));
        let hit = with
            .iter()
            .find(|entry| entry.id == target)
            .expect("目标游戏应在目录中");
        assert!(hit.has_bundled_component, "声明了资产的工具应点亮该标记");
        // 其余游戏不受影响 —— 否则空状态会到处提示「需额外组件」
        assert_eq!(with.iter().filter(|e| e.has_bundled_component).count(), 1);
    }

    #[test]
    fn settings_dto_serializes_with_dotted_keys() {
        let dto = AppSettingsDto::from(AppSettings {
            version_check_enabled: false,
            log_retention_days: 7,
            playtime_checkpoint_sec: 45,
        });
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["version_check.enabled"], false);
        assert_eq!(json["log.retention_days"], 7);
        assert_eq!(json["playtime.checkpoint_sec"], 45);
        assert_eq!(json.as_object().unwrap().len(), 3, "不得出现多余字段");
    }

    // ── 安装实例 ─────────────────────────────────────────

    use orbis_core::{Region, Version};
    use orbis_platform::installation::{AddedVia, PersistedStatus, VersionSource};

    /// 构造一条实例记录。时间用 epoch **毫秒**（契约 §1）。
    fn record(id: &str, game_id: &str, version_norm: Option<&str>) -> InstallationRecord {
        InstallationRecord {
            id: id.to_owned(),
            game_id: game_id.to_owned(),
            region: Region::Cn,
            install_path: "C:/sample".to_owned(),
            executable_path: "C:/sample/game.exe".to_owned(),
            local_version: version_norm.map(|v| format!("{v}.0.128940")),
            version_norm: version_norm.and_then(Version::parse),
            version_source: version_norm.map(|_| VersionSource::ExeVersionInfo),
            status: PersistedStatus::Installed,
            added_via: AddedVia::Manual,
            created_at: 1_758_000_000_000,
            updated_at: 1_758_000_000_000,
        }
    }

    #[test]
    fn installation_dto_projects_the_record_and_derives_config_capability() {
        let data = data_with(ManifestSet::from_sources(&[]));

        // 原神：策略上「不适用」配置备份（04 §4.2 明文标注）
        let genshin = installation_dto(&data, &record("i1", "genshin-impact", Some("7.0")));
        assert_eq!(genshin.game_id, "genshin-impact");
        assert_eq!(genshin.status, "installed");
        assert_eq!(genshin.region, "cn");
        assert_eq!(genshin.added_via, "manual");
        assert_eq!(genshin.version_norm.as_deref(), Some("7.0"));
        assert_eq!(genshin.version_source, Some("exe_versioninfo"));
        assert!(!genshin.version_unknown);
        assert!(!genshin.has_config_source);
        assert_eq!(genshin.config_unsupported_reason, Some("not_applicable"));

        // 鸣潮：MVP 里唯一声明了配置源的游戏
        let wuwa = installation_dto(&data, &record("i2", "wuthering-waves", Some("3.5")));
        assert!(wuwa.has_config_source);
        assert_eq!(wuwa.config_unsupported_reason, None);
    }

    #[test]
    fn unlanded_capabilities_are_fixed_values_not_placeholders() {
        // 这三个字段在对应能力落地前只能是唯一的诚实取值（见 InstallationDto 文档）
        let dto = installation_dto(
            &data_with(ManifestSet::from_sources(&[])),
            &record("i1", "genshin-impact", Some("7.0")),
        );
        assert!(!dto.update_available, "E1 未落地 → 不误报（02 E1）");
        assert_eq!(dto.pid, None, "A5 未落地 → 不可能处于运行中");
        assert_eq!(
            dto.playtime,
            PlaytimeDto {
                today_sec: 0,
                week_sec: 0,
                total_sec: 0
            },
            "A6 未落地 → 没有任何会话记录"
        );
    }

    #[test]
    fn unknown_version_and_tool_compat_drive_attention() {
        // 空种子表 → 任何工具查询都落到 unknown；版本未知 → version_unknown。
        // 两条都必须出现在 attentionReasons 里（否则首页徽标与卡片不一致）
        let tool = CONFIG_MODIFY.replace("sample-game", "wuthering-waves");
        let data = data_with(ManifestSet::from_sources(&[("wuwa.json", tool.as_str())]));

        let dto = installation_dto(&data, &record("i1", "wuthering-waves", None));
        assert!(dto.version_unknown);
        assert!(dto.needs_attention);
        assert!(dto.attention_reasons.contains(&"version_unknown"));
        assert!(
            dto.attention_reasons.contains(&"tool_unknown"),
            "工具未定稿必须对用户可见（00 §8.8）：{:?}",
            dto.attention_reasons
        );
    }

    #[test]
    fn summary_counts_match_the_reference_implementation() {
        let data = data_with(ManifestSet::from_sources(&[]));

        let mut broken = record("i2", "genshin-impact", Some("7.0"));
        broken.status = PersistedStatus::Broken;

        let result = installation_list(
            &data,
            vec![
                record("i1", "wuthering-waves", Some("3.5")),
                broken,
                record("i3", "honkai-star-rail", None),
            ],
        );

        assert_eq!(result.summary.total, 3);
        assert_eq!(result.summary.broken, 1);
        assert_eq!(result.summary.update_available, 0);
        assert_eq!(
            result.summary.tool_attention, 0,
            "没有任何工具的游戏不得计入工具维"
        );
        // broken（i2）与 version_unknown（i3）各一条
        assert_eq!(result.summary.needs_attention, 2);
    }

    #[test]
    fn tool_attention_counts_into_the_summary() {
        let tool = CONFIG_MODIFY.replace("sample-game", "wuthering-waves");
        let data = data_with(ManifestSet::from_sources(&[("wuwa.json", tool.as_str())]));
        let result = installation_list(&data, vec![record("i1", "wuthering-waves", Some("3.5"))]);

        assert_eq!(
            result.summary.tool_attention, 1,
            "工具 unknown 是「需处理」的工具维来源"
        );
        assert_eq!(result.summary.needs_attention, 1);
    }

    #[test]
    fn launch_profile_falls_back_to_the_creation_time() {
        let installation = record("i1", "wuthering-waves", Some("3.5"));

        let default = launch_profile_dto(&installation, None);
        assert_eq!(default.installation_id, "i1");
        assert_eq!(default.args, "");
        assert_eq!(
            default.updated_at, installation.created_at,
            "没有自定义参数 → 自实例建立以来未改过，不得编造时间点"
        );

        let custom = launch_profile_dto(
            &installation,
            Some(LaunchProfileRecord {
                installation_id: "i1".to_owned(),
                args: "-windowed".to_owned(),
                updated_at: 42,
            }),
        );
        assert_eq!(custom.args, "-windowed");
        assert_eq!(custom.updated_at, 42);
    }

    #[test]
    fn installation_dto_serializes_with_contract_field_names() {
        let dto = installation_dto(
            &data_with(ManifestSet::from_sources(&[])),
            &record("i1", "genshin-impact", Some("7.0")),
        );
        let json = serde_json::to_value(&dto).unwrap();

        for key in [
            "id",
            "gameId",
            "region",
            "installPath",
            "executablePath",
            "localVersion",
            "versionNorm",
            "versionSource",
            "status",
            "updateAvailable",
            "versionUnknown",
            "needsAttention",
            "attentionReasons",
            "addedVia",
            "pid",
            "playtime",
            "hasConfigSource",
            "configUnsupportedReason",
            "createdAt",
            "updatedAt",
        ] {
            assert!(json.get(key).is_some(), "缺少契约字段 {key}");
        }
        assert!(json["playtime"].get("todaySec").is_some());
        assert_eq!(
            json.as_object().unwrap().len(),
            20,
            "字段数必须与契约 §6 的 InstallationDto 一致"
        );
    }
}
