//! 命令面实现（`docs/ipc-contract.md` §3）—— 壳层唯一的业务入口。
//!
//! # 这里不做业务判断
//!
//! 每条命令只做三件事：**取状态 → 调领域 crate → 投影成 DTO**。门控（04 §5.5）、
//! 兼容判定（Core）、降级策略（领域 crate）都不在这里 —— 壳层做决策会让「同一规则
//! 在 UI 与后端各有一份」，这正是 00 §7.9 要消除的。
//!
//! # 为什么返回 `Vec` 而不是 `Result<Vec, _>`（`listGames` / `listTools` / `getCompatibility`）
//!
//! 契约 §3 只给这三条命令列了「无错误路径」：数据损坏时它们返回**降级结果**
//! （空表 / 全部 `unknown`）而不是错误（02 C1 / C4 验收要求「不崩溃、不阻塞」）。
//! 前端参照实现（`src/api/mock.ts`）同样不抛 —— 后端若改成抛错，UI 会把
//! 「数据未定稿」渲染成一片红色错误，把降级变成了故障。
//!
//! # 已实现 / 未实现
//!
//! 已实现：`listGames`、`listTools`、`getCompatibility`、`listInstallations`、
//! `getInstallationDetail`、`removeInstallation`、`getRuntimeStates`、`getPlaytime`、
//! `getLaunchProfile`、`setLaunchProfile`、`resetLaunchProfile`、
//! `getSettings`、`setSetting`、`windowControl`。
//!
//! **不复述数量**：写在注释里的进度数字必然 stale。权威计数由 `npm run check:commands`
//! 从契约 §3、`src/api/tauri.ts` 的接线与壳层注册表现算并断言。
//!
//! 两条**刻意未实现**的命令，理由都是「缺数据来源」而不是「来不及」：
//!
//! - **`getTool`**：`consentText`（L3 版本化授权全文）没有任何数据来源 —— 契约 §7.3
//!   说它来自「Manifest / 资源内的版本化文案」，而 `manifests/*.json` 里没有这个字段，
//!   `consentTextHash` 也就无从计算。先返回 `consentText: null` 是危险的：UI 会据此
//!   认为「这个工具不需要授权」，而 L3 的授权门槛是 00 §12.3 规则 7 的硬约束。
//!   等文案源定稿（04 §7.2）再落地。
//! - **`scanGames` / `validateExecutable` / `addInstallation`**：三者都依赖「这个 exe
//!   是游戏本体还是官方启动器」这类**按游戏判定**的规则（`DetectRule` / `LaunchSpec`），
//!   而它们的取值被实测项 T5/T7 阻塞（需要目标机器上真的装着游戏才能收敛）。
//!   现在实现必然要凭推测填常量，于是会**静默接受用户选错的 exe**（02 A2 边界明确
//!   禁止「不静默接受」）。注意 `addInstallation` 的路径 / 续重 / 可执行性检查本身
//!   并不需要实测 —— 缺的只是「选到启动器」这一关，所以整条命令一起等。
//!
//! 与它们相对，本文件已落地的安装实例命令只依赖**数据库与已定稿的规则**，因此可以先做。

use std::collections::HashMap;
use std::sync::Mutex;

use orbis_core::Version;
use orbis_platform::db::{Db, SettingKey, SettingValue};
use orbis_platform::installation::InstallationRecord;
use orbis_platform::log::LogLevel;
use orbis_platform::playtime::{self, PlaytimeTracker, TrackedEvent};
use orbis_platform::process::{KillOutcome, ProcessSnapshot};
use orbis_tools::{BuiltinData, CompatibilityDto, ToolDto};
use serde_json::Value;
use tauri::State;

use crate::dto::{
    backup_summary_dto, game_catalog_entries, installation_dto, installation_list,
    launch_profile_dto, runtime_state_dto, state_changed_payload, AppSettingsDto,
    BackupStorageInfoDto, BackupSummaryDto, GameCatalogEntryDto, GameStateChangedPayload,
    InstallationDetailDto, InstallationListResultDto, LaunchProfileDto, PlaytimeDto,
    PlaytimePerInstallationDto, PlaytimeResultDto, RuntimeState, RuntimeStateDto,
};
use crate::error::{internal, ErrorCode, OrbisError, OrbisResult};

/// 命令共享状态。
///
/// 内置数据一次性装载后**只读共享**；数据库用 `Mutex` 包住 —— `rusqlite::Connection`
/// 是 `Send` 但**不是** `Sync`，而 04 §5.12 的「按安装实例串行化」正需要一把锁。
/// 用 `Option` 而不是直接持有 `Db`：数据库打不开是**已设计**的降级态（启动自检里
/// 那条 ERROR），类型上必须能表达「本次运行没有数据库」，否则只剩 panic 一条路。
pub struct AppState {
    data: BuiltinData,
    db: Mutex<Option<Db>>,
    /// 日志是否成功落盘 —— 命令期的降级信息据此决定要不要回退 stderr。
    logging_ok: bool,
    /// 上一次的运行态。`None` = 还没建立基线 —— 首帧数据由 `getRuntimeStates` 拉取，
    /// 事件只负责增量（契约 §4），因此首次轮询不该把「全部实例」当成「全部变化」。
    last_runtime: Mutex<Option<Vec<RuntimeState>>>,
    /// 进行中的时长会话（A6）。**内存态是有意的**：进程是否在跑只有当前进程知道，
    /// 库里的会话行只是崩溃恢复的检查点（见 `orbis_platform::playtime`）。
    playtime: Mutex<PlaytimeTracker>,
}

impl AppState {
    pub fn new(data: BuiltinData, db: Option<Db>, logging_ok: bool) -> Self {
        Self {
            data,
            db: Mutex::new(db),
            logging_ok,
            last_runtime: Mutex::new(None),
            playtime: Mutex::new(PlaytimeTracker::new()),
        }
    }

    pub fn data(&self) -> &BuiltinData {
        &self.data
    }

