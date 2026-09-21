//! 进程快照（`pm/04-技术设计.md` §5.10，A5 运行状态检测）。
//!
//! # 为什么 A5 不需要任何游戏知识
//!
//! 04 §5.10 定的匹配口径是「**exe 路径前缀**」，而安装实例的 `executable_path`
//! 已经在库里 —— 于是判断「某实例是否在跑」只需要比较路径字符串，
//! **不需要任何按游戏的进程名常量**（`GenshinImpact.exe` / `client.exe` 之类）。
//! 这正是 A5 能在实测项（T5/T7）收敛前落地的原因：它不依赖 providers 给任何新数据。
//!
//! # 平台策略（docs/05 §1.1）
//!
//! `sysinfo` 本身跨平台，因此本模块**不需要** `#[cfg(windows)]` 门控 ——
//! 在 macOS/Linux 上同样能 `cargo test`。但路径比较的大小写敏感度是平台属性：
//! Windows 路径大小写不敏感，其它平台敏感，因此那一段用了门控。
//! 真机行为只在 Windows 验收。
//!
//! # 「快照失败」与「进程已退出」是两件事
//!
//! - 快照成功、目标进程不在列表 → **真实的进程退出**（含崩溃），按 04 §5.10 应
//!   回落 `Installed` 并写日志
//! - 快照本身拿不到进程表 → **无法判断**，此时不该把「正在运行」谎报成「已停止」
//!
//! [`ProcessSnapshot::capture`] 是**不失败**的（枚举进程表在 sysinfo 里没有失败路径），
//! 因此调用方拿到的永远是「尽力而为的结果」：拿不到 exe 路径的进程会被跳过，
//! 而不是让整个快照失败。

/// 一个进程的最小表示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: u32,
    /// OS 原生路径字符串（Windows 用 `\`）。拿不到路径的进程不进快照
    pub exe_path: String,
}

/// 某一时刻的进程表快照。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessSnapshot {
    entries: Vec<ProcessEntry>,
}

impl ProcessSnapshot {
    /// 采集当前进程表。拿不到 exe 路径的进程会被跳过（系统/权限受限进程常见）。
    pub fn capture() -> Self {
        let mut system = sysinfo::System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        // `processes()` 返回 `&HashMap<Pid, Process>`（按 pid 索引），不是迭代器
        let entries = system
            .processes()
            .values()
            .filter_map(|process| {
                let exe_path = process.exe()?.to_string_lossy().into_owned();
                // 空路径没有匹配价值，且会让前缀匹配退化成「匹配一切」
                if exe_path.is_empty() {
                    return None;
                }
                Some(ProcessEntry {
                    pid: process.pid().as_u32(),
                    exe_path,
                })
            })
            .collect();

        Self { entries }
    }

    /// 空快照（用于测试，以及在无法采集时表达「一无所知」）。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 由给定的进程条目构造快照。
    ///
    /// 存在的理由是**依赖注入**：A5 的装配逻辑（壳层）必须能在不起真实进程的前提下
    /// 被测试，否则「运行态是否覆盖了落库状态」只能靠在 Windows 上真的装一款游戏
    /// 才能验证 —— 那会让这条规则永远无法进 CI。
    pub fn from_entries(entries: Vec<ProcessEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[ProcessEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 找出正在运行该 exe 的进程，返回其 pid。
    ///
    /// 匹配口径 = **前缀**（04 §5.10）：进程 exe 路径以记录的 `executable_path` 开头。
    /// 相等自然满足前缀，因此「子进程与主进程同路径」也能命中。
    /// 路径比较的大小写敏感度按平台处理（Windows 不敏感）。
    pub fn find_running(&self, executable_path: &str) -> Option<u32> {
        if executable_path.is_empty() {
            return None;
        }
        self.entries
            .iter()
            .find(|entry| path_matches(&entry.exe_path, executable_path))
            .map(|entry| entry.pid)
    }
}

/// 平台相关的路径相等/前缀判定。
fn path_matches(process_exe: &str, recorded_exe: &str) -> bool {
    #[cfg(windows)]
    {
        // Windows 路径大小写不敏感：进程表里的盘符大小写与用户选择时可能不同
        let process = process_exe.to_lowercase();
        let recorded = recorded_exe.to_lowercase();
        process.starts_with(&recorded)
    }
    #[cfg(not(windows))]
    {
        process_exe.starts_with(recorded_exe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_of(entries: &[(&str, u32)]) -> ProcessSnapshot {
        ProcessSnapshot {
            entries: entries
                .iter()
                .map(|(path, pid)| ProcessEntry {
                    pid: *pid,
                    exe_path: (*path).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn exact_path_matches() {
        let snapshot = snapshot_of(&[("C:/Games/Sample/game.exe", 100)]);
        assert_eq!(snapshot.find_running("C:/Games/Sample/game.exe"), Some(100));
    }

    #[test]
    fn prefix_matching_covers_processes_sharing_the_path() {
        // 04 §5.10 的口径：相等之外的同路径进程也应命中（例如主进程拉起的同路径子进程）
        let snapshot = snapshot_of(&[("C:/Games/Sample/game.exe", 7)]);
        assert_eq!(snapshot.find_running("C:/Games/Sample/game.exe"), Some(7));
        // 目录前缀也命中 —— 这是「前缀」而非「相等」的语义
        assert_eq!(snapshot.find_running("C:/Games/Sample/game"), Some(7));
    }

    #[test]
    fn unrelated_paths_do_not_match() {
        let snapshot = snapshot_of(&[("C:/Games/Other/game.exe", 100)]);
        assert_eq!(snapshot.find_running("C:/Games/Sample/game.exe"), None);
        // 反向前缀不得命中：短记录不应匹配上更长的进程路径之外的东西
        assert_eq!(snapshot.find_running("C:/Games/Other/game.exe.bak"), None);
    }

    #[test]
    fn empty_input_never_matches_everything() {
        // 前缀匹配的经典陷阱：空字符串是所有字符串的前缀，
        // 会让「没有记录路径」瞬间变成「全部在运行」
        let snapshot = snapshot_of(&[("C:/Games/Sample/game.exe", 100)]);
        assert_eq!(snapshot.find_running(""), None);
        assert_eq!(ProcessSnapshot::empty().find_running("C:/a.exe"), None);
    }

    #[test]
    fn the_real_process_table_contains_this_test_binary() {
        // 端到端：sysinfo 真的能枚举到当前进程。用 std 拿当前 exe 路径做记录值。
        let Ok(current) = std::env::current_exe() else {
            return; // 极少数环境拿不到自身路径，跳过而不是误报失败
        };
        let recorded = current.to_string_lossy().into_owned();
        let snapshot = ProcessSnapshot::capture();
        assert!(!snapshot.is_empty(), "进程表不应为空");
        assert_eq!(
            snapshot.find_running(&recorded),
            Some(std::process::id()),
            "快照里应能找到当前测试进程（pid 应与自身一致）"
        );
    }
}
