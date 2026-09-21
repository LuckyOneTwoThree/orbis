//! Orbis Tools —— 工具运行时。
//!
//! # 职责（`pm/04-技术设计.md` §5.7 / §5.8）
//!
//! - **数据加载**：[`manifest`] 加载 `data/tools/manifests/*.json`，
//!   [`assets`] 加载 `data/tools/assets.json`，均经 `include_str!` 打包进二进制（04 §6.2）
//! - **契约投影**：[`contract`] 把「Manifest + 种子表 + 工具启停 + 资产清单」拼成
//!   `docs/ipc-contract.md` §6 的 DTO（`listTools` / `getCompatibility` 的数据源）
//! - **执行器分派**（待落地）：`config_modify` → `EnhancementExecutor`（04 §5.5）；
//!   `external_process` → `UnlockerRunner`（04 §5.8）
//! - **panic 隔离**（待落地）：执行器内 `catch_unwind`，任何 panic 转入 RollingBack，
//!   **不影响主进程**（00 §12.3 规则 9）。根 `Cargo.toml` 因此刻意**未**启用
//!   `panic = "abort"` —— abort 会让 `catch_unwind` 失效并直接终止进程
//!
//! # 架构不变量
//!
//! 新增工具 = 新 Manifest +（必要时）providers 扩展，**Core 不动**（04 §9.1）。
//! 本 crate 不得依赖 `orbis-providers`。
//!
//! # 降级契约汇总
//!
//! | 数据 | 损坏时的行为 | 依据 |
//! |------|--------------|------|
//! | 单个 Manifest | 该工具不可见，其余照常 | 02 C1 验收 |
//! | 资产清单 | 工具仍可见，仅无法下载资产 | 发行包分离（B7） |
//! | 种子表（`orbis-core`） | 兼容查询全部 `Unknown` | 02 C4 验收 |
//!
//! 三者都**不 panic**，且失败原因一律经 [`BuiltinData::degradations`] 交回调用方
//! 写日志（04 §8：禁止静默失败）。

pub mod assets;
pub mod contract;
pub mod manifest;

use std::fmt;

use orbis_core::SeedTable;

pub use assets::{Artifact, AssetCatalog, AssetEntry, AssetError, Packaging, UpstreamRef};
pub use contract::{
    compatibility, list_tools, CompatibilityDto, ToolAssetDto, ToolDto, ToolSourceDto,
};
pub use manifest::{ManifestError, ManifestSet, RejectedManifest, ToolEntry, ToolManifest};

/// 装载期数据完整性问题的严重度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 真实缺陷：数据自相矛盾，工具会给出错误行为
    Problem,
    /// 合法但需可见：例如资产构建管道未定稿（04 §11 Q1）。
    /// 记录它不是为了报错，而是为了「未定稿」这件事对用户与日志显式可见
    Notice,
}

/// 三份内置数据之间的不一致（`npm run validate:data` 的 Rust 侧补充）。
///
/// 两边都做不是冗余：`validate:data` 只在 CI 跑，而这里随发行包一起走，
/// 能挡住「CI 通过后数据被本地改坏」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataIssue {
    /// Manifest 没有对应的种子表条目 → 该工具查不到兼容状态，会被无条件判为 `Unknown`
    ManifestWithoutSeedEntry { tool_id: String },
    /// 种子表条目没有对应的 Manifest → 该条目永远不会被消费（数据腐烂）
    SeedEntryWithoutManifest { tool_id: String },
    /// 同一个 tool_id 在两份数据里挂了不同的游戏
    GameMismatch {
        tool_id: String,
        manifest_game: String,
        seed_game: String,
    },
    /// `external_process` 声明的 asset 在 assets.json 中不存在 → 启用时必然失败
    MissingAsset { tool_id: String, asset_key: String },
    /// 资产存在但没有可下载发行物（Q1 未定稿的正常态；UI 走降级提示，非错误）
    AssetNotConfigured { tool_id: String, asset_key: String },
}

