//! 解锁器资产清单加载（`pm/04-技术设计.md` §5.8 / B7）。
//!
//! # 发行包分离
//!
//! 主安装包**不含**注入器（01 B7）。首次启用时按本清单下载并做 SHA256 校验；
//! 哈希不符 → 拒绝加载（防篡改）。清单随 Orbis 版本分发，`include_str!` 打包。
//!
//! # 「空 artifacts」是合法状态，不是错误
//!
//! 资产构建管道（04 §11 Q1：基于上游源码自建 vs 复用上游发行物）尚未定稿，
//! 因此 `artifacts` 当前为空数组。此时 [`AssetCatalog::download_url_configured`]
//! 返回 `false`，UI 走 `TOOL_ASSET_NOT_CONFIGURED` 降级路径，显示「组件构建管道
//! 待定稿」——**不显示为错误、不阻塞游戏本体启动**。把它当成错误会让 L3 未定稿
//! 这件事污染整个应用的可用性。

use std::fmt;

use orbis_core::is_valid_tool_id;
use serde::Deserialize;

/// 本加载器支持的 assets schema 版本（须与 `assets.schema.json` 的 `const` 一致）。
pub const SUPPORTED_SCHEMA_VERSION: &str = "0.1.0";

/// MVP 只发行 Windows x64（02 §5）。严格匹配 —— 平台串写错会让资产查询静默落空。
const SUPPORTED_ARTIFACT_PLATFORMS: &[&str] = &["windows-x64"];

/// 资产清单加载失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    Malformed {
        detail: String,
    },
    UnsupportedSchema {
        found: String,
        expected: &'static str,
    },
    InvalidEntry {
        index: usize,
        detail: String,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { detail } => write!(f, "assets.json 结构非法：{detail}"),
            Self::UnsupportedSchema { found, expected } => write!(
                f,
                "assets schema_version 不符：文件为 {found:?}，加载器支持 {expected:?}"
            ),
            Self::InvalidEntry { index, detail } => write!(f, "assets[{index}] 非法：{detail}"),
        }
    }
}

impl std::error::Error for AssetError {}

/// 打包形态（上游为 C#/.NET 8；self-contained 免用户装运行时，代价约 +70MB，04 §5.8）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packaging {
    SelfContained,
    FrameworkDependent,
}

impl Packaging {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::SelfContained => "self-contained",
            Self::FrameworkDependent => "framework-dependent",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "self-contained" => Some(Self::SelfContained),
            "framework-dependent" => Some(Self::FrameworkDependent),
            _ => None,
        }
    }
}

/// 上游署名信息（00 §8.6；与 Manifest 的 `source` 应一致）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamRef {
    pub repo: String,
    pub license: String,
    pub version: String,
}

/// 一个可下载的发行物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub platform: String,
    pub version: String,
    /// Orbis Releases 独立资产 URL（非上游仓库直链，便于镜像与下架免疫）
    pub url: String,
    /// 小写 64 位十六进制
    pub sha256: String,
    /// 字节数；下载前空间检查用
    pub size: u64,
    pub packaging: Option<Packaging>,
}

/// 一个资产 key 的完整定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetEntry {
    /// Manifest 的 `entry.asset` 引用此值
    pub asset_key: String,
    pub tool_id: String,
    pub channel: String,
    pub upstream: UpstreamRef,
    /// 空 = 构建管道未定稿（Q1）
    pub artifacts: Vec<Artifact>,
}

impl AssetEntry {
    /// 是否有可下载的发行物。`false` → `TOOL_ASSET_NOT_CONFIGURED` 降级路径。
    pub fn has_downloadable_artifact(&self) -> bool {
        !self.artifacts.is_empty()
    }
}

/// 内存中的资产清单（量与工具数同阶，只读索引）。
///
/// `schema_version` 为 `None` 表示降级态（文件缺失 / 损坏 / 版本不符）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetCatalog {
    schema_version: Option<String>,
    updated_at: Option<String>,
    note: Option<String>,
    assets: Vec<AssetEntry>,
}

