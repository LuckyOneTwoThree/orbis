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
//! 已实现 13/28 条命令（契约 §3）：
//!
//! - 目录与工具：`listGames`（§3.1）、`listTools`（§3.6）、`getCompatibility`（§3.7）
//! - 安装实例与运行：`listInstallations` / `getInstallationDetail`（§3.1）、
//!   `removeInstallation`（§3.1）、`getRuntimeStates`（§3.2，A5）
//! - 启动参数：`getLaunchProfile` / `setLaunchProfile` / `resetLaunchProfile`（§3.3）
//! - 设置：`getSettings` / `setSetting`（§3.9）
//! - 壳层：`windowControl`（§3.10）
//!
//! 另有单实例互斥（04 §5.12）与启动引导（D3 日志落盘、内置数据装载、SQLite 建库与
//! 迁移、按设置清理过期日志）。
//!
//! 其余命令按 `docs/ipc-contract.md` §3 逐条实现，命名必须与契约一致（camelCase，
//! 见下方 allow 说明）。**不要**为了让命令数变多而实现没有数据来源的命令 ——
//! `getTool`（缺 L3 授权文案源）与 `scanGames` / `validateExecutable` /
//! `addInstallation`（缺实测项 T5/T7 的检测规则，见 `commands` 模块文档）都是这种情形。
//!
//! # 文件划分
//!
//! - [`commands`]：命令实现（取状态 → 调领域 crate → 投影 DTO）
//! - [`error`]：契约 §5 的错误码与 `OrbisError`（壳层对前端唯一的错误形状）
//! - [`dto`]：必须由壳层拼装的 DTO（需要同时看见 providers 与 tools 的那几个）

// 契约 §1 规定命令名为 camelCase（`windowControl` / `scanGames` / ...），
// 且 tauri-specta 会把 Rust 函数名直接镜像成 TS 侧的命令名。
// 因此这里刻意使用非 snake_case 命名，换取「契约、Rust 函数名、TS 调用名」三者一致。
#![allow(non_snake_case)]

mod commands;
mod dto;
mod error;

use commands::*;

use error::{internal, ErrorCode, OrbisError, OrbisResult};
use orbis_platform::db::Db;
use orbis_platform::log::{self, LogCategory, LogLevel, LogRecord, LogSource};
use orbis_platform::paths;
use orbis_tools::BuiltinData;
use tauri::{Emitter, Manager};

/// 窗口控制（契约 §3.10 / 03 §5.1 自绘标题栏）。
///
/// 发布包使用 `decorations: false`，窗口的最小化 / 最大化 / 关闭全部由界面按钮触发，
/// 因此这条命令是**可用性必需**而非可选增强。
///
/// 失败返回契约错误体而不是裸字符串：前端 `normalizeError` 靠 `'code' in raw` 判定
/// 契约错误（契约 §5「Tauri 侧约定」），返回字符串会被归成 `INTERNAL` 并丢掉
/// `detail` —— 整个命令面应该只有一种错误形状。
#[tauri::command]
fn windowControl(window: tauri::Window, action: String) -> OrbisResult<()> {
    match action.as_str() {
        "minimize" => window.minimize().map_err(window_error),
        "maximize" => {
            // 前端只有一个按钮，语义是「切换最大化」，避免按钮状态与实际窗口状态不一致
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize().map_err(window_error)
            } else {
                window.maximize().map_err(window_error)
            }
        }
        "close" => window.close().map_err(window_error),
        other => Err(
            OrbisError::new(ErrorCode::Internal, format!("未知窗口动作：{other}"))
                .with_detail("action", other.to_owned()),
        ),
    }
}

/// 窗口 API 失败一律 `INTERNAL`：这是窗口系统层面的问题，用户没有可执行的补救动作，
/// 因此不给「重试」按钮（契约 §5 的 `INTERNAL` 行）。
fn window_error(err: tauri::Error) -> OrbisError {
    internal(format!("窗口操作失败：{err}"))
}

