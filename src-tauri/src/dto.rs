//! 壳层投影的契约 DTO（`docs/ipc-contract.md` §6）。
//!
//! 大部分 DTO 的组装在领域 crate 里（见 `orbis_tools::contract`）；这里只放**必须由
//! 壳层拼装**的两个 —— 理由是它们需要同时看见「providers 的游戏目录」与
//! 「tools 的工具数据」，而这两个 crate 之间禁止横向依赖（04 §9.1 不变量 [3]）。
//! 壳层是唯一组装根，因此这类跨域投影只能落在这里。
//!
//! 这里做的事是**投影**（把已有事实摆成契约形状），不是**决策** —— 后者的例子是
//! 「该不该允许启用这个工具」（04 §5.5 门控），那属于 Core，不在这里。

use orbis_platform::db::AppSettings;
use orbis_tools::BuiltinData;
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
}
