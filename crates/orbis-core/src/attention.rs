//! 「需处理」判定式（`pm/04-技术设计.md` §6.4.3）。
//!
//! # 为什么放在 Core
//!
//! 首页摘要条、卡片徽标、`UpdateStatus` 都依赖同一套判定。若让 UI 自行计算，
//! 就会出现两处口径不一致（原前端 mock 正是这样：把状态压成单一枚举后无法聚合）。
//! 因此判定**在 Core 算一次**，UI 只消费结果（04 §3.2：主进程承担全部决策）。
//!
//! 判定式：
//!
//! ```text
//! needsAttention :=
//!      updateAvailable                      // 有更新（工具可能失效）
//!   ∨  status == Broken                     // 游戏异常
//!   ∨  version_norm 未知                    // 版本无法识别 → B6 / E1 判定基础缺失
//!   ∨  ∃ tool: compat ∈ { Unknown, Incompatible, Deprecated }
//! ```
//!
//! 注意最后一条**不论工具是否启用**：用户需要知道「这个工具现在还能不能用」，
//! 这正是兼容性数据集的价值主张（00 §8.8）。

use crate::compat::CompatStatus;
use crate::model::GameRuntimeStatus;
use crate::version::Version;

/// 需处理的原因（对应契约 §2 的 `AttentionReason`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttentionReason {
    UpdateAvailable,
    Broken,
    VersionUnknown,
    ToolUnknown,
    ToolIncompatible,
}

impl AttentionReason {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::UpdateAvailable => "update_available",
            Self::Broken => "broken",
            Self::VersionUnknown => "version_unknown",
            Self::ToolUnknown => "tool_unknown",
            Self::ToolIncompatible => "tool_incompatible",
        }
    }
}

/// 判定输入。刻意做成「显式入参」而不是读取数据库——便于单元测试穷举组合。
#[derive(Debug, Clone)]
pub struct AttentionInput {
    pub status: GameRuntimeStatus,
    pub update_available: bool,
    /// `None` = 本地版本无法识别
    pub version_norm: Option<Version>,
    /// 该游戏全部工具的兼容状态（无工具则为空）
    pub tool_compat: Vec<CompatStatus>,
}

impl AttentionInput {
    /// 便于测试与构造的简洁构造器。
    pub fn new(
        status: GameRuntimeStatus,
        update_available: bool,
        version_norm: Option<Version>,
        tool_compat: Vec<CompatStatus>,
    ) -> Self {
        Self {
            status,
            update_available,
            version_norm,
            tool_compat,
        }
    }
}

/// 返回去重且按类型排序的需处理原因；空向量即「无需处理」。
pub fn attention_reasons(input: &AttentionInput) -> Vec<AttentionReason> {
    let mut reasons = Vec::new();

    if input.status == GameRuntimeStatus::Broken {
        reasons.push(AttentionReason::Broken);
    }
    if input.version_norm.is_none() {
        reasons.push(AttentionReason::VersionUnknown);
    }
    if input.update_available {
        reasons.push(AttentionReason::UpdateAvailable);
    }

    // 工具维：Unknown 与「硬阻止」拆成两类，便于首页分类计数
    if input
        .tool_compat
        .iter()
        .any(|c| *c == CompatStatus::Unknown)
    {
        reasons.push(AttentionReason::ToolUnknown);
    }
    if input.tool_compat.iter().any(|c| c.is_hard_blocked()) {
        reasons.push(AttentionReason::ToolIncompatible);
    }

    reasons.sort();
    reasons.dedup();
    reasons
}

