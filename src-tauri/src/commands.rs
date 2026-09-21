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
//! 已实现 6/28：`listGames`、`listTools`、`getCompatibility`、`getSettings`、
//! `setSetting`、`windowControl`。
//!
//! **`getTool` 刻意未实现**：它的 `consentText`（L3 版本化授权全文）目前**没有任何
//! 数据来源** —— 契约 §7.3 说它来自「Manifest / 资源内的版本化文案」，而
//! `manifests/*.json` 里没有这个字段，`consentTextHash` 也就无从计算。
//! 先返回 `consentText: null` 是危险的：UI 会据此认为「这个工具不需要授权」，
//! 而 L3 的授权门槛是 00 §12.3 规则 7 的硬约束。等文案源定稿（04 §7.2）再落地。

use std::sync::Mutex;

use orbis_platform::db::{Db, SettingKey, SettingValue};
use orbis_platform::log::LogLevel;
use orbis_tools::{BuiltinData, CompatibilityDto, ToolDto};
use serde_json::Value;
use tauri::State;

use crate::dto::{game_catalog_entries, AppSettingsDto, GameCatalogEntryDto};
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
}

impl AppState {
    pub fn new(data: BuiltinData, db: Option<Db>, logging_ok: bool) -> Self {
        Self {
            data,
            db: Mutex::new(db),
            logging_ok,
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
/// 安装实例（A3）尚未落地 → 本地版本未知，按契约 §3.7 必返回
/// `status: "unknown"` + `matchKind: "none"`（02 A3：不猜版本）。
/// 因此本命令当前用于验证「种子表 → DTO」这条链路，而不是产出一个有用的判定。
#[tauri::command]
pub fn getCompatibility(
    state: State<'_, AppState>,
    gameId: String,
    toolId: String,
) -> CompatibilityDto {
    orbis_tools::compatibility(&state.data().seed, &gameId, &toolId, None)
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
}