/// 应用级事件的日志归属 —— **04 §11 Q10 未裁决前的临时映射**。
///
/// 00 §9.1 / 04 §5.11 的 `Source` 只有 `Game / Tool / Launcher / Backup / Update`，
/// 没有「应用自身」一档。受此影响的事件已从「内置数据装载」（勉强算工具域）
/// 扩大到**启动自检**与**数据库打开失败**——这两者与工具域无关。
///
/// 映射到这里已经明显勉强，Q10 应尽快裁决（倾向增补 `app`）。
/// 裁决后**只需改这一处**，这也是刻意把它们提成常量的原因。
const APP_EVENT_SOURCE: LogSource = LogSource::Tool;
const APP_EVENT_CATEGORY: LogCategory = LogCategory::Detect;

/// A5 运行态轮询间隔（04 §5.10：5s 周期，对应验收「5 秒内反映」）。
const RUNTIME_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// 契约 §4 的运行状态事件名（`域:动作` 口径）。
const RUNTIME_STATE_EVENT: &str = "game:state-changed";

/// 初始化 D3 日志：安装 JSONL subscriber。返回日志是否成功落盘。
///
/// 日志初始化失败**不阻止启动**（诊断能力缺失不该让产品不可用），但必须显式可见：
/// 调用方据返回值决定是否用 stderr 兜底。
fn init_logging() -> bool {
    let Some(dir) = paths::logs_dir() else {
        eprintln!("orbis: 无法解析应用数据目录 —— 本次日志不落盘");
        return false;
    };

    if let Err(err) = log::init(&dir) {
        eprintln!("orbis: 日志初始化失败 —— 本次日志不落盘：{err}");
        return false;
    }
    true
}

/// 按保留期清理过期日志（04 §5.11：整文件删除，不做行级裁剪）。
fn cleanup_logs(retention_days: u32, logging_ok: bool) {
    let Some(dir) = paths::logs_dir() else {
        return;
    };
    let report = log::cleanup_old_logs(&dir, retention_days);
    if !report.is_clean() {
        announce(
            LogLevel::Warn,
            &format!(
                "日志清理有 {} 项失败，其余照常：{:?}",
                report.failed.len(),
                report.failed
            ),
            logging_ok,
        );
    }
}

/// 打开数据库并迁移到当前 schema（04 §6.1）。
///
/// 打开失败**不阻止启动**，但这是一条 ERROR：没有数据库意味着本次运行不持久化任何
/// 状态（安装列表、时长、工具启停、备份记录全都没有落点）。
fn open_database(logging_ok: bool) -> Option<Db> {
    let Some(path) = paths::db_path() else {
        announce(
            LogLevel::Error,
            "无法解析应用数据目录 —— 本次不持久化任何状态",
            logging_ok,
        );
        return None;
    };

    match Db::open(&path) {
        Ok(db) => Some(db),
        Err(err) => {
            announce(
                LogLevel::Error,
                &format!("数据库不可用 —— 本次不持久化任何状态：{err}"),
                logging_ok,
            );
            None
        }
    }
}

/// 记录一条降级/缺陷信息：写日志；若日志未落盘则回退到 stderr。
///
/// 两步缺一不可 —— 只写日志会在「日志不可用」时静默丢掉降级，
/// 这正是 04 §8「禁止静默失败」要防的情形。
///
/// `pub(crate)`：命令期（[`commands::AppState`]）也要报降级，且必须走同一套
/// 「日志 + stderr 兜底」逻辑，否则同一种降级在启动期与运行期会有两种可见性。
pub(crate) fn announce(level: LogLevel, message: &str, logging_ok: bool) {
    LogRecord::new(APP_EVENT_SOURCE, APP_EVENT_CATEGORY, level, message).emit();
    if !logging_ok {
        eprintln!("orbis [{level}] {message}");
    }
}