pub fn needs_attention(input: &AttentionInput) -> bool {
    !attention_reasons(input).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::normalize;

    fn v(raw: &str) -> Option<Version> {
        normalize(raw)
    }

    #[test]
    fn healthy_game_needs_no_attention() {
        // 已安装 + 版本已知 + 无更新 + 工具推断兼容 → 无需处理
        let input = AttentionInput::new(
            GameRuntimeStatus::Installed,
            false,
            v("7.1"),
            vec![CompatStatus::Compatible],
        );
        assert!(!needs_attention(&input));
        assert!(attention_reasons(&input).is_empty());
    }

    #[test]
    fn running_alone_is_not_an_attention_item() {
        // 「正在运行」是正常状态，不应出现在待处理清单里
        let input = AttentionInput::new(GameRuntimeStatus::Running, false, v("4.5"), vec![]);
        assert!(!needs_attention(&input));
    }

    #[test]
    fn tool_unknown_flags_attention_even_when_not_enabled() {
        // 鸣潮 3.x 的常态：工具未启用，但用户必须知道它现在能不能用（00 §8.8）
        let input = AttentionInput::new(
            GameRuntimeStatus::Installed,
            false,
            v("3.5.0.128940"),
            vec![CompatStatus::Unknown],
        );
        assert_eq!(attention_reasons(&input), vec![AttentionReason::ToolUnknown]);
    }

    #[test]
    fn incompatible_tool_flags_tool_incompatible_not_unknown() {
        let input = AttentionInput::new(
            GameRuntimeStatus::Installed,
            false,
            v("1.0"),
            vec![CompatStatus::Incompatible],
        );
        assert_eq!(
            attention_reasons(&input),
            vec![AttentionReason::ToolIncompatible]
        );
    }

    #[test]
    fn deprecated_is_also_a_hard_block() {
        let input = AttentionInput::new(
            GameRuntimeStatus::Installed,
            false,
            v("1.0"),
            vec![CompatStatus::Deprecated],
        );
        assert_eq!(
            attention_reasons(&input),
            vec![AttentionReason::ToolIncompatible]
        );
    }

    #[test]
    fn version_unknown_is_independent_from_update() {
        // 04 §6.4.1：version_unknown 与 update_available 正交。
        // 版本未知时不应被误判为「有更新」（E1 不误报）。
        let input = AttentionInput::new(GameRuntimeStatus::Installed, false, None, vec![]);
        assert_eq!(
            attention_reasons(&input),
            vec![AttentionReason::VersionUnknown]
        );
    }

    #[test]
    fn reasons_accumulate_and_dedupe() {
        let input = AttentionInput::new(
            GameRuntimeStatus::Broken,
            true,
            None,
            vec![
                CompatStatus::Unknown,
                CompatStatus::Unknown,
                CompatStatus::Incompatible,
            ],
        );
        let reasons = attention_reasons(&input);
        // 五类原因全部命中，且 Unknown 重复出现只计一次
        assert_eq!(reasons.len(), 5);
        assert_eq!(reasons.iter().filter(|r| **r == AttentionReason::ToolUnknown).count(), 1);
        assert!(reasons.contains(&AttentionReason::Broken));
        assert!(reasons.contains(&AttentionReason::VersionUnknown));
        assert!(reasons.contains(&AttentionReason::UpdateAvailable));
        assert!(reasons.contains(&AttentionReason::ToolIncompatible));
    }

    #[test]
    fn multiple_healthy_tools_do_not_flag() {
        let input = AttentionInput::new(
            GameRuntimeStatus::Installed,
            false,
            v("7.1"),
            vec![
                CompatStatus::Verified,
                CompatStatus::Compatible,
                CompatStatus::Verified,
            ],
        );
        assert!(!needs_attention(&input));
    }

    #[test]
    fn all_reason_slugs_match_contract() {
        // 契约 §2 的 AttentionReason 字面量
        for (reason, slug) in [
            (AttentionReason::UpdateAvailable, "update_available"),
            (AttentionReason::Broken, "broken"),
            (AttentionReason::VersionUnknown, "version_unknown"),
            (AttentionReason::ToolUnknown, "tool_unknown"),
            (AttentionReason::ToolIncompatible, "tool_incompatible"),
        ] {
            assert_eq!(reason.slug(), slug);
        }
    }
}