impl DataIssue {
    pub const fn severity(&self) -> Severity {
        match self {
            // 构建管道未定稿是已拍板的合法状态（04 §11 Q1），不是缺陷
            Self::AssetNotConfigured { .. } => Severity::Notice,
            _ => Severity::Problem,
        }
    }
}

impl fmt::Display for DataIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestWithoutSeedEntry { tool_id } => write!(
                f,
                "工具 {tool_id} 有 Manifest 但种子表无条目 —— 兼容状态会恒为 Unknown"
            ),
            Self::SeedEntryWithoutManifest { tool_id } => write!(
                f,
                "种子表条目 {tool_id} 没有对应 Manifest —— 该数据永远不会被消费"
            ),
            Self::GameMismatch {
                tool_id,
                manifest_game,
                seed_game,
            } => write!(
                f,
                "工具 {tool_id} 的游戏标识不一致：Manifest = {manifest_game}，种子表 = {seed_game}"
            ),
            Self::MissingAsset { tool_id, asset_key } => write!(
                f,
                "工具 {tool_id} 声明了资产 {asset_key}，但 assets.json 中不存在 —— 启用必然失败"
            ),
            Self::AssetNotConfigured { tool_id, asset_key } => write!(
                f,
                "工具 {tool_id} 的资产 {asset_key} 尚无发行物（构建管道未定稿，04 §11 Q1）"
            ),
        }
    }
}

/// 三份内置数据的装载结果（桌面壳启动时一次性构建，之后只读共享）。
#[derive(Debug, Clone)]
pub struct BuiltinData {
    pub seed: SeedTable,
    pub manifests: ManifestSet,
    pub assets: AssetCatalog,
    /// 跨文件一致性问题与须知项
    pub issues: Vec<DataIssue>,
    /// 人类可读的降级原因；空 = 本次装载没有任何降级（D3 日志用）
    pub degradations: Vec<String>,
}

impl BuiltinData {
    /// 装载全部内置数据。**任何单份数据的损坏都不会中断装载**。
    pub fn load() -> Self {
        let (seed, seed_err) = SeedTable::builtin_or_degraded();
        let manifests = ManifestSet::builtin();
        let (assets, asset_err) = AssetCatalog::builtin_or_degraded();

        let mut degradations = Vec::new();
        if let Some(err) = &seed_err {
            degradations.push(format!("种子表降级为空表（兼容查询全部 Unknown）：{err}"));
        }
        for rejected in manifests.rejected() {
            degradations.push(format!(
                "Manifest 被拒（该工具不可见）：{} — {}",
                rejected.source, rejected.error
            ));
        }
        if let Some(err) = &asset_err {
            degradations.push(format!("资产清单降级为空清单（工具仍可见）：{err}"));
        }

        let issues = cross_check(&seed, &manifests, &assets);

        Self {
            seed,
            manifests,
            assets,
            issues,
            degradations,
        }
    }

    /// 是否存在真实缺陷（`Notice` 不算）。
    pub fn has_problems(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity() == Severity::Problem)
    }

    /// 是否存在任何降级（含被拒的 Manifest 与降级的数据源）。
    pub fn is_degraded(&self) -> bool {
        !self.degradations.is_empty() || self.has_problems()
    }

    /// 供启动日志与 UI 的「须知」列表（含 Notice）。
    pub fn notices(&self) -> impl Iterator<Item = &DataIssue> {
        self.issues.iter()
    }
}