    /// 记录一条降级信息（走 D3 日志；日志不可用时回退 stderr）。
    fn announce(&self, level: LogLevel, message: &str) {
        crate::announce(level, message, self.logging_ok);
    }

    /// 借用数据库执行一段逻辑。数据库不可用 → `INTERNAL`（**不**静默返回默认值：
    /// 让 UI 明确知道「本次运行的设置不会被记住」，比给它一份假的设置诚实）。
    fn with_db<T>(&self, use_db: impl FnOnce(&Db) -> OrbisResult<T>) -> OrbisResult<T> {
        let guard = self
            .db
            .lock()
            .map_err(|_| internal("数据库互斥锁已中毒（持锁线程在持有期间 panic）"))?;
        match guard.as_ref() {
            Some(db) => use_db(db),
            None => Err(internal("数据库不可用 —— 本次运行不持久化任何状态")
                .with_detail("reason", "db_unavailable")),
        }
    }

    /// 工具启停查询（**不自锁**）—— 供已经持有 `db` 锁的调用点使用。
    ///
    /// 这两个函数必须分开存在，原因是 `std::sync::Mutex` **不可重入**：若在持锁期间
    /// 再调 [`Self::tool_enabled`]，同一线程会永久阻塞在第二次 `lock()` 上，
    /// 而 guard 永不释放 → 5s 轮询线程与**全部** `with_db` 命令排队 → 整个后端冻结。
    ///
    /// 所以约定是：**闭包内用这个（`db` 由调用方保证），闭包外用 [`Self::tool_enabled`]**。
    /// 这条约束类型系统表达不了，因此配了回归测试
    /// （`installation_detail_does_not_deadlock_on_its_own_lock`）来兜住。
    ///
    /// 读取失败 → `false` 并留痕：静默把「启用中」显示成「已关闭」会让用户以为
    /// 自己的设置丢了（04 §8 禁止静默失败）。
    fn tool_enabled_with(&self, db: &Db, tool_id: &str) -> bool {
        match db.is_tool_enabled(tool_id) {
            Ok(enabled) => enabled,
            Err(err) => {
                self.announce(
                    LogLevel::Warn,
                    &format!("工具 {tool_id} 的启停状态读取失败，按未启用处理：{err}"),
                );
                false
            }
        }
    }

    /// 工具启停查询（**自锁**）—— 供未持有 `db` 锁的调用点使用。
    ///
    /// DB 不可用 → 一律 `false`：`listTools` 是首页刷新路径，为它抛错会让整个工具
    /// 面板变成错误页，而「数据库打不开」这件事已经在启动自检里记了 ERROR。
    fn tool_enabled(&self, tool_id: &str) -> bool {
        let Ok(guard) = self.db.lock() else {
            self.announce(LogLevel::Warn, "工具状态查询失败：数据库锁中毒");
            return false;
        };
        guard
            .as_ref()
            .is_some_and(|db| self.tool_enabled_with(db, tool_id))
    }

    /// 某款游戏**最近添加的实例**的归一化版本（契约 §3.7 的本地版本口径）。
    ///
    /// 读取失败 / 没有实例 / 版本未知 → `None`。这三种情况在契约 §3.7 里是**同一个含义**
    /// （「无安装实例或版本未知」→ 落到 `unknown`，即默认安全态），
    /// 因此这里可以降级而不必让 `getCompatibility` 变成会抛错的命令 ——
    /// 故障与「没有安装」在结果上不可区分，正是契约对该命令的规定。
    /// 但降级仍然留痕（04 §8）。
    fn latest_version_norm(&self, game_id: &str) -> Option<Version> {
        let Ok(guard) = self.db.lock() else {
            self.announce(LogLevel::Warn, "读取本地版本失败：数据库锁中毒");
            return None;
        };
        let db = guard.as_ref()?;
        match db.latest_installation_for_game(game_id) {
            Ok(record) => record.and_then(|r| r.version_norm),
            Err(err) => {
                self.announce(
                    LogLevel::Warn,
                    &format!("读取 {game_id} 的本地版本失败，按版本未知处理：{err}"),
                );
                None
            }
        }
    }

    /// 这批实例此刻的运行态（进程快照现算，04 §5.10）。
    pub(crate) fn runtime_states(&self, records: &[InstallationRecord]) -> Vec<RuntimeState> {
        crate::dto::runtime_states(records, &ProcessSnapshot::capture())
    }

    /// 备份目录所在卷的可用字节数。无法判定 → `0` 并留痕。
    ///
    /// 契约 §6 的 `freeDiskBytes` 是**非空**数值，所以只能给 `0`、给不了 null ——
    /// 因此留痕是必须的：`0` 会被 UI 读成「磁盘满了」，一声不响地降级等于报了个假警。
    fn backup_disk_free_bytes(&self) -> u64 {
        let Some(dir) = orbis_platform::paths::data_dir() else {
            self.announce(
                LogLevel::Warn,
                "无法解析应用数据目录，备份可用空间按 0 报告",
            );
            return 0;
        };
        match orbis_platform::backup::free_disk_bytes(&dir) {
            Some(bytes) => bytes,
            None => {
                self.announce(
                    LogLevel::Warn,
                    &format!("无法判定 {} 所在卷的可用空间，按 0 报告", dir.display()),
                );
                0
            }
        }
    }

