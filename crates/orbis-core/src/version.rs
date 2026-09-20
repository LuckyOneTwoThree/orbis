//! 版本归一化与比较（`pm/04-技术设计.md` §5.1 归一化契约 / §5.2 比较器）。
//!
//! # 为什么需要归一化
//!
//! exe `VERSIONINFO` 实际产出常为多段构建号（如 `7.0.10.xxxxx`），而种子表与远程
//! 版本源的口径是 `major.minor`。若不归一化，`exe_ver(7.0.10.x)` 会恒大于
//! `remote(7.0)`，导致**静默漏报更新**；同时 exact 种子条目会常态化落空。
//!
//! # 规则（不得放宽）
//!
//! - 归一化只取**前两段**，且两段都必须是纯数字
//! - 任一段缺失 / 非数值 → `None`，即「未知」。**绝不猜测、绝不用缓存值兜底**
//!   （`pm/02-MVP-PRD.md` A3：识别失败必须显式可见）
//! - 比较只发生在归一化之后，且在 `(major, minor)` 粒度

use std::cmp::Ordering;
use std::fmt;

/// 归一化后的版本（`major.minor`）。构造只能经 [`normalize`] 或 [`Version::parse`]，
/// 因此拿到本类型就代表「版本已知」，无需再判空。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    /// 严格解析 `major.minor`（两段且均为纯数字）。多段 / 缺段 / 非数值一律 `None`。
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (a, b) = text.split_once('.')?;
        if b.contains('.') {
            return None; // 需要调用方显式走 normalize；这里不静默截断
        }
        let major = parse_segment(a)?;
        let minor = parse_segment(b)?;
        Some(Self { major, minor })
    }

    /// 种子表的 `prefix` 条目（形如 `3.x`）是否覆盖本版本。
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        prefix_major(prefix) == Some(self.major)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// 只接受 ASCII 数字（拒绝 `+` / `-` / 空白 / 非 ASCII 数字，避免 `"+3"` 这类解析意外）。
fn parse_segment(seg: &str) -> Option<u32> {
    if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    seg.parse::<u32>().ok()
}

/// 取出 prefix 键（形如 `3.x`）的主版本号；形态非法 → `None`。
///
/// [`Version::matches_prefix`] 与 [`is_prefix_key`] 共用本函数，使「匹配期判定」与
/// 「加载期校验」不可能出现两套规则 —— 若各写一份，seed 里一个非法 prefix 键
/// 会在加载时通过、在匹配时静默永不命中。
fn prefix_major(key: &str) -> Option<u32> {
    let (major, wildcard) = key.split_once('.')?;
    if wildcard != "x" {
        return None;
    }
    parse_segment(major)
}

/// prefix 键形态校验（供 seed 加载期使用）。
pub fn is_prefix_key(key: &str) -> bool {
    prefix_major(key).is_some()
}

/// 版本归一化契约：`raw → (major, minor)`。
///
/// ```text
/// "3.5.0.128940" -> Some(3.5)
/// "7.1"          -> Some(7.1)
/// "7"            -> None   （缺段，不猜）
/// "v7.1"         -> None   （非数值，不猜）
/// ""             -> None
/// ```
pub fn normalize(raw: &str) -> Option<Version> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let mut parts = text.split('.');
    let major = parse_segment(parts.next()?)?;
    let minor = parse_segment(parts.next()?)?;
    Some(Version { major, minor })
}

/// 数值点分比较（游戏版本非 semver，不可用字符串比较）。
pub fn compare(a: Version, b: Version) -> Ordering {
    a.cmp(&b)
}

