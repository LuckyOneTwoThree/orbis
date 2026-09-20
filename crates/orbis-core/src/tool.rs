//! 工具域通用词汇（`pm/04-技术设计.md` §5.7 / `docs/ipc-contract.md` §2、§6）。
//!
//! # 为什么这些枚举在 core 而不是 tools
//!
//! 它们**同时被两份数据源消费**：
//!
//! - `risk_level` 既出现在 `data/compatibility/seed.json` 的条目里，也出现在
//!   Tool Manifest 里（权威值在 Manifest，seed 侧为展示用，见 seed.schema.json）
//! - `ToolSource` 是 `pm/00-产品基石.md` §8.6「第三方项目必须记录 License / Source /
//!   Author」义务在数据结构上的载体
//!
//! 若把它们放进 `orbis-tools`，`orbis-core` 的 seed 加载器就得自带一份同义枚举，
//! 于是同一条规则出现两个定义 —— 这正是项目一直在消除的双源问题。
//!
//! Manifest 的**聚合结构**（`ToolManifest`）仍然归 `orbis-tools`：04 §4.1 明确
//! 「Manifest 加载」是 tools 的职责，本模块只提供它所需的词汇与校验。

use crate::model::GameId;

/// 风险分级（`pm/00-产品基石.md` §8.1；L0 只读 → L3 进程级修改）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    /// 严格只读（扫描 / 版本读取 / 时长统计）
    L0,
    /// 落盘配置修改（走备份 → 修改 → 验证）
    L1,
    /// 启动参数等（持久化但不改游戏文件）
    L2,
    /// 进程级修改（注入 / 附加），**必须显式授权**
    L3,
}

impl RiskLevel {
    /// 契约与 JSON 中的字面量（大写带 `L` 前缀，如 `"L1"`）。
    pub const fn literal(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
        }
    }

    /// 严格解析 —— 大小写 / 空白错误一律 `None`，避免数据错误被静默吞掉。
    pub fn from_literal(literal: &str) -> Option<Self> {
        match literal {
            "L0" => Some(Self::L0),
            "L1" => Some(Self::L1),
            "L2" => Some(Self::L2),
            "L3" => Some(Self::L3),
            _ => None,
        }
    }

    /// 是否必须走显式授权流（00 §12.3 规则 7 / 契约 §5 `CONSENT_REQUIRED`）。
    pub const fn requires_explicit_consent(self) -> bool {
        matches!(self, Self::L3)
    }
}

/// 工具类型 = 执行器分派键（04 §5.7 `ToolRuntime`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    /// 落盘配置修改 → `EnhancementExecutor`（04 §5.5）
    ConfigModify,
    /// 外部进程 → `UnlockerRunner`（04 §5.8）
    ExternalProcess,
}

impl ToolType {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ConfigModify => "config_modify",
            Self::ExternalProcess => "external_process",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "config_modify" => Some(Self::ConfigModify),
            "external_process" => Some(Self::ExternalProcess),
            _ => None,
        }
    }

    /// 是否会落盘改写游戏文件 —— 决定 `backup_required` 的下限（规则 5）。
    pub const fn writes_game_files(self) -> bool {
        matches!(self, Self::ConfigModify)
    }
}

/// 工具申请的权限范围（01 C3 验收要求「展示风险等级与权限范围」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPermission {
    ReadConfig,
    WriteConfig,
    LaunchExternal,
    ProcessAttach,
}

impl ToolPermission {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ReadConfig => "read_config",
            Self::WriteConfig => "write_config",
            Self::LaunchExternal => "launch_external",
            Self::ProcessAttach => "process_attach",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "read_config" => Some(Self::ReadConfig),
            "write_config" => Some(Self::WriteConfig),
            "launch_external" => Some(Self::LaunchExternal),
            "process_attach" => Some(Self::ProcessAttach),
            _ => None,
        }
    }
}

/// 工具来源类别（命名空间口径见 `pm/Phase0-冲刺C-F2种子表定稿.md` §1.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// `orbis-builtin/*` —— Orbis 自研
    Builtin,
    /// `orbis-bundled/*` —— 随包分发但实现来自上游
    Bundled,
    /// 上游外部项目（必须记录 repo + license）
    Upstream,
}

impl SourceKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Bundled => "bundled",
            Self::Upstream => "upstream",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "builtin" => Some(Self::Builtin),
            "bundled" => Some(Self::Bundled),
            "upstream" => Some(Self::Upstream),
            _ => None,
        }
    }
}

/// 工具来源声明（00 §8.6 的 License / Source / Author 记录义务）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSource {
    pub kind: SourceKind,
    /// 上游仓库（`SourceKind::Upstream` 时必填）
    pub repo: Option<String>,
    /// 上游 License 标识（如 `"MIT"`；`Upstream` 时必填）
    pub license: Option<String>,
    /// 上游版本（如 `"v3.0.4"`）
    pub version: Option<String>,
}

impl ToolSource {
    pub fn new(kind: SourceKind) -> Self {
        Self {
            kind,
            repo: None,
            license: None,
            version: None,
        }
    }