    /// 轮询一次运行态，返回**发生变化**的实例（供 `game:state-changed` 事件）。
    ///
    /// 读不到实例列表时返回空并留痕 —— 此时「不知道」比「谎报未运行」安全：
    /// 把正在跑的游戏显示成已停止，会让用户以为时长统计或工具状态出了问题。
    pub(crate) fn poll_runtime_changes(&self) -> Vec<GameStateChangedPayload> {
        let Ok(guard) = self.db.lock() else {
            self.announce(LogLevel::Warn, "运行态轮询跳过：数据库锁中毒");
            return Vec::new();
        };
        let Some(db) = guard.as_ref() else {
            return Vec::new();
        };
        let records = match db.installations() {
            Ok(records) => records,
            Err(err) => {
                self.announce(LogLevel::Warn, &format!("运行态轮询跳过：{err}"));
                return Vec::new();
            }
        };

        let current = self.runtime_states(&records);

        // A6：用**同一份**运行态推进时长会话。分成两次快照会让「进程刚退出」
        // 被记成两段时长（一次关旧会话、一次开新会话）。
        self.observe_playtime(db, &records, &current);

        let Ok(mut previous) = self.last_runtime.lock() else {
            self.announce(LogLevel::Warn, "运行态轮询跳过：上次状态锁中毒");
            return Vec::new();
        };

        let mut changed = Vec::new();
        if let Some(before_states) = previous.as_ref() {
            for (state, record) in current.iter().zip(&records) {
                let before = before_states
                    .iter()
                    .find(|p| p.installation_id == state.installation_id);
                let changed_now = match before {
                    Some(prev) => prev.status != state.status || prev.pid != state.pid,
                    None => true, // 新出现的实例
                };
                if !changed_now {
                    continue;
                }
                // 进程消失（含崩溃）必须留痕：04 §5.10 明确要求「状态回落 + 写日志」，
                // 静默回落会让「游戏崩了」这件事无从追溯
                if let Some(prev) = before.filter(|p| p.pid.is_some()) {
                    if state.pid.is_none() {
                        self.announce(
                            LogLevel::Info,
                            &format!(
                                "{} 的进程已退出（pid {:?}），状态回落 installed",
                                record.game_id, prev.pid
                            ),
                        );
                    }
                }
                changed.push(state_changed_payload(state, record));
            }
        }

        *previous = Some(current);
        changed
    }

    /// 推进时长会话（A6）。
    ///
    /// 失败只记日志、不影响运行态事件：时长是「附加值」，让它拖垮状态刷新得不偿失。
    fn observe_playtime(&self, db: &Db, records: &[InstallationRecord], runtime: &[RuntimeState]) {
        // 读设置失败就回落文档默认值（`settings()` 在未设置时本来就返回默认值，
        // 走到这里说明库有问题 —— 但那不该让时长记录整段停摆）
        let checkpoint_sec = db
            .settings()
            .map(|settings| settings.playtime_checkpoint_sec)
            .unwrap_or(orbis_platform::db::DEFAULT_PLAYTIME_CHECKPOINT_SEC);

        let Ok(mut tracker) = self.playtime.lock() else {
            self.announce(LogLevel::Warn, "时长记录跳过：会话状态锁中毒");
            return;
        };

        let is_running = |id: &str| {
            runtime
                .iter()
                .any(|state| state.installation_id == id && state.pid.is_some())
        };

        match tracker.observe(records, is_running, db, epoch_millis(), checkpoint_sec) {
            Ok(events) => {
                for event in events {
                    let (installation_id, message) = match &event {
                        TrackedEvent::Started {
                            installation_id, ..
                        } => (installation_id, "开始记录时长".to_owned()),
                        TrackedEvent::Ended {
                            installation_id,
                            duration_sec,
                            ..
                        } => (installation_id, format!("本次时长已记录 {duration_sec} 秒")),
                    };
                    let game = records
                        .iter()
                        .find(|record| &record.id == installation_id)
                        .map(|record| record.game_id.clone())
                        .unwrap_or_else(|| installation_id.clone());
                    self.announce(LogLevel::Info, &format!("{game}：{message}"));
                }
            }
            Err(err) => self.announce(LogLevel::Warn, &format!("时长记录失败：{err}")),
        }
    }
}

// ── 游戏目录（契约 §3.1）────────────────────────────────────

/// 编译期静态游戏目录。UI 取得游戏显示名的**唯一**途径（架构断言 [8]）。
#[tauri::command]
pub fn listGames(state: State<'_, AppState>) -> Vec<GameCatalogEntryDto> {
    game_catalog_entries(state.data())
}

// ── 工具（契约 §3.6 / §3.7）─────────────────────────────────

/// 工具列表。`gameId` 省略 = 全部。
///
/// 未知 `gameId` **不报错**而是返回空列表：与前端参照实现一致，且「某游戏没有工具」
/// 与「游戏标识不存在」对 UI 是同一种表现（空态），区分它们只会制造一个没有
/// 行动建议的错误。
#[tauri::command]
pub fn listTools(state: State<'_, AppState>, gameId: Option<String>) -> Vec<ToolDto> {
    let is_enabled = |tool_id: &str| state.tool_enabled(tool_id);
    orbis_tools::list_tools(state.data(), gameId.as_deref(), &is_enabled)
}

/// 单个「游戏 × 工具」的兼容性。
///
/// 本地版本按契约 §3.7 取自**该游戏最近添加的实例**的 `version_norm`；
/// 没有实例 / 版本未知 → `unknown` + `matchKind: 'none'`（02 A3：不猜版本）。
#[tauri::command]
pub fn getCompatibility(
    state: State<'_, AppState>,
    gameId: String,
    toolId: String,
) -> CompatibilityDto {
    let local = state.latest_version_norm(&gameId);
    orbis_tools::compatibility(&state.data().seed, &gameId, &toolId, local)
}

// ── 安装实例（契约 §3.1 / §3.3）──────────────────────────────

/// 全部安装实例 + 首页摘要（契约 §3.1）。
///
/// 与 `listGames` 不同，这条命令**会失败**：它的内容全部来自数据库，
/// 数据库不可用时「返回空列表」会被读成「一台游戏都没装」—— 那是把故障伪装成事实。
/// 因此这里选择显式报错，而不是给出一份看起来正常的空列表。
#[tauri::command]
pub fn listInstallations(state: State<'_, AppState>) -> OrbisResult<InstallationListResultDto> {
    state.with_db(|db| {
        let records = db.installations()?;
        // 顺手做一次进程快照：列表页要显示「运行中」徽标与 pid，
        // 让 UI 再调一次 getRuntimeStates 会多一次全表进程枚举
        let runtime = state.runtime_states(&records);
        let playtime = playtime_totals(db, epoch_millis())?;
        Ok(installation_list(
            state.data(),
            records,
            &runtime,
            &playtime,
        ))
    })
}