/// E1 更新判定：本地严格小于远程才算有更新。
///
/// 任一侧未知（`None`）时返回 `false` —— 即「**不误报**」
/// （`pm/02-MVP-PRD.md` E1：「版本数据源不可达时显示 Unknown，不误报」）。
/// 未知态由调用方通过独立的 `version_unknown` 标志呈现，不在此处与「无更新」混淆。
pub fn is_update_available(local: Option<Version>, remote: Option<Version>) -> bool {
    match (local, remote) {
        (Some(l), Some(r)) => l < r,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_takes_first_two_segments() {
        assert_eq!(
            normalize("3.5.0.128940"),
            Some(Version { major: 3, minor: 5 })
        );
        assert_eq!(
            normalize("7.0.10.4821"),
            Some(Version { major: 7, minor: 0 })
        );
        assert_eq!(normalize("7.1"), Some(Version { major: 7, minor: 1 }));
        assert_eq!(normalize(" 2.7 "), Some(Version { major: 2, minor: 7 }));
    }

    #[test]
    fn normalize_refuses_to_guess() {
        // 02 A3：识别失败必须显示 Unknown，不猜、不显示过期缓存
        assert_eq!(normalize("7"), None);
        assert_eq!(normalize("v7.1"), None);
        assert_eq!(normalize("7.x"), None);
        assert_eq!(normalize("7.1-beta"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("Patch 2.6"), None);
        assert_eq!(normalize("+7.1"), None);
    }

    #[test]
    fn parse_is_stricter_than_normalize() {
        // parse 不接受多段：调用方必须显式选择 normalize，避免静默截断
        assert_eq!(Version::parse("7.1"), Some(Version { major: 7, minor: 1 }));
        assert_eq!(Version::parse("7.1.0.1"), None);
        assert_eq!(Version::parse("7"), None);
    }

    #[test]
    fn compares_numerically_not_lexically() {
        // 字符串比较会把 "10.0" 判成小于 "9.0"
        let v10 = normalize("10.0").unwrap();
        let v9 = normalize("9.0").unwrap();
        assert_eq!(compare(v10, v9), Ordering::Greater);
        assert_eq!(compare(v9, v10), Ordering::Less);
        assert_eq!(compare(v9, normalize("9.0").unwrap()), Ordering::Equal);
    }

    #[test]
    fn minor_is_compared_within_same_major() {
        assert!(normalize("3.10").unwrap() > normalize("3.9").unwrap());
        assert!(normalize("3.5").unwrap() < normalize("4.0").unwrap());
    }

    #[test]
    fn display_roundtrips() {
        assert_eq!(normalize("3.5.0.1").unwrap().to_string(), "3.5");
    }

    #[test]
    fn prefix_matching_covers_whole_minor_range() {
        // P0-C §1.3：prefix "3.x" 覆盖 3.0–3.9，避免每个小版本手工补条目
        let prefix = "3.x";
        assert!(normalize("3.0").unwrap().matches_prefix(prefix));
        assert!(normalize("3.5.0.1").unwrap().matches_prefix(prefix));
        assert!(normalize("3.9").unwrap().matches_prefix(prefix));
        assert!(!normalize("4.0").unwrap().matches_prefix(prefix));
        assert!(!normalize("2.7").unwrap().matches_prefix(prefix));
        assert!(
            !normalize("3.5").unwrap().matches_prefix("3.y"),
            "非法通配不匹配"
        );
    }

    #[test]
    fn prefix_key_validation_agrees_with_matching() {
        // 加载期校验（is_prefix_key）与匹配期判定（matches_prefix）必须一致：
        // 若二者漂移，一个非法 prefix 键会在加载时通过、在匹配时静默永不命中。
        for key in ["3.x", "10.x"] {
            assert!(is_prefix_key(key), "{key} 应为合法 prefix 键");
        }
        for key in ["3.5", "3.X", "x.x", ".x", "3.x.1", "", "3"] {
            assert!(!is_prefix_key(key), "{key} 应为非法 prefix 键");
            assert!(
                !normalize("3.5").unwrap().matches_prefix(key),
                "{key} 不应匹配任何版本"
            );
        }
    }

    #[test]
    fn update_detection_never_over_reports() {
        let local = normalize("4.4.0.3105");
        let remote = normalize("4.5");
        assert!(is_update_available(local, remote));

        assert!(!is_update_available(normalize("4.5"), normalize("4.5")));
        assert!(!is_update_available(normalize("4.6"), normalize("4.5")));

        // 远程不可达 / 本地未知 → 一律不报有更新（E1 不误报）
        assert!(!is_update_available(local, None));
        assert!(!is_update_available(None, remote));
        assert!(!is_update_available(None, None));
    }

    #[test]
    fn regression_raw_build_number_must_not_hide_updates() {
        // 04 §5.1 记录的回归：不做归一化时 7.0.10.x 恒大于远程 7.0，更新被静默漏掉。
        // 归一化后二者同为 7.0，因此正确判定为「无更新」；
        // 而远程 7.1 时必须判为「有更新」。
        let local = normalize("7.0.10.4821").unwrap();
        assert_eq!(local, normalize("7.0").unwrap());
        assert!(!is_update_available(Some(local), normalize("7.0")));
        assert!(is_update_available(Some(local), normalize("7.1")));
    }
}
