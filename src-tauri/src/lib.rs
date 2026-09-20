//! Orbis Tauri 壳（**唯一组装根**，pm/04 §4.1）。
//!
//! # 职责边界
//!
//! 只做三件事：**命令注册、生命周期、事件桥**。业务逻辑在 `orbis-core` /
//! `orbis-platform` / `orbis-providers` / `orbis-tools`，此文件不得承载业务判断
//! （00 §7.9：UI 与壳层都不做决策）。
//!
//! # 当前实现范围
//!
//! 已实现：`windowControl`（契约 §3.10）+ 单实例互斥（04 §5.12）。
//! 其余 27 条命令待 Core 落地后按 `docs/ipc-contract.md` §3 逐条实现 ——
//! 命名必须与契约一致（camelCase，见下方 allow 说明）。

// 契约 §1 规定命令名为 camelCase（`windowControl` / `scanGames` / ...），
// 且 tauri-specta 会把 Rust 函数名直接镜像成 TS 侧的命令名。
// 因此这里刻意使用非 snake_case 命名，换取「契约、Rust 函数名、TS 调用名」三者一致。
#![allow(non_snake_case)]

use orbis_platform::log::{self, LogCategory, LogLevel, LogRecord, LogSource};
use tauri::Manager;

/// 窗口控制（契约 §3.10 / 03 §5.1 自绘标题栏）。
///
/// 发布包使用 `decorations: false`，窗口的最小化 / 最大化 / 关闭全部由界面按钮触发，
/// 因此这条命令是**可用性必需**而非可选增强。
#[tauri::command]
fn windowControl(window: tauri::Window, action: String) -> Result<(), String> {
    match action.as_str() {
        "minimize" => window.minimize().map_err(|e| e.to_string()),
        "maximize" => {
            // 前端只有一个按钮，语义是「切换最大化」，避免按钮状态与实际窗口状态不一致
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize().map_err(|e| e.to_string())
            } else {
                window.maximize().map_err(|e| e.to_string())
            }
        }
        "close" => window.close().map_err(|e| e.to_string()),
        other => Err(format!("unknown window action: {other}")),
    }
}

/// 内置数据装载事件的日志归属。
///
/// **一处需要回写文档的映射**：00 §9.1 / 04 §5.11 的 `Source` 枚举是
/// `Game / Tool / Launcher / Backup / Update`，没有「应用自身」这一档；而三份内置数据
/// （工具定义 / 工具资产 / 兼容状态）确实都是**工具域**数据，动作类别用 `Detect`
/// （加载即发现）。若将来 00 §9.1 增补 app 级来源，应改回并同步本文。
const DATA_SOURCE: LogSource = LogSource::Tool;
const DATA_CATEGORY: LogCategory = LogCategory::Detect;

/// 初始化 D3 日志并清理过期文件。返回日志是否成功落盘。
///
/// 日志初始化失败**不阻止启动**（诊断能力缺失不该让产品不可用），但必须显式可见：
/// 调用方据返回值决定是否用 stderr 兜底。
fn init_logging() -> bool {
    let Some(dir) = log::logs_dir() else {
        eprintln!("orbis: 无法解析应用数据目录 —— 本次日志不落盘");
        return false;
    };

    if let Err(err) = log::init(&dir) {
        eprintln!("orbis: 日志初始化失败 —— 本次日志不落盘：{err}");
        return false;
    }

    // 04 §5.11：启动时按保留期清理（整文件删除，不做行级裁剪）。
    // 保留天数暂用默认值；DB 落地后接 `app_setting.log.retention_days`。
    let report = log::cleanup_old_logs(&dir, log::DEFAULT_RETENTION_DAYS);
    if !report.is_clean() {
        eprintln!(
            "orbis: 日志清理有 {} 项失败，其余照常：{:?}",
            report.failed.len(),
            report.failed
        );
    }
    true
}

/// 记录一条降级/缺陷信息：写日志；若日志未落盘则回退到 stderr。
///
/// 两步缺一不可 —— 只写日志会在「日志不可用」时静默丢掉降级，
/// 这正是 04 §8「禁止静默失败」要防的情形。
fn announce(level: LogLevel, message: &str, logging_ok: bool) {
    LogRecord::new(DATA_SOURCE, DATA_CATEGORY, level, message).emit();
    if !logging_ok {
        eprintln!("orbis [{level}] {message}");
    }
}

/// 启动自检：校验 Core 契约可用，并**装载内置数据**。
///
/// 返回值只表达「Core 契约不可用」这类**硬失败**。数据降级**不阻止启动** ——
/// 02 C1（单个 Manifest 损坏）/ C4（种子表损坏）/ B7（资产未配置）都明确要求
/// 降级后仍可运行，但必须显式可见（04 §8 禁止静默失败）：降级与缺陷一律经 D3 落盘，
/// 日志不可用时回退 stderr。
///
/// 这也不是仪式性代码 —— 它是「Core + 内置数据 + 日志能在 Windows 上跑通」的最早信号。
/// 2026-09-20 起 Windows 开发机已装好 Rust 工具链，本地 `cargo check -p orbis` /
/// `cargo run -p orbis` 即可直接验证（此前只有 CI 能提供反馈）。
fn startup_self_check(logging_ok: bool) -> bool {
    // 1. 版本归一化契约：多段构建号必须归一到 major.minor（04 §5.1）
    let version_ok = orbis_core::normalize("3.5.0.128940")
        .map(|v| v.to_string() == "3.5")
        .unwrap_or(false);

    // 2. 内置数据装载：任一数据源损坏都不中断（各自降级，见 orbis_tools 模块文档）
    let data = orbis_tools::BuiltinData::load();

    // 3. platform 能报告当前目标平台
    let supported = orbis_platform::is_supported_target();

    for reason in &data.degradations {
        announce(LogLevel::Warn, reason, logging_ok);
    }
    for issue in data.notices() {
        let level = match issue.severity() {
            orbis_tools::Severity::Problem => LogLevel::Error,
            orbis_tools::Severity::Notice => LogLevel::Info,
        };
        announce(level, &issue.to_string(), logging_ok);
    }

    let core_ok = version_ok && supported;
    announce(
        if core_ok {
            LogLevel::Info
        } else {
            LogLevel::Error
        },
        &format!(
            "启动自检{}（种子条目 {}、工具 {}、资产 {}）",
            if core_ok { "通过" } else { "失败" },
            data.seed.entries().len(),
            data.manifests.len(),
            data.assets.entries().len(),
        ),
        logging_ok,
    );

    core_ok
}

/// 应用入口（由 `main.rs` 调用）。
pub fn run() {
    let logging_ok = init_logging();
    startup_self_check(logging_ok);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 第二实例不启动新的 Core，只把已有窗口前置后退出（04 §5.12）。
            // 若允许两个实例并存：会并发写同一份游戏配置、重复累计时长、
            // 并各自持有不一致的备份视图 —— UNIQUE 约束挡不住这类破坏。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![windowControl])
        .run(tauri::generate_context!())
        .expect("failed to run Orbis");
}
