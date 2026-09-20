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

/// 启动自检：校验 Core 契约可用，并**装载内置数据**。
///
/// 返回值只表达「Core 契约不可用」这类**硬失败**。数据降级**不阻止启动** ——
/// 02 C1（单个 Manifest 损坏）/ C4（种子表损坏）/ B7（资产未配置）都明确要求
/// 降级后仍可运行，但必须显式可见（04 §8 禁止静默失败）。D3 日志落地前，
/// 这里以 stderr 作为临时出口。
///
/// 这也不是仪式性代码 —— 它是「Core + 内置数据能在 Windows 上跑通」的最早信号。
/// 2026-09-20 起 Windows 开发机已装好 Rust 工具链，本地 `cargo check -p orbis` /
/// `cargo run -p orbis` 即可直接验证（此前只有 CI 能提供反馈）。
fn startup_self_check() -> bool {
    // 1. 版本归一化契约：多段构建号必须归一到 major.minor（04 §5.1）
    let version_ok = orbis_core::normalize("3.5.0.128940")
        .map(|v| v.to_string() == "3.5")
        .unwrap_or(false);

    // 2. 内置数据装载：任一数据源损坏都不中断（各自降级，见 orbis_tools 模块文档）
    let data = orbis_tools::BuiltinData::load();

    // 3. platform 能报告当前目标平台
    let supported = orbis_platform::is_supported_target();

    if data.has_problems() {
        eprintln!("orbis 内置数据存在缺陷（不阻止启动，但需修复）：");
        for issue in data
            .notices()
            .filter(|i| i.severity() == orbis_tools::Severity::Problem)
        {
            eprintln!("  ✗ {issue}");
        }
    }

    let core_ok = version_ok && supported;
    eprintln!(
        "orbis startup self-check {} (seed={}, manifests={}, assets={})",
        if core_ok { "ok" } else { "FAILED" },
        data.seed.entries().len(),
        data.manifests.len(),
        data.assets.entries().len(),
    );

    core_ok
}

/// 应用入口（由 `main.rs` 调用）。
pub fn run() {
    startup_self_check();

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
