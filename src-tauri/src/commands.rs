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
//! 已实现 13/28：`listGames`、`listTools`、`getCompatibility`、`listInstallations`、
//! `getInstallationDetail`、`removeInstallation`、`getRuntimeStates`、
//! `getLaunchProfile`、`setLaunchProfile`、`resetLaunchProfile`、
//! `getSettings`、`setSetting`、`windowControl`。
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

use std::sync::Mutex;

use orbis_core::Version;
use orbis_platform::db::{Db, SettingKey, SettingValue};
use orbis_platform::installation::InstallationRecord;
use orbis_platform::log::LogLevel;
use orbis_platform::process::ProcessSnapshot;
use orbis_tools::{BuiltinData, CompatibilityDto, ToolDto};
use serde_json::Value;
use tauri::State;

use crate::dto::{
    game_catalog_entries, installation_dto, installation_list, launch_profile_dto,
    runtime_state_dto, state_changed_payload, AppSettingsDto, GameCatalogEntryDto,
    GameStateChangedPayload, InstallationDetailDto, InstallationListResultDto, LaunchProfileDto,
    RuntimeState, RuntimeStateDto,
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
}

impl AppState {
    pub fn new(data: BuiltinData, db: Option<Db>, logging_ok: bool) -> Self {
        Self {
            data,
            db: Mutex::new(db),
            logging_ok,
            last_runtime: Mutex::new(None),
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

    /// 工具启停查询。
    ///
    /// DB 不可用 → 一律 `false`：`listTools` 是首页刷新路径，为它抛错会让整个工具
    /// 面板变成错误页，而「数据库打不开」这件事已经在启动自检里记了 ERROR。
    /// 但**读取失败必须留痕** —— 静默把「启用中」显示成「已关闭」会让用户以为
    /// 自己的设置丢了（04 §8 禁止静默失败）。
    fn tool_enabled(&self, tool_id: &str) -> bool {
        let Ok(guard) = self.db.lock() else {
            self.announce(LogLevel::Warn, "工具状态查询失败：数据库锁中毒");
            return false;
        };
        guard
            .as_ref()
            .is_some_and(|db| match db.is_tool_enabled(tool_id) {
                Ok(enabled) => enabled,
                Err(err) => {
                    self.announce(
                        LogLevel::Warn,
                        &format!("工具 {tool_id} 的启停状态读取失败，按未启用处理：{err}"),
                    );
                    false
                }
            })
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

    /// 轮询一次运行态，返回**发生变化**的实例（供 `game:state-changed` 事件）。
    ///
    /// 读不到实例列表时返回空并留痕 —— 此时「不知道」比「谎报未运行」安全：
    /// 把正在跑的游戏显示成已停止，会让用户以为时长统计或工具状态出了问题。
    pub(crate) fn poll_runtime_changes(&self) -> Vec<GameStateChangedPayload> {
        let Some(records) = self.try_installations() else {
            return Vec::new();
        };
        let current = self.runtime_states(&records);

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

    /// 读实例列表；读不到（无数据库 / 查询失败 / 锁中毒）→ `None` 并留痕。
    fn try_installations(&self) -> Option<Vec<InstallationRecord>> {
        let Ok(guard) = self.db.lock() else {
            self.announce(LogLevel::Warn, "运行态轮询跳过：数据库锁中毒");
            return None;
        };
        let db = guard.as_ref()?;
        match db.installations() {
            Ok(records) => Some(records),
            Err(err) => {
                self.announce(LogLevel::Warn, &format!("运行态轮询跳过：{err}"));
                None
            }
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
        Ok(installation_list(state.data(), records, &runtime))
    })
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
/// `latestBackup` 恒 `null`：A8 备份未落地（被实测项 T3/T6 阻塞），
/// 契约允许它为 null —— 不在这里伪造一条备份记录。
#[tauri::command]
pub fn getInstallationDetail(
    state: State<'_, AppState>,
    installationId: String,
) -> OrbisResult<InstallationDetailDto> {
    state.with_db(|db| {
        let record = require_installation(db, &installationId)?;
        let runtime = state.runtime_states(std::slice::from_ref(&record));
        let is_enabled = |tool_id: &str| state.tool_enabled(tool_id);
        Ok(InstallationDetailDto {
            installation: installation_dto(
                state.data(),
                &record,
                runtime.first().and_then(|state| state.pid),
            ),
            tools: orbis_tools::list_tools(state.data(), Some(&record.game_id), &is_enabled),
            latest_backup: None,
            launch_profile: launch_profile_dto(&record, db.launch_profile(&installationId)?),
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

    fn sample_installation(id: &str) -> orbis_platform::installation::InstallationRecord {
        use orbis_core::Region;
        use orbis_platform::installation::{AddedVia, PersistedStatus};
        orbis_platform::installation::InstallationRecord {
            id: id.to_owned(),
            game_id: "sample-game".to_owned(),
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
}