/// 启动自检：校验 Core 契约、报告内置数据状态、并报告当前生效的存储版本。
///
/// 返回值只表达「Core 契约不可用」这类**硬失败**。数据降级与数据库不可用
/// **都不阻止启动** —— 02 C1（单个 Manifest 损坏）/ C4（种子表损坏）/ B7（资产未配置）
/// 都明确要求降级后仍可运行，但必须显式可见（04 §8 禁止静默失败）：
/// 一切降级一律经 D3 落盘，日志不可用时回退 stderr。
///
/// `data` 由调用方传入而不是在这里装载：同一份内置数据随后要移进命令状态
/// （[`commands::AppState`]），装载两次会得到两份独立的副本 —— 数据一样，
/// 但「启动自检报告的内容」与「命令实际服务的内容」就失去了同一性保证。
fn startup_self_check(logging_ok: bool, data: &BuiltinData, database: Option<&Db>) -> bool {
    // 1. 版本归一化契约：多段构建号必须归一到 major.minor（04 §5.1）
    let version_ok = orbis_core::normalize("3.5.0.128940")
        .map(|v| v.to_string() == "3.5")
        .unwrap_or(false);

    // 2. platform 能报告当前目标平台
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
    let storage = match database {
        Some(db) => format!(
            "schema v{}",
            db.schema_version()
                .map(|v| v.to_string())
                .unwrap_or_else(|e| format!("读取失败：{e}"))
        ),
        None => "不可用".to_owned(),
    };
    announce(
        if core_ok {
            LogLevel::Info
        } else {
            LogLevel::Error
        },
        &format!(
            "启动自检{}（种子条目 {}、工具 {}、资产 {}、数据库 {storage}）",
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

    let database = open_database(logging_ok);

    // 内置数据只装载一次（Manifest / 种子表 / 资产清单都要 `include_str!` + JSON 解析）。
    // 装载本身不产生降级副作用 —— 降级信息在下面的自检里统一落盘。
    let data = BuiltinData::load();

    // 日志保留天数来自 app_setting（04 §5.11：默认 14 天，可由设置覆盖）。
    // 读设置失败时回落默认值，但**不静默**——降级必须可见。
    let retention_days = match database.as_ref().map(Db::settings) {
        Some(Ok(settings)) => settings.log_retention_days,
        Some(Err(err)) => {
            announce(
                LogLevel::Error,
                &format!(
                    "设置读取失败，日志保留天数回落默认 {} 天：{err}",
                    log::DEFAULT_RETENTION_DAYS
                ),
                logging_ok,
            );
            log::DEFAULT_RETENTION_DAYS
        }
        None => log::DEFAULT_RETENTION_DAYS,
    };
    cleanup_logs(retention_days, logging_ok);

    startup_self_check(logging_ok, &data, database.as_ref());

    // `data` 与 `database` 从此归命令状态所有，生命周期与进程一致。
    // 不在自检后 drop 连接：连接本身是有状态资源（WAL / 事务 / FK 开关都是每连接的），
    // 频繁开关会不断重设这些状态。命令经 AppState 借用它，04 §5.12 的
    // 「按安装实例串行化」就架在 AppState 内部那把 Mutex 上。
    let state = AppState::new(data, database, logging_ok);

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
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            windowControl,
            listGames,
            listTools,
            getCompatibility,
            listInstallations,
            getInstallationDetail,
            removeInstallation,
            getRuntimeStates,
            getLaunchProfile,
            setLaunchProfile,
            resetLaunchProfile,
            getSettings,
            setSetting,
        ])
        .setup(|app| {
            // A5：进程快照轮询（04 §5.10：5s 周期，满足「5 秒内反映」）。
            // 用 std::thread 而不是 tokio —— 这里只需要一个定时循环，
            // 为它引入异步运行时不划算。
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(RUNTIME_POLL_INTERVAL);
                let state = handle.state::<AppState>();
                for payload in state.poll_runtime_changes() {
                    // 事件发送失败通常是「前端还没订阅 / 序列化失败」。
                    // 前者无害，后者是缺陷 —— 因此保留一条 stderr 而不是静默吞掉。
                    if let Err(err) = handle.emit(RUNTIME_STATE_EVENT, payload) {
                        eprintln!("orbis [warn] {RUNTIME_STATE_EVENT} 事件发送失败：{err}");
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Orbis");
}
