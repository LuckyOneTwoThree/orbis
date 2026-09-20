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

/// 启动自检：确认 4 个领域 crate 在目标平台上可链接、且核心契约可用。
///
/// 这不是仪式性代码 —— 它是「Core 能在 Windows 上编译」的最早信号。
/// 2026-09-20 起 Windows 开发机已装好 Rust 工具链，本地 `cargo check -p orbis`
/// 即可直接验证（此前只有 CI 能提供反馈）；一旦某个 crate 在 Windows 上编不过，
/// 这里会第一时间让它暴露。
fn startup_self_check() -> bool {
    // 1. 版本归一化契约：多段构建号必须归一到 major.minor（04 §5.1）
    let version_ok = orbis_core::normalize("3.5.0.128940")
        .map(|v| v.to_string() == "3.5")
        .unwrap_or(false);

    // 2. providers 的装配表与 tools 的 Manifest 加载器可访问
    //    （MVP 阶段两者都还是空骨架，这里只验证符号可链接）
    let providers_count = orbis_providers::catalog().len();
    let manifests_count = orbis_tools::load_builtin_manifests().len();

    // 3. platform 能报告当前目标平台
    let supported = orbis_platform::is_supported_target();

    if !version_ok || !supported {
        eprintln!(
            "orbis startup self-check FAILED (version_ok={version_ok}, supported={supported})"
        );
        return false;
    }

    // 骨架阶段数量为 0 属预期，不作为失败条件
    eprintln!(
        "orbis startup self-check ok (providers={providers_count}, manifests={manifests_count})"
    );
    true
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