    /// `Upstream` 必须在数据里写明 repo 与 license，否则无法履行署名义务。
    pub fn is_attribution_complete(&self) -> bool {
        if self.kind != SourceKind::Upstream {
            return true;
        }
        matches!(
            (self.repo.as_deref(), self.license.as_deref()),
            (Some(r), Some(l)) if !r.trim().is_empty() && !l.trim().is_empty()
        )
    }
}

/// 工具标识形态校验：`namespace/name`（seed 的 `tool_id` 与 Manifest 的 `id` 共用）。
///
/// **只校验形态，不校验存在性** —— 存在性属于数据层（`validate:data` 的跨文件一致性），
/// Core 不得知道有哪些工具。两段分别复用 [`GameId`] 的 slug 规则，因此
/// `-lead` / `trail-` / `Upper` / 中文一律拒绝。
pub fn is_valid_tool_id(id: &str) -> bool {
    let Some((namespace, name)) = id.split_once('/') else {
        return false;
    };
    if name.contains('/') {
        return false; // 只允许恰好一个斜杠
    }
    GameId::is_valid_slug(namespace) && GameId::is_valid_slug(name)
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.literal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_level_literals_roundtrip_and_are_strict() {
        for level in [RiskLevel::L0, RiskLevel::L1, RiskLevel::L2, RiskLevel::L3] {
            assert_eq!(RiskLevel::from_literal(level.literal()), Some(level));
        }
        // 严格解析：不得静默容错
        assert_eq!(RiskLevel::from_literal("l1"), None);
        assert_eq!(RiskLevel::from_literal("L1 "), None);
        assert_eq!(RiskLevel::from_literal("L4"), None);
    }

    #[test]
    fn only_l3_requires_explicit_consent() {
        assert!(RiskLevel::L3.requires_explicit_consent());
        assert!(!RiskLevel::L2.requires_explicit_consent());
        assert!(!RiskLevel::L1.requires_explicit_consent());
        assert!(!RiskLevel::L0.requires_explicit_consent());
    }

    #[test]
    fn tool_type_slugs_match_contract_literals() {
        assert_eq!(ToolType::ConfigModify.slug(), "config_modify");
        assert_eq!(ToolType::ExternalProcess.slug(), "external_process");
        assert_eq!(
            ToolType::from_slug("external_process"),
            Some(ToolType::ExternalProcess)
        );
        assert_eq!(ToolType::from_slug("mod"), None);
    }

    #[test]
    fn only_config_modify_writes_game_files() {
        // 规则 5「修改前必须备份」只对落盘修改成立；L3 不落盘（backup_required = false）
        assert!(ToolType::ConfigModify.writes_game_files());
        assert!(!ToolType::ExternalProcess.writes_game_files());
    }

    #[test]
    fn tool_permission_slugs_match_contract_literals() {
        for p in [
            ToolPermission::ReadConfig,
            ToolPermission::WriteConfig,
            ToolPermission::LaunchExternal,
            ToolPermission::ProcessAttach,
        ] {
            assert_eq!(ToolPermission::from_slug(p.slug()), Some(p));
        }
        assert_eq!(ToolPermission::from_slug("attach"), None);
    }

    #[test]
    fn source_kind_slugs_match_contract_literals() {
        for k in [
            SourceKind::Builtin,
            SourceKind::Bundled,
            SourceKind::Upstream,
        ] {
            assert_eq!(SourceKind::from_slug(k.slug()), Some(k));
        }
        assert_eq!(SourceKind::from_slug("community"), None);
    }

    #[test]
    fn upstream_source_requires_attribution() {
        // 00 §8.6：上游项目必须写明仓库与 License，否则无法在 NOTICE 中署名
        let mut s = ToolSource::new(SourceKind::Upstream);
        assert!(!s.is_attribution_complete());
        s.repo = Some("34736384/genshin-fps-unlock".into());
        s.license = Some("MIT".into());
        assert!(s.is_attribution_complete());

        // 空白字符串不算填写 —— 否则「占位但未回填」会被当成完整
        s.license = Some("   ".into());
        assert!(!s.is_attribution_complete());

        // builtin 无署名义务
        assert!(ToolSource::new(SourceKind::Builtin).is_attribution_complete());
    }

    #[test]
    fn tool_id_requires_exactly_one_slash_and_valid_slugs() {
        assert!(is_valid_tool_id("orbis-builtin/wuwa-fps-120"));
        assert!(is_valid_tool_id("orbis-bundled/genshin-fps-unlock"));

        assert!(!is_valid_tool_id("no-namespace"));
        assert!(!is_valid_tool_id("a/b/c"));
        assert!(!is_valid_tool_id("/name"));
        assert!(!is_valid_tool_id("ns/"));
        assert!(!is_valid_tool_id("Ns/name"));
        assert!(!is_valid_tool_id("ns/Na me"));
        assert!(!is_valid_tool_id("ns/-lead"));
        assert!(!is_valid_tool_id("tool/名"));
    }
}