/// 跨文件一致性检查（seed ↔ manifest ↔ assets）。
///
/// 检查项与 `npm run validate:data` 的 5 类跨文件一致性对齐，另加
/// 「资产声明存在但无发行物」这一须知项。
pub fn cross_check(
    seed: &SeedTable,
    manifests: &ManifestSet,
    assets: &AssetCatalog,
) -> Vec<DataIssue> {
    let mut issues = Vec::new();

    for manifest in manifests.iter() {
        match seed.entries().iter().find(|e| e.tool_id == manifest.id) {
            None => issues.push(DataIssue::ManifestWithoutSeedEntry {
                tool_id: manifest.id.clone(),
            }),
            Some(entry) if entry.game_id != manifest.game_id => {
                issues.push(DataIssue::GameMismatch {
                    tool_id: manifest.id.clone(),
                    manifest_game: manifest.game_id.clone(),
                    seed_game: entry.game_id.clone(),
                });
            }
            Some(_) => {}
        }

        if let Some(asset_key) = manifest.entry.asset.as_deref() {
            if assets.get(asset_key).is_none() {
                issues.push(DataIssue::MissingAsset {
                    tool_id: manifest.id.clone(),
                    asset_key: asset_key.to_owned(),
                });
            } else if !assets.download_url_configured(asset_key) {
                issues.push(DataIssue::AssetNotConfigured {
                    tool_id: manifest.id.clone(),
                    asset_key: asset_key.to_owned(),
                });
            }
        }
    }

    for entry in seed.entries() {
        if manifests.get(&entry.tool_id).is_none() {
            issues.push(DataIssue::SeedEntryWithoutManifest {
                tool_id: entry.tool_id.clone(),
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造三份**自洽**的合成数据（刻意不含任何真实游戏/工具标识）。
    fn coherent_data() -> (SeedTable, ManifestSet, AssetCatalog) {
        let seed = SeedTable::parse(
            r#"{
              "schema_version": "0.1.0",
              "updated_at": "2026-09-18",
              "entries": [
                {
                  "game_id": "sample-game",
                  "tool_id": "sample-ns/sample-tool",
                  "risk_level": "L1",
                  "compatibility": [
                    { "game_version": "1.0", "status": "Verified" }
                  ]
                }
              ]
            }"#,
        )
        .expect("合成种子表应可解析");

        let manifests = ManifestSet::from_sources(&[(
            "sample.json",
            r#"{
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
            }"#,
        )]);

        let assets = AssetCatalog::parse(
            r#"{ "schema_version": "0.1.0", "updated_at": "2026-09-18", "assets": [] }"#,
        )
        .expect("合成资产清单应可解析");

        (seed, manifests, assets)
    }

    #[test]
    fn coherent_data_reports_no_problems() {
        let (seed, manifests, assets) = coherent_data();
        let issues = cross_check(&seed, &manifests, &assets);
        assert!(issues.is_empty(), "自洽数据不应产生问题：{issues:?}");
    }

    #[test]
    fn manifest_without_seed_entry_is_a_problem() {
        let (seed, _, assets) = coherent_data();
        let manifests = ManifestSet::from_sources(&[(
            "orphan.json",
            r#"{
              "id": "sample-ns/orphan-tool",
              "game": "sample-game",
              "name": "孤儿工具",
              "description": "没有种子表条目。",
              "version": "0.1.0",
              "type": "config_modify",
              "risk_level": "L1",
              "permissions": ["write_config"],
              "requires_admin": false,
              "backup_required": true,
              "entry": { "executor": "sample_executor" },
              "source": { "kind": "builtin" },
              "pending_verifications": []
            }"#,
        )]);

        let issues = cross_check(&seed, &manifests, &assets);
        let hit = issues
            .iter()
            .find(|i| matches!(i, DataIssue::ManifestWithoutSeedEntry { .. }))
            .expect("应报告缺失种子表条目");
        assert_eq!(hit.severity(), Severity::Problem);
    }

    #[test]
    fn game_mismatch_between_the_two_sources_is_detected() {
        let (_, manifests, assets) = coherent_data();
        let seed = SeedTable::parse(
            r#"{
              "schema_version": "0.1.0",
              "updated_at": "2026-09-18",
              "entries": [
                {
                  "game_id": "other-game",
                  "tool_id": "sample-ns/sample-tool",
                  "risk_level": "L1",
                  "compatibility": [{ "game_version": "1.0", "status": "Verified" }]
                }
              ]
            }"#,
        )
        .expect("合成种子表应可解析");

        let issues = cross_check(&seed, &manifests, &assets);
        assert!(
            issues
                .iter()
                .any(|i| matches!(i, DataIssue::GameMismatch { .. })),
            "应报告游戏标识不一致：{issues:?}"
        );
    }

    #[test]
    fn seed_entry_without_manifest_is_detected() {
        let (seed, _, assets) = coherent_data();
        let empty = ManifestSet::from_sources(&[]);
        let issues = cross_check(&seed, &empty, &assets);
        assert!(
            issues
                .iter()
                .any(|i| matches!(i, DataIssue::SeedEntryWithoutManifest { .. })),
            "应报告种子表条目无人消费：{issues:?}"
        );
    }

    #[test]
    fn asset_paths_are_checked_and_notice_is_not_a_problem() {
        let (seed, _, _) = coherent_data();
        let manifests = ManifestSet::from_sources(&[(
            "unlocker.json",
            r#"{
              "id": "sample-ns/sample-tool",
              "game": "sample-game",
              "name": "示例解锁",
              "description": "运行时工具。",
              "version": "0.1.0",
              "type": "external_process",
              "risk_level": "L3",
              "permissions": ["launch_external"],
              "requires_admin": false,
              "backup_required": false,
              "entry": { "executor": "unlocker", "asset": "sample-asset", "channel": "releases" },
              "source": { "kind": "upstream", "repo": "owner/repo", "license": "MIT" },
              "pending_verifications": []
            }"#,
        )]);

        // (a) 资产完全不存在 → Problem
        let empty_assets = AssetCatalog::parse(
            r#"{ "schema_version": "0.1.0", "updated_at": "2026-09-18", "assets": [] }"#,
        )
        .unwrap();
        let issues = cross_check(&seed, &manifests, &empty_assets);
        let missing = issues
            .iter()
            .find(|i| matches!(i, DataIssue::MissingAsset { .. }))
            .expect("应报告缺失资产");
        assert_eq!(missing.severity(), Severity::Problem);

        // (b) 资产存在但 artifacts 为空 → Notice（Q1 未定稿的合法态，不应报错）
        let unconfigured = AssetCatalog::parse(
            r#"{
              "schema_version": "0.1.0",
              "updated_at": "2026-09-18",
              "assets": [
                {
                  "asset_key": "sample-asset",
                  "tool_id": "sample-ns/sample-tool",
                  "channel": "releases",
                  "upstream": { "repo": "owner/repo", "license": "MIT", "version": "v1.0.0" },
                  "artifacts": []
                }
              ]
            }"#,
        )
        .unwrap();
        let issues = cross_check(&seed, &manifests, &unconfigured);
        assert!(
            issues
                .iter()
                .any(|i| matches!(i, DataIssue::AssetNotConfigured { .. })),
            "应报告「资产无发行物」：{issues:?}"
        );
        assert!(
            !issues
                .iter()
                .any(|i| matches!(i, DataIssue::MissingAsset { .. })),
            "资产已声明，不应再报缺失"
        );
        assert!(
            issues.iter().all(|i| i.severity() == Severity::Notice),
            "未定稿不应被当成缺陷：{issues:?}"
        );
    }

    #[test]
    fn builtin_data_loads_coherently() {
        // 真实数据的整体一致性：这是「随包数据可用」的端到端断言。
        // 具体内容（游戏清单 / 工具条目）由 npm run validate:data 守护，
        // 此处只断言装载不降级、无真实缺陷 —— 刻意不出现任何游戏标识。
        let data = BuiltinData::load();
        assert!(
            data.degradations.is_empty(),
            "内置数据不应触发降级：{:?}",
            data.degradations
        );
        assert!(
            !data.has_problems(),
            "内置数据不应有跨文件缺陷：{:?}",
            data.issues
        );
        assert!(!data.is_degraded());
        assert!(!data.manifests.is_empty());

        // 应当存在「资产未配置」须知（Q1 未定稿的真实状态）—— 它是合法态，不是缺陷。
        // 若将来 Q1 定稿并填入 artifacts，这条断言会失败，提示同步更新此处预期。
        assert!(
            data.notices()
                .any(|i| matches!(i, DataIssue::AssetNotConfigured { .. })),
            "预期存在「资产构建管道未定稿」须知项：{:?}",
            data.issues
        );
    }
}