/// 时长查询（契约 §3.4 / A6）。
///
/// 口径 = **游戏进程存活时长**（02 A6 边界，UI 必须明示这一口径）；
/// `today` / `week` 按本地时区自然日界，「本周」起点 = 周一（04 §5.10）。
#[tauri::command]
pub fn getPlaytime(
    state: State<'_, AppState>,
    scope: String,
    installationId: Option<String>,
) -> OrbisResult<PlaytimeResultDto> {
    let now = epoch_millis();
    let (scope_slug, since) = match scope.as_str() {
        "today" => ("today", Some(playtime::local_day_start_ms(now))),
        "week" => ("week", Some(playtime::local_week_start_ms(now))),
        "total" => ("total", None),
        // 非法 scope 是契约违例（前端只会传这三个字面量），归 INTERNAL：
        // 它不是用户能处理的错误，也没有对应的 §5 错误码
        other => {
            return Err(internal(format!("未知的时长 scope：{other:?}"))
                .with_detail("scope", other.to_owned()))
        }
    };

    state.with_db(|db| {
        let rows = playtime::playtime_rows(db, since, now, installationId.as_deref())?;
        Ok(PlaytimeResultDto {
            scope: scope_slug,
            total_sec: rows.iter().map(|row| row.seconds).sum(),
            per_installation: rows
                .into_iter()
                .map(|row| PlaytimePerInstallationDto {
                    installation_id: row.installation_id,
                    game_id: row.game_id,
                    seconds: row.seconds,
                })
                .collect(),
        })
    })
}

/// 每个实例的三档时长（供 `listInstallations` 的 `playtime` 字段）。
///
/// 三次聚合（今日 / 本周 / 全部）而不是一次 SQL：三者的窗口不同，且区间重叠
/// 计算已在 `playtime_rows` 里做过一次，这里只是把结果按实例摆好。
fn playtime_totals(db: &Db, now: i64) -> OrbisResult<HashMap<String, PlaytimeDto>> {
    let today = playtime::playtime_rows(db, Some(playtime::local_day_start_ms(now)), now, None)?;
    let week = playtime::playtime_rows(db, Some(playtime::local_week_start_ms(now)), now, None)?;
    let total = playtime::playtime_rows(db, None, now, None)?;

    let mut totals: HashMap<String, PlaytimeDto> = HashMap::new();
    for row in &total {
        totals.insert(
            row.installation_id.clone(),
            PlaytimeDto {
                today_sec: 0,
                week_sec: 0,
                total_sec: row.seconds,
            },
        );
    }
    for row in &week {
        if let Some(entry) = totals.get_mut(&row.installation_id) {
            entry.week_sec = row.seconds;
        }
    }
    for row in &today {
        if let Some(entry) = totals.get_mut(&row.installation_id) {
            entry.today_sec = row.seconds;
        }
    }
    Ok(totals)
}

/// 运行状态快照（契约 §3.2 / A5）。
///
/// 这是**首次拉取**用的；之后的刷新走 `game:state-changed` 事件增量驱动
/// （04 §5.10：5s 轮询，满足 A5「5 秒内反映」）。
#[tauri::command]
pub fn getRuntimeStates(state: State<'_, AppState>) -> OrbisResult<Vec<RuntimeStateDto>> {
    state.with_db(|db| {
        let records = db.installations()?;
        let runtime = state.runtime_states(&records);
        Ok(runtime
            .iter()
            .zip(&records)
            .map(|(state, record)| runtime_state_dto(state, record))
            .collect())
    })
}

/// 移除实例：只删条目与管理数据，**不碰游戏文件与存档**（02 A2 验收）。
///
/// 从属的启动参数 / 时长会话 / 备份记录经外键级联删除（04 §6.1）；
/// 备份**文件**是否连带清理属于 A8 的范围，未落地前不存在备份文件。
#[tauri::command]
pub fn removeInstallation(state: State<'_, AppState>, installationId: String) -> OrbisResult<()> {
    state.with_db(|db| {
        if db.delete_installation(&installationId)? {
            Ok(())
        } else {
            Err(
                OrbisError::new(ErrorCode::GameNotFound, "安装实例不存在（可能已被移除）")
                    .with_detail("installationId", installationId.clone()),
            )
        }
    })
}

/// 单个实例的详情（契约 §3.1）。
///
/// `latestBackup` 取该实例最近一条备份。备份的**写入**路径（A8 的 `createBackup`）尚未
/// 落地，所以它当前实际总是 `null` —— 但那是「这个实例还没有备份」这个事实本身，
/// 不是占位值。
#[tauri::command]
pub fn getInstallationDetail(
    state: State<'_, AppState>,
    installationId: String,
) -> OrbisResult<InstallationDetailDto> {
    installation_detail(state.inner(), &installationId)
}

/// [`getInstallationDetail`] 的命令体，接收 `&AppState` 以便被单测直接调用。
///
/// 抽出这一层不是风格问题：`State<'_, AppState>` 在单测里构造不出来，命令体若只存在于
/// `#[tauri::command]` 函数内部，就**永远进不了测试** —— 而下面这条死锁正是这样藏了很久。
fn installation_detail(
    state: &AppState,
    installation_id: &str,
) -> OrbisResult<InstallationDetailDto> {
    state.with_db(|db| {
        let record = require_installation(db, installation_id)?;
        let runtime = state.runtime_states(std::slice::from_ref(&record));
        // 持锁点必须用**不自锁**的读取：`tool_enabled` 会再次 `self.db.lock()`，
        // 而 `std::sync::Mutex` 不可重入 → 同一线程永久阻塞、guard 永不释放。
        let is_enabled = |tool_id: &str| state.tool_enabled_with(db, tool_id);
        let playtime = playtime_totals(db, epoch_millis())?
            .get(&record.id)
            .copied()
            .unwrap_or_default();
        Ok(InstallationDetailDto {
            installation: installation_dto(
                state.data(),
                &record,
                runtime.first().and_then(|rt| rt.pid),
                playtime,
            ),
            tools: orbis_tools::list_tools(state.data(), Some(&record.game_id), &is_enabled),
            // 最近一条备份（§6.1 的索引 `idx_backup_install` 即按 created_at DESC 排序）
            latest_backup: db.backups(installation_id)?.first().map(backup_summary_dto),
            launch_profile: launch_profile_dto(&record, db.launch_profile(installation_id)?),
        })
    })
}

