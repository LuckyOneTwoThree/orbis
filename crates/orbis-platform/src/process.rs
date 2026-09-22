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

/// 路径归一化：剥掉 `\\?\` 长路径前缀，并把分隔符统一为 `\`。
///
/// 为什么必须做：`sysinfo` 报告的路径形态与用户在文件对话框里的选择**不保证同形** ——
/// 分隔符（`/` vs `\`）与 `\\?\` 前缀都可能只出现在一侧。不归一化就会**漏报**
/// （游戏在跑却显示未运行 → 时长不统计、状态不更新），而漏报是静默降级，撞 04 §8。
fn normalize(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('/', "\\")
}

/// 平台相关的路径相等/前缀判定（04 §5.10：**以路径分隔符为界**的前缀）。
///
/// 判据 = 归一化后「完全相等」或「以 `记录 + 分隔符` 开头」。边界是必要的：
/// 记录值若退化成目录（或任何短于实际 exe 的路径），裸字符串前缀会让 `...\Endfield`
/// 命中 `...\Endfield2\game.exe` —— 多游戏共存的机器上这不是构造场景。
fn path_matches(process_exe: &str, recorded_exe: &str) -> bool {
    let process = normalize(process_exe);
    let recorded = normalize(recorded_exe);
    // Windows 路径大小写不敏感：进程表里的盘符大小写与用户选择时可能不同
    #[cfg(windows)]
    let (process, recorded) = (process.to_lowercase(), recorded.to_lowercase());

    if process == recorded {
        return true;
    }
    // 记录值以分隔符结尾（目录）→ 直接用；否则要求「记录 + 分隔符」为前缀
    let prefix = if recorded.ends_with('\\') {
        recorded
    } else {
        format!("{recorded}\\")
    };
    process.starts_with(&prefix)
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
    fn prefix_matching_stops_at_a_path_separator() {
        let snapshot = snapshot_of(&[("C:\\Games\\Sample\\game.exe", 7)]);
        // 相等
        assert_eq!(
            snapshot.find_running("C:\\Games\\Sample\\game.exe"),
            Some(7)
        );
        // 记录值是目录（带不带结尾分隔符都算同一个目录）→ 该目录下的 exe 命中
        assert_eq!(snapshot.find_running("C:\\Games\\Sample\\"), Some(7));
        assert_eq!(snapshot.find_running("C:\\Games\\Sample"), Some(7));

        // 但**只共享字符前缀的兄弟目录不得命中** —— 这才是「以分隔符为界」的意义
        let sibling = snapshot_of(&[("C:\\Games\\Sample2\\game.exe", 8)]);
        assert_eq!(
            sibling.find_running("C:\\Games\\Sample"),
            None,
            "裸字符串前缀会在这里误报（04 §5.10 要求以分隔符为界）"
        );
    }

    #[test]
    fn unrelated_paths_do_not_match() {
        let snapshot = snapshot_of(&[("C:/Games/Other/game.exe", 100)]);
        assert_eq!(snapshot.find_running("C:/Games/Sample/game.exe"), None);
        // 反向前缀不得命中：短记录不应匹配上更长的进程路径之外的东西
        assert_eq!(snapshot.find_running("C:/Games/Other/game.exe.bak"), None);
    }

    #[test]
    fn sibling_directories_sharing_a_prefix_do_not_match() {
        // 多游戏共存的真实形态：目录名互为前缀
        let snapshot = snapshot_of(&[
            ("C:\\Games\\Endfield2\\game.exe", 11),
            ("C:\\Games\\Endfield\\game.exe", 22),
        ]);
        assert_eq!(
            snapshot.find_running("C:\\Games\\Endfield\\"),
            Some(22),
            "目录记录值只能命中自己目录下的 exe"
        );
        assert_eq!(
            snapshot.find_running("C:\\Games\\Endfield2\\"),
            Some(11),
            "兄弟目录各自独立"
        );
        assert_eq!(
            snapshot.find_running("C:\\Games\\Endfield2\\game.exe"),
            Some(11)
        );

        // 字符前缀相同的第三个目录同样不得命中
        let third = ProcessSnapshot::from_entries(vec![ProcessEntry {
            pid: 33,
            exe_path: "C:\\Games\\Endfield-Extra\\game.exe".to_owned(),
        }]);
        assert_eq!(third.find_running("C:\\Games\\Endfield\\"), None);
    }

    #[test]
    fn path_form_differences_do_not_cause_false_negatives() {
        // **漏报是更现实的风险**：进程报告的形态与用户选择时的形态可能不同，
        // 三种差异都必须仍然命中 —— 否则就是「游戏在跑却显示未运行」（静默降级）。
        let snapshot = snapshot_of(&[("C:\\Games\\Foo\\game.exe", 5)]);

        // 1) 分隔符：用户选择可能是正斜杠（文件对话框的常见返回）
        assert_eq!(snapshot.find_running("C:/Games/Foo/game.exe"), Some(5));
        // 2) `\\?\` 长路径前缀只出现在**记录**侧
        assert_eq!(snapshot.find_running(r"\\?\C:\Games\Foo\game.exe"), Some(5));

        // 3) `\\?\` 只出现在**进程**侧（sysinfo 在某些环境下如此）
        let verbatim = snapshot_of(&[(r"\\?\C:\Games\Foo\game.exe", 5)]);
        assert_eq!(verbatim.find_running("C:\\Games\\Foo\\game.exe"), Some(5));
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