impl AssetCatalog {
    /// 空清单 = 降级态：一切资产查询返回「未配置」，工具照常可见但不可下载。
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn parse(json: &str) -> Result<Self, AssetError> {
        let RawCatalog {
            schema_version,
            updated_at,
            note,
            assets: raw_assets,
        } = serde_json::from_str::<RawCatalog>(json).map_err(|e| AssetError::Malformed {
            detail: e.to_string(),
        })?;

        if schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(AssetError::UnsupportedSchema {
                found: schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }

        let mut assets = Vec::with_capacity(raw_assets.len());
        for (index, raw) in raw_assets.into_iter().enumerate() {
            assets.push(convert_entry(index, raw)?);
        }

        Ok(Self {
            schema_version: Some(schema_version),
            updated_at: Some(updated_at),
            note,
            assets,
        })
    }

    /// 加载随二进制打包的内置资产清单（04 §5.8 `include_str!`）。
    pub fn builtin() -> Result<Self, AssetError> {
        Self::parse(include_str!("../../../data/tools/assets.json"))
    }

    /// **推荐入口**：失败则降级为空清单并交回原因（供 D3 日志）。
    ///
    /// 注意降级后工具**仍然可见**，只是无法下载资产 —— 发行包分离的设计使
    /// 「资产清单坏掉」不会连带影响游戏启动。
    pub fn builtin_or_degraded() -> (Self, Option<AssetError>) {
        Self::parse_or_degraded(include_str!("../../../data/tools/assets.json"))
    }

    /// 降级组合：任何失败 → 空清单 + 原因。`builtin_or_degraded` 与单测共用本实现，
    /// 保证「真实数据路径」与「被测路径」是同一条。
    fn parse_or_degraded(json: &str) -> (Self, Option<AssetError>) {
        match Self::parse(json) {
            Ok(catalog) => (catalog, None),
            Err(err) => (Self::empty(), Some(err)),
        }
    }

    pub fn schema_version(&self) -> Option<&str> {
        self.schema_version.as_deref()
    }

    /// 清单维护日期原文（`YYYY-MM-DD`）；`None` = 降级态。
    pub fn updated_at(&self) -> Option<&str> {
        self.updated_at.as_deref()
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    pub fn entries(&self) -> &[AssetEntry] {
        &self.assets
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn is_degraded(&self) -> bool {
        self.schema_version.is_none()
    }

    pub fn get(&self, asset_key: &str) -> Option<&AssetEntry> {
        self.assets.iter().find(|a| a.asset_key == asset_key)
    }

    /// 契约 `ToolAssetDto.downloadUrlConfigured`：资产是否存在且有可下载发行物。
    ///
    /// `false` 时 UI 显示「组件构建管道待定稿」，**不显示为错误**（契约 §5
    /// `TOOL_ASSET_NOT_CONFIGURED`）。
    pub fn download_url_configured(&self, asset_key: &str) -> bool {
        self.get(asset_key)
            .map(AssetEntry::has_downloadable_artifact)
            .unwrap_or(false)
    }
}

fn convert_entry(index: usize, raw: RawAsset) -> Result<AssetEntry, AssetError> {
    let invalid = |detail: String| AssetError::InvalidEntry { index, detail };

    if raw.asset_key.trim().is_empty() {
        return Err(invalid("asset_key 不得为空".into()));
    }
    if !is_valid_tool_id(&raw.tool_id) {
        return Err(invalid(format!(
            "tool_id 须为 namespace/name 形态：{:?}",
            raw.tool_id
        )));
    }
    if raw.channel != "releases" {
        return Err(invalid(format!(
            "channel 仅支持 releases：{:?}",
            raw.channel
        )));
    }
    if raw.upstream.repo.trim().is_empty() || raw.upstream.license.trim().is_empty() {
        return Err(invalid(
            "upstream.repo / upstream.license 不得为空（00 §8.6 署名义务）".into(),
        ));
    }

    let mut artifacts = Vec::with_capacity(raw.artifacts.len());
    for (pos, a) in raw.artifacts.into_iter().enumerate() {
        if !SUPPORTED_ARTIFACT_PLATFORMS.contains(&a.platform.as_str()) {
            return Err(invalid(format!(
                "artifacts[{pos}] platform 不支持：{:?}（MVP 仅 {SUPPORTED_ARTIFACT_PLATFORMS:?}）",
                a.platform
            )));
        }
        if !is_sha256_hex(&a.sha256) {
            // 哈希是防篡改的唯一依据，形态错误等于没有校验
            return Err(invalid(format!(
                "artifacts[{pos}] sha256 须为 64 位小写十六进制：{:?}",
                a.sha256
            )));
        }
        if a.size == 0 {
            return Err(invalid(format!(
                "artifacts[{pos}] size 必须为正数（schema minimum: 1）"
            )));
        }
        if a.url.trim().is_empty() {
            return Err(invalid(format!("artifacts[{pos}] url 不得为空")));
        }
        let packaging = match a.packaging.as_deref() {
            None => None,
            Some(slug) => Some(
                Packaging::from_slug(slug)
                    .ok_or_else(|| invalid(format!("artifacts[{pos}] packaging 非法：{slug:?}")))?,
            ),
        };
        artifacts.push(Artifact {
            platform: a.platform,
            version: a.version,
            url: a.url,
            sha256: a.sha256,
            size: a.size,
            packaging,
        });
    }

    Ok(AssetEntry {
        asset_key: raw.asset_key,
        tool_id: raw.tool_id,
        channel: raw.channel,
        upstream: UpstreamRef {
            repo: raw.upstream.repo,
            license: raw.upstream.license,
            version: raw.upstream.version,
        },
        artifacts,
    })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// ── 反序列化中间结构（与 assets.schema.json 逐字对应）────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    schema_version: String,
    updated_at: String,
    #[serde(default)]
    note: Option<String>,
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAsset {
    asset_key: String,
    tool_id: String,
    channel: String,
    upstream: RawUpstream,
    artifacts: Vec<RawArtifact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUpstream {
    repo: String,
    license: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArtifact {
    platform: String,
    version: String,
    url: String,
    sha256: String,
    size: u64,
    #[serde(default)]
    packaging: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_ARTIFACTS: &str = r#"{
      "schema_version": "0.1.0",
      "updated_at": "2026-09-18",
      "note": "artifacts 为空 = 构建管道未定稿",
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

    fn with_one_artifact() -> String {
        let sha = "a".repeat(64);
        EMPTY_ARTIFACTS.replace(
            "\"artifacts\": []",
            &format!(
                r#""artifacts": [
                  {{
                    "platform": "windows-x64",
                    "version": "v1.0.0",
                    "url": "https://example.invalid/asset.zip",
                    "sha256": "{sha}",
                    "size": 73400320,
                    "packaging": "self-contained"
                  }}
                ]"#
            ),
        )
    }

    #[test]
    fn empty_artifacts_is_a_legal_state_not_an_error() {
        // Q1 未定稿的真实状态：能解析、工具可见、只是下载未配置
        let catalog = AssetCatalog::parse(EMPTY_ARTIFACTS).expect("空 artifacts 必须可解析");
        assert_eq!(catalog.schema_version(), Some("0.1.0"));
        assert!(!catalog.is_degraded());
        assert_eq!(catalog.entries().len(), 1);

        let entry = catalog.get("sample-asset").expect("资产条目应存在");
        assert!(!entry.has_downloadable_artifact());
        assert!(entry.artifacts.is_empty());

        // 契约 ToolAssetDto.downloadUrlConfigured = false → UI 走降级提示（非错误）
        assert!(!catalog.download_url_configured("sample-asset"));
        // 不存在的 key 同样返回 false，而不是 panic
        assert!(!catalog.download_url_configured("no-such-asset"));
    }

    #[test]
    fn configured_artifact_reports_download_available() {
        let catalog = AssetCatalog::parse(&with_one_artifact()).expect("带发行物应可解析");
        let entry = catalog.get("sample-asset").unwrap();
        assert!(entry.has_downloadable_artifact());
        assert!(catalog.download_url_configured("sample-asset"));

        let artifact = &entry.artifacts[0];
        assert_eq!(artifact.platform, "windows-x64");
        assert_eq!(artifact.size, 73_400_320);
        assert_eq!(artifact.packaging, Some(Packaging::SelfContained));
    }

    #[test]
    fn degraded_catalog_still_yields_visible_tools() {
        // 资产清单坏掉不得连带影响工具可见性（发行包分离的设计红利）
        let (catalog, err) = AssetCatalog::parse_or_degraded("{ broken");
        assert!(err.is_some());
        assert!(catalog.is_degraded());
        assert_eq!(catalog.schema_version(), None);
        assert!(!catalog.download_url_configured("sample-asset"));

        // 真实数据路径不应触发降级
        let (builtin, builtin_err) = AssetCatalog::builtin_or_degraded();
        assert!(builtin_err.is_none(), "内置数据不应降级：{builtin_err:?}");
        assert!(!builtin.is_degraded());
    }

    #[test]
    fn rejects_schema_mismatch_and_malformed_json() {
        let mismatched = EMPTY_ARTIFACTS.replace("\"0.1.0\"", "\"9.9.9\"");
        assert!(matches!(
            AssetCatalog::parse(&mismatched),
            Err(AssetError::UnsupportedSchema { .. })
        ));
        assert!(matches!(
            AssetCatalog::parse("{ nope"),
            Err(AssetError::Malformed { .. })
        ));
        // additionalProperties: false
        let extra = EMPTY_ARTIFACTS.replace("\"channel\"", "\"unexpected\": 1, \"channel\"");
        assert!(matches!(
            AssetCatalog::parse(&extra),
            Err(AssetError::Malformed { .. })
        ));
    }

    #[test]
    fn rejects_invalid_artifact_fields() {
        // 哈希形态错误 = 没有防篡改能力，必须在加载期拦下
        let bad_sha = with_one_artifact().replace(&"a".repeat(64), &"Z".repeat(64));
        assert!(matches!(
            AssetCatalog::parse(&bad_sha),
            Err(AssetError::InvalidEntry { .. })
        ));

        // 不支持的平台
        let bad_platform = with_one_artifact().replace("windows-x64", "linux-x64");
        assert!(matches!(
            AssetCatalog::parse(&bad_platform),
            Err(AssetError::InvalidEntry { .. })
        ));

        // size = 0（空间检查会失真）
        let zero_size = with_one_artifact().replace("\"size\": 73400320", "\"size\": 0");
        assert!(matches!(
            AssetCatalog::parse(&zero_size),
            Err(AssetError::InvalidEntry { .. })
        ));

        // 非法 packaging
        let bad_packaging = with_one_artifact().replace("\"self-contained\"", "\"portable\"");
        assert!(matches!(
            AssetCatalog::parse(&bad_packaging),
            Err(AssetError::InvalidEntry { .. })
        ));

        // 缺失署名
        let no_license = EMPTY_ARTIFACTS.replace("\"license\": \"MIT\"", "\"license\": \"\"");
        assert!(matches!(
            AssetCatalog::parse(&no_license),
            Err(AssetError::InvalidEntry { .. })
        ));
    }

    #[test]
    fn builtin_asset_index_is_loadable() {
        let catalog = AssetCatalog::builtin().expect("内置 assets.json 必须可解析");
        assert_eq!(catalog.schema_version(), Some(SUPPORTED_SCHEMA_VERSION));
        assert!(!catalog.is_degraded());
        assert!(!catalog.is_empty());
        // 每条资产的 tool_id 形态合法（不涉及具体游戏/工具标识的字面量）
        for entry in catalog.entries() {
            assert!(
                is_valid_tool_id(&entry.tool_id),
                "tool_id 非法：{}",
                entry.tool_id
            );
        }
    }
}