/// 读取启动参数（A7）。从未设置过 → `args` 为空串。
#[tauri::command]
pub fn getLaunchProfile(
    state: State<'_, AppState>,
    installationId: String,
) -> OrbisResult<LaunchProfileDto> {
    state.with_db(|db| {
        let record = require_installation(db, &installationId)?;
        Ok(launch_profile_dto(
            &record,
            db.launch_profile(&installationId)?,
        ))
    })
}

/// 保存启动参数（A7）。参数按游戏独立保存；冲突检测不做（02 A7 边界：P1）。
#[tauri::command]
pub fn setLaunchProfile(
    state: State<'_, AppState>,
    installationId: String,
    args: String,
) -> OrbisResult<LaunchProfileDto> {
    state.with_db(|db| {
        let record = require_installation(db, &installationId)?;
        db.set_launch_profile(&installationId, &args, epoch_millis())?;
        Ok(launch_profile_dto(
            &record,
            db.launch_profile(&installationId)?,
        ))
    })
}

/// 清除自定义启动参数（回到默认）。
///
/// 幂等：本来就没有参数也算成功 —— 用户点「恢复默认」的意图是「结果要默认」，
/// 而不是「必须删掉某一行」。
#[tauri::command]
pub fn resetLaunchProfile(state: State<'_, AppState>, installationId: String) -> OrbisResult<()> {
    state.with_db(|db| {
        require_installation(db, &installationId)?;
        db.reset_launch_profile(&installationId)?;
        Ok(())
    })
}

// ── 运行控制（契约 §3.2）──────────────────────────────────

/// 终止游戏进程（A4）。
///
/// 契约 §3.2 明确「数据损坏风险提示 + 确认属 **UI 责任**，Core 只执行」——
/// 所以这里不做任何二次确认，只把动作做掉。
///
/// 「进程已经不在」**不报错**：用户要的是「游戏关掉了」这个状态，而它已经达成；
/// 为此报错只会让 UI 弹一个没有意义的失败。真正需要让用户知道的是**权限被拒** ——
/// 那时游戏还在跑，静默返回成功等于骗人（04 §8）。
#[tauri::command]
pub fn terminateGame(state: State<'_, AppState>, installationId: String) -> OrbisResult<()> {
    terminate_game(state.inner(), &installationId)
}

fn terminate_game(state: &AppState, installation_id: &str) -> OrbisResult<()> {
    let record = state.with_db(|db| require_installation(db, installation_id))?;
    let pid = state
        .runtime_states(std::slice::from_ref(&record))
        .first()
        .and_then(|runtime| runtime.pid);
    let Some(pid) = pid else {
        return Err(
            OrbisError::new(ErrorCode::GameNotRunning, "该实例当前没有在运行")
                .with_detail("installationId", installation_id.to_owned()),
        );
    };

    // 交给 platform 做「校验 exe 再终止」：pid 会被系统回收复用，单凭 pid 终止可能杀错进程
    match orbis_platform::process::kill(pid, &record.executable_path) {
        KillOutcome::Signalled | KillOutcome::Gone => {
            state.announce(
                LogLevel::Info,
                &format!("已终止 {}（pid {pid}）", record.game_id),
            );
            Ok(())
        }
        KillOutcome::Denied => {
            state.announce(
                LogLevel::Warn,
                &format!("终止 {} 失败（pid {pid}）：权限不足", record.game_id),
            );
            Err(
                internal("终止进程被拒绝（通常是权限不足：游戏可能以管理员身份运行）")
                    .with_detail("installationId", installation_id.to_owned()),
            )
        }
    }
}

// ── 备份（契约 §3.8）──────────────────────────────────────
//
// 写入路径（`createBackup` / `restoreBackup`）**刻意未落地**：它们要展开 provider 声明的
// `declared_paths`（「该备份哪些文件」），而那属于实测项 T3/T6 的范围。凭推测填出的文件
// 清单会**备份不到该备份的东西** —— 用户以为有备份，这比没有备份更危险。
// 读路径与删除不依赖它，因此先落地。

/// 某实例的备份列表，最近的在前。没有备份 → 空列表（不是错误）。
#[tauri::command]
pub fn listBackups(
    state: State<'_, AppState>,
    installationId: String,
) -> OrbisResult<Vec<BackupSummaryDto>> {
    list_backups(state.inner(), &installationId)
}

fn list_backups(state: &AppState, installation_id: &str) -> OrbisResult<Vec<BackupSummaryDto>> {
    state.with_db(|db| {
        Ok(db
            .backups(installation_id)?
            .iter()
            .map(backup_summary_dto)
            .collect())
    })
}

/// 备份占用与磁盘余量。
#[tauri::command]
pub fn getBackupStorageInfo(
    state: State<'_, AppState>,
    installationId: String,
) -> OrbisResult<BackupStorageInfoDto> {
    backup_storage_info(state.inner(), &installationId)
}

fn backup_storage_info(
    state: &AppState,
    installation_id: &str,
) -> OrbisResult<BackupStorageInfoDto> {
    state.with_db(|db| {
        let (backup_count, total_bytes) = db.backup_totals(installation_id)?;
        Ok(BackupStorageInfoDto {
            installation_id: installation_id.to_owned(),
            backup_count,
            total_bytes,
            free_disk_bytes: state.backup_disk_free_bytes(),
            // 源目录大小要展开 `declared_paths` 才估得出 —— 同属 T3/T6，故为 null。
            // 契约允许「无法估算」；用 0 会被读成「下一个备份不占空间」。
            estimated_next_size_bytes: None,
        })
    })
}

/// 删除一个备份（记录 + 文件）。破坏性操作，UI 需二次确认（契约 §3.8）。
#[tauri::command]
pub fn deleteBackup(state: State<'_, AppState>, backupId: String) -> OrbisResult<()> {
    delete_backup(state.inner(), &backupId)
}

fn delete_backup(state: &AppState, backup_id: &str) -> OrbisResult<()> {
    state.with_db(|db| {
        let record = db.backup(backup_id)?.ok_or_else(|| {
            OrbisError::new(ErrorCode::BackupNotFound, "备份不存在（可能已被删除）")
                .with_detail("backupId", backup_id.to_owned())
        })?;

        // **先删文件、后删记录**：反序的话，一旦文件删除失败，记录已经没了，
        // 重试只会得到 BACKUP_NOT_FOUND —— 那些文件就永远留在磁盘上，且不再出现在任何列表里。
        if let Some(dir) = orbis_platform::paths::backup_dir(&record.installation_id, &record.id) {
            orbis_platform::backup::remove_dir_if_exists(&dir).map_err(|err| {
                state.announce(
                    LogLevel::Warn,
                    &format!("删除备份 {backup_id} 的文件失败：{err}"),
                );
                internal(format!("删除备份文件失败：{err}"))
                    .with_detail("backupId", backup_id.to_owned())
            })?;
        }

        // 删不到行说明已被并发删除 —— 目标状态已达成，不报错
        let _ = db.delete_backup_row(backup_id)?;
        state.announce(LogLevel::Info, &format!("已删除备份 {backup_id}"));
        Ok(())
    })
}

/// 取实例，不存在 → `GAME_NOT_FOUND`（契约 §5：`installationId` 不存在）。
///
/// 三道启动参数命令都要先确认实例存在：不做这一步的话，不存在的 id 会撞外键约束，
/// 于是 `GAME_NOT_FOUND` 变成 `INTERNAL` —— UI 从「刷新列表」变成「导出日志」。
fn require_installation(db: &Db, installation_id: &str) -> OrbisResult<InstallationRecord> {
    db.installation(installation_id)?.ok_or_else(|| {
        OrbisError::new(ErrorCode::GameNotFound, "安装实例不存在")
            .with_detail("installationId", installation_id.to_owned())
    })
}

/// 当前时间：Unix epoch **毫秒**（契约 §1）。
///
/// 系统时钟早于 1970 时退化为 0 而不是 panic：一个错误的时间戳不该让
/// 「保存启动参数」失败。
fn epoch_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

// ── 设置（契约 §3.9）───────────────────────────────────────

#[tauri::command]
pub fn getSettings(state: State<'_, AppState>) -> OrbisResult<AppSettingsDto> {
    state.with_db(|db| Ok(db.settings()?.into()))
}

/// 写入设置项并返回**写入后的全部设置**（契约 §3.9）。
///
/// 只有白名单里的三个键存在（04 §7.4），且取值范围由 platform 校验 ——
/// 这里刻意**不重复**实现一遍区间判断：两份校验必然漂移。
#[tauri::command]
pub fn setSetting(
    state: State<'_, AppState>,
    key: String,
    value: Value,
) -> OrbisResult<AppSettingsDto> {
    let setting_key = SettingKey::from_slug(&key).ok_or_else(|| {
        OrbisError::new(
            ErrorCode::SettingUnknownKey,
            format!("非白名单设置键：{key:?}"),
        )
        .with_detail("key", key.clone())
    })?;

    let setting_value = parse_setting_value(setting_key, &value)?;

    state.with_db(|db| {
        db.set_setting(setting_key, setting_value)?;
        Ok(db.settings()?.into())
    })
}

/// `bool | number` → 类型化的设置值。
///
/// 前端只能传布尔或数字（契约 §3.9）；传字符串 / 小数 / 负数属于契约违例，
/// 一律 `SETTING_INVALID_VALUE`。区间检查**不在这里**（见 `setSetting` 注释）。
fn parse_setting_value(key: SettingKey, raw: &Value) -> OrbisResult<SettingValue> {
    let mismatch = || {
        OrbisError::new(
            ErrorCode::SettingInvalidValue,
            format!("设置项 {} 的取值类型不符：{raw}", key.slug()),
        )
        .with_detail("key", key.slug())
        .with_detail("value", raw.to_string())
    };

    match (key, raw) {
        (SettingKey::VersionCheckEnabled, Value::Bool(flag)) => Ok(SettingValue::Bool(*flag)),
        (SettingKey::VersionCheckEnabled, _) => Err(mismatch()),
        (_, Value::Number(number)) => {
            let int = number.as_i64().ok_or_else(mismatch)?;
            u32::try_from(int)
                .map(SettingValue::U32)
                .map_err(|_| mismatch())
        }
        (_, _) => Err(mismatch()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(db: Option<Db>) -> AppState {
        AppState::new(BuiltinData::load(), db, false)
    }

    /// 读当前设置（显式标注返回类型：`into()` 的目标类型无法从闭包推断出来）。
    fn read_settings(state: &AppState) -> OrbisResult<AppSettingsDto> {
        state.with_db(|db| -> OrbisResult<AppSettingsDto> { Ok(db.settings()?.into()) })
    }

    #[test]
    fn settings_are_unavailable_without_a_database() {
        // 降级态必须**显式失败**，不能返回一份「看起来正常」的默认设置：
        // 否则用户改了保留天数、界面显示成功，重启后全部丢失
        let err = read_settings(&state_with(None)).expect_err("没有数据库时应报错");
        assert_eq!(err.code(), ErrorCode::Internal);
        assert!(!err.retryable());
    }

    #[test]
    fn settings_roundtrip_through_the_state() {
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        assert_eq!(
            read_settings(&state).unwrap(),
            AppSettingsDto {
                version_check_enabled: true,
                log_retention_days: 14,
                playtime_checkpoint_sec: 30,
            }
        );

        state
            .with_db(|db| -> OrbisResult<()> {
                db.set_setting(SettingKey::LogRetentionDays, SettingValue::U32(7))?;
                Ok(())
            })
            .unwrap();
        assert_eq!(read_settings(&state).unwrap().log_retention_days, 7);
    }

    #[test]
    fn tool_state_is_false_without_a_database_and_reads_the_flag_with_one() {
        let no_db = state_with(None);
        assert!(!no_db.tool_enabled("sample-ns/sample-tool"));

        let with_db = state_with(Some(Db::open_in_memory().unwrap()));
        assert!(!with_db.tool_enabled("sample-ns/sample-tool"), "默认未启用");
        with_db
            .with_db(|db| -> OrbisResult<()> {
                db.conn()
                    .execute(
                        "INSERT INTO tool_state(tool_id, enabled, updated_at) VALUES (?1, 1, 0)",
                        ["sample-ns/sample-tool"],
                    )
                    .expect("写入工具状态应成功");
                Ok(())
            })
            .unwrap();
        assert!(with_db.tool_enabled("sample-ns/sample-tool"));
    }

    #[test]
    fn setting_value_parsing_rejects_contract_violations() {
        // 布尔键只吃布尔
        assert_eq!(
            parse_setting_value(SettingKey::VersionCheckEnabled, &Value::Bool(false)).unwrap(),
            SettingValue::Bool(false)
        );
        for bad in [Value::from(1), Value::from("true"), Value::Null] {
            let err = parse_setting_value(SettingKey::VersionCheckEnabled, &bad).unwrap_err();
            assert_eq!(err.code(), ErrorCode::SettingInvalidValue, "{bad}");
        }

        // 整数键只吃整数：小数 / 负数 / 字符串 / 布尔全部拒绝
        assert_eq!(
            parse_setting_value(SettingKey::LogRetentionDays, &Value::from(30)).unwrap(),
            SettingValue::U32(30)
        );
        for bad in [
            Value::from(7.5),
            Value::from(-1),
            Value::from("30"),
            Value::Bool(true),
        ] {
            let err = parse_setting_value(SettingKey::LogRetentionDays, &bad).unwrap_err();
            assert_eq!(err.code(), ErrorCode::SettingInvalidValue, "{bad}");
        }

        // 超出 u32 的天数：拒绝，而不是截断成一个看似合理的值
        let huge = Value::from(i64::from(u32::MAX) + 1);
        assert_eq!(
            parse_setting_value(SettingKey::LogRetentionDays, &huge)
                .unwrap_err()
                .code(),
            ErrorCode::SettingInvalidValue
        );
    }

    #[test]
    fn the_wire_shape_of_a_setting_error_carries_the_key() {
        // UI 需要知道「哪个键」被拒绝才能定位到设置页的那一行
        let err = parse_setting_value(SettingKey::LogRetentionDays, &Value::from(1.5)).unwrap_err();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "SETTING_INVALID_VALUE");
        assert_eq!(json["detail"]["key"], "log.retention_days");
    }

    // ── 安装实例 ─────────────────────────────────────────

    /// 指定游戏的最小实例记录（版本未知）。
    fn installation_for(
        game_id: &str,
        id: &str,
    ) -> orbis_platform::installation::InstallationRecord {
        use orbis_core::Region;
        use orbis_platform::installation::{AddedVia, PersistedStatus};
        orbis_platform::installation::InstallationRecord {
            id: id.to_owned(),
            game_id: game_id.to_owned(),
            region: Region::Cn,
            install_path: "C:/sample".to_owned(),
            executable_path: format!("C:/sample/{id}.exe"),
            local_version: None,
            version_norm: None,
            version_source: None,
            status: PersistedStatus::Installed,
            added_via: AddedVia::Manual,
            created_at: 1_758_000_000_000,
            updated_at: 1_758_000_000_000,
        }
    }

    fn sample_installation(id: &str) -> orbis_platform::installation::InstallationRecord {
        installation_for("sample-game", id)
    }

    #[test]
    fn missing_installations_map_to_game_not_found() {
        // 不先查存在性的话，外键冲突会把 GAME_NOT_FOUND 变成 INTERNAL，
        // UI 的行动建议也会从「刷新列表」变成「导出日志」
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        let err = state
            .with_db(|db| -> OrbisResult<()> {
                require_installation(db, "ghost")?;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::GameNotFound);
        assert!(!err.retryable());
    }

    #[test]
    fn latest_version_norm_degrades_to_none_not_to_an_error() {
        // 数据库不可用、没有实例、版本未知三者在契约 §3.7 里是同一个含义
        // （→ unknown 默认安全态），因此这里必须是 None 而不是 panic / 抛错
        let no_db = state_with(None);
        assert_eq!(no_db.latest_version_norm("sample-game"), None);

        let with_db = state_with(Some(Db::open_in_memory().unwrap()));
        assert_eq!(
            with_db.latest_version_norm("sample-game"),
            None,
            "空库 → 没有实例"
        );

        with_db
            .with_db(|db| -> OrbisResult<()> {
                db.insert_installation(&sample_installation("i1"))?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            with_db.latest_version_norm("sample-game"),
            None,
            "实例存在但版本未知 → 仍是 None（02 A3：不猜）"
        );
    }

    #[test]
    fn epoch_millis_is_milliseconds_not_seconds() {
        // 契约 §1：IPC 层时间是 epoch **毫秒**。写成秒会让 UI 把 2026 年显示成 1970 年
        let now = epoch_millis();
        assert!(
            now > 1_700_000_000_000,
            "应远大于「秒」量级（2023-11 的毫秒值）：{now}"
        );
        assert!(now < 4_000_000_000_000, "不应是微秒或纳秒量级：{now}");
    }

    // ── 运行控制与备份 ────────────────────────────────────

    #[test]
    fn terminating_an_unknown_installation_is_game_not_found() {
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        assert_eq!(
            terminate_game(&state, "ghost").unwrap_err().code(),
            ErrorCode::GameNotFound
        );
    }

    #[test]
    fn terminating_a_stopped_installation_is_game_not_running() {
        // 没在运行时终止 → GAME_NOT_RUNNING，而不是「成功」：
        // 用户点「终止」却发现游戏本来就没开，这是要告知的事实，不是幂等成功。
        // （「进程在我们发出信号前自己退了」才是幂等成功，两者在 platform 层被分开。）
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        state
            .with_db(|db| -> OrbisResult<()> {
                db.insert_installation(&installation_for("genshin-impact", "i1"))?;
                Ok(())
            })
            .unwrap();

        assert_eq!(
            terminate_game(&state, "i1").unwrap_err().code(),
            ErrorCode::GameNotRunning
        );
    }

    #[test]
    fn a_fresh_installation_has_no_backups_and_reports_zero() {
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        state
            .with_db(|db| -> OrbisResult<()> {
                db.insert_installation(&installation_for("wuthering-waves", "i1"))?;
                Ok(())
            })
            .unwrap();

        assert!(
            list_backups(&state, "i1").unwrap().is_empty(),
            "没有备份是空列表，不是错误"
        );

        let info = backup_storage_info(&state, "i1").unwrap();
        assert_eq!(info.installation_id, "i1");
        assert_eq!(info.backup_count, 0);
        assert_eq!(info.total_bytes, 0);
        assert!(
            info.estimated_next_size_bytes.is_none(),
            "源目录大小需要 declared_paths（实测 T3/T6）→ 必须是 null，不能用 0 冒充「不占空间」"
        );
        assert!(info.free_disk_bytes > 0, "正常环境应能读到该卷的可用空间");
    }

    #[test]
    fn deleting_a_missing_backup_is_backup_not_found() {
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        assert_eq!(
            delete_backup(&state, "ghost").unwrap_err().code(),
            ErrorCode::BackupNotFound
        );
    }

    #[test]
    fn deleting_a_backup_removes_it_from_the_list() {
        let state = state_with(Some(Db::open_in_memory().unwrap()));
        state
            .with_db(|db| -> OrbisResult<()> {
                db.insert_installation(&installation_for("wuthering-waves", "i1"))?;
                // 直接插一行：A8 的写入路径尚未落地，但删除必须对**已存在**的记录工作
                db.conn()
                    .execute(
                        "INSERT INTO backup(id, installation_id, game_id, tool_id, trigger, \
                         file_count, total_bytes, manifest_json, created_at) \
                         VALUES ('b1', 'i1', 'wuthering-waves', NULL, 'manual', 2, 40, '{}', 1)",
                        [],
                    )
                    .expect("插入备份记录应成功");
                Ok(())
            })
            .unwrap();

        let listed = list_backups(&state, "i1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].trigger, "manual");
        assert_eq!(listed[0].tool_id, None, "NULL tool_id = 手动备份");

        delete_backup(&state, "b1").unwrap();
        assert!(
            list_backups(&state, "i1").unwrap().is_empty(),
            "删除后不应再出现在列表里"
        );
    }

    // ── 死锁回归（不做这一步，同类缺陷仍会靠「没人点那个页面」逃过 CI）──────

    /// 含一条指给 `game` 的 manifest 的状态。
    ///
    /// 用显式构造而不是 `BuiltinData::load()`：本测试依赖「该游戏确实有 manifest」这个前提，
    /// 而 `load()` 的降级路径（数据损坏 → 空集）会让前提悄悄失效。
    fn state_with_manifest(game: &str) -> AppState {
        use orbis_tools::{AssetCatalog, ManifestSet};
        let json = format!(
            r#"{{
              "id": "sample-ns/sample-tool",
              "game": "{game}",
              "name": "示例工具",
              "description": "示例说明。",
              "version": "0.1.0",
              "type": "config_modify",
              "risk_level": "L1",
              "permissions": ["write_config"],
              "requires_admin": false,
              "backup_required": true,
              "entry": {{ "executor": "sample_executor" }},
              "source": {{ "kind": "builtin" }},
              "pending_verifications": []
            }}"#
        );
        let data = BuiltinData {
            seed: orbis_core::SeedTable::empty(),
            manifests: ManifestSet::from_sources(&[("test-manifest.json", json.as_str())]),
            assets: AssetCatalog::empty(),
            issues: Vec::new(),
            degradations: Vec::new(),
        };
        AppState::new(data, Some(Db::open_in_memory().unwrap()), false)
    }

    /// 回归：命令体不得在 `with_db` 持锁期间重入 `db` 锁。
    ///
    /// 曾经的缺陷是 `getInstallationDetail` 在闭包内调用了自锁的 `tool_enabled`，
    /// 而 `std::sync::Mutex` 不可重入 → 同一线程永久阻塞 → guard 不释放 →
    /// 5s 轮询线程与全部 `with_db` 命令排队 → 整个后端冻结。
    ///
    /// **前提不能省**：实例必须属于有 manifest 的游戏。空集时 `list_tools` 的 filter
    /// 产出空集、回调根本不被调用，即使死锁仍在，这条测试也会绿灯 ——
    /// 假绿灯正是这个缺陷当初能藏住的原因，所以下面额外断言了工具非空。
    #[test]
    fn installation_detail_does_not_deadlock_on_its_own_lock() {
        use std::sync::mpsc;
        use std::time::Duration;

        let state = state_with_manifest("genshin-impact");
        state
            .with_db(|db| -> OrbisResult<()> {
                db.insert_installation(&installation_for("genshin-impact", "i1"))?;
                Ok(())
            })
            .unwrap();

        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = tx.send(installation_detail(&state, "i1"));
        });

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(detail)) => {
                assert!(
                    !detail.tools.is_empty(),
                    "该游戏有 manifest → 必须返回工具；空集说明回调没被调用，本测试失去意义"
                );
                worker.join().unwrap();
            }
            Ok(Err(_)) => panic!("应正常返回详情，却报错了"),
            // 死锁必须**失败而不是挂起**：挂起要耗到作业超时，且日志里看不出原因
            Err(_) => panic!(
                "getInstallationDetail 5 秒内未返回 —— 十有八九在 with_db 持锁期间重入了 db 锁"
            ),
        }
    }
}
