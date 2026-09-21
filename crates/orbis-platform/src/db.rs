//! SQLite 持久化（`pm/04-技术设计.md` §6.1 schema / §6.3 目录布局）。
//!
//! # 本模块只做「建库 + 迁移 + 设置」
//!
//! 各业务实体的仓储方法（installation / backup / playtime_session …）**随对应域落地时
//! 再加**，不在此处预留空方法 —— 空的仓储层只会让「已实现」看起来比实际多。
//!
//! # 迁移策略（前向、单源、拒绝降级）
//!
//! 版本号存在 SQLite 自带的 `PRAGMA user_version`，不额外建表（少一张表就少一处
//! 与 SQLite 自身冲突的可能）。三条规则：
//!
//! 1. **前向**：只从低版本迁到本代码支持的版本，不提供回退脚本。
//! 2. **拒绝降级**：库版本**高于**代码支持的版本时直接报错、**一个字节都不改**。
//!    用旧版 Orbis 打开新版建的数据（或用户回退了版本）时，静默按旧 schema 读写
//!    会写坏数据 —— 宁可不启动。
//! 3. **幂等**：已是最新版本时迁移是空操作。
//!
//! # 设置项白名单（04 §7.4）
//!
//! `app_setting` 只允许 §6.1 预置的三个键，且**只允许作为设置项出现**的语义都在
//! [`SettingKey`] 里显式列出。新增设置项必须先回 01 清单评审 —— 因此本模块不提供
//! 「按任意字符串读写设置」的通用入口：那种 API 会让白名单形同虚设。
//! 强制备份 / 修改后验证 / L3 授权门槛 / 兼容门控**不是**设置项，不该也无法从这里关掉。

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};

/// 本代码支持的 schema 版本（与 `PRAGMA user_version` 对应）。
///
/// 每次改 schema 都要 +1 并在 [`migrate`] 里补一段迁移；**不要**改动已发布的迁移。
pub const SCHEMA_VERSION: i32 = 1;

/// 保留天数护栏 —— 口径来自契约 §3.9「`log.retention_days` 限 1–365」。
const RETENTION_DAYS_MIN: u32 = 1;
const RETENTION_DAYS_MAX: u32 = 365;
/// 时长 checkpoint 间隔护栏 —— 口径来自契约 §3.9「`playtime.checkpoint_sec` 限 10–300」。
const CHECKPOINT_SEC_MIN: u32 = 10;
const CHECKPOINT_SEC_MAX: u32 = 300;

/// 数据库操作失败的原因。
#[derive(Debug)]
pub enum DbError {
    /// 打开文件或建目录失败
    Open { path: PathBuf, detail: String },
    /// 库版本高于本代码支持的版本 —— 拒绝打开，避免按旧 schema 读写写坏数据
    UnsupportedSchemaVersion { found: i32, supported: i32 },
    /// 迁移脚本执行失败
    Migration { to: i32, detail: String },
    /// SQLite 层面的错误
    Sqlite(rusqlite::Error),
    /// 设置项取值与键不匹配或越界
    InvalidSettingValue { key: &'static str, detail: String },
    /// 设置项读取到的存库文本不是合法 JSON / 与键类型不符（数据被外部改坏）
    CorruptSettingValue { key: String, detail: String },
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, detail } => write!(f, "打开数据库失败（{}）：{detail}", path.display()),
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "数据库 schema 版本为 {found}，本程序只支持到 {supported} —— 拒绝打开（请升级 Orbis）"
            ),
            Self::Migration { to, detail } => write!(f, "迁移到 v{to} 失败：{detail}"),
            Self::Sqlite(err) => write!(f, "SQLite 错误：{err}"),
            Self::InvalidSettingValue { key, detail } => write!(f, "设置项 {key} 取值非法：{detail}"),
            Self::CorruptSettingValue { key, detail } => {
                write!(f, "设置项 {key} 的存库值无法解析：{detail}")
            }
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err)
    }
}

// ── 设置项（04 §6.1 预置键 / §7.4 边界）────────────────────

/// `app_setting` 允许出现的键。
///
/// 枚举即白名单 —— 拿不到一个「未知键」的 [`SettingKey`]，因此不可能绕过白名单。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingKey {
    /// E1：版本探测开关（02 E1 明确「用户可关」，是少数**允许**的开关）
    VersionCheckEnabled,
    /// Q4 已拍板：日志保留天数默认 14（04 §5.11）
    LogRetentionDays,
    /// A6：运行中会话的落库间隔（04 §5.10）
    PlaytimeCheckpointSec,
}

impl SettingKey {
    pub const ALL: [Self; 3] = [
        Self::VersionCheckEnabled,
        Self::LogRetentionDays,
        Self::PlaytimeCheckpointSec,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::VersionCheckEnabled => "version_check.enabled",
            Self::LogRetentionDays => "log.retention_days",
            Self::PlaytimeCheckpointSec => "playtime.checkpoint_sec",
        }
    }

    /// 严格解析（不认非白名单键，也不做大小写 / 空白容错）。
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.slug() == slug)
    }

    /// 未设置时的默认值（对应契约 `AppSettings` 的三个字段）。
    pub const fn default_value(self) -> SettingValue {
        match self {
            Self::VersionCheckEnabled => SettingValue::Bool(true),
            Self::LogRetentionDays => SettingValue::U32(14),
            Self::PlaytimeCheckpointSec => SettingValue::U32(30),
        }
    }

    /// 已知键的语义说明（供实现期护栏报错时引用）。
    const fn expectation(self) -> &'static str {
        match self {
            Self::VersionCheckEnabled => "布尔值 true / false",
            Self::LogRetentionDays => "1–365 的整数（天）",
            Self::PlaytimeCheckpointSec => "10–300 的整数（秒）",
        }
    }
}

/// 设置项取值。类型化而非 `String`，使「用字符串写布尔」这类错误在编译期就不成立。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValue {
    Bool(bool),
    U32(u32),
}

impl SettingValue {
    /// 存库形式（`app_setting.value` 是 JSON 文本，04 §6.1）。
    fn to_storage(self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::U32(value) => value.to_string(),
        }
    }

    /// 解析存库文本。类型不符即报错 —— 不静默转成默认值（那会把数据损坏藏起来）。
    fn parse_stored(key: SettingKey, raw: &str) -> Result<Self, DbError> {
        let corrupt = |detail: String| DbError::CorruptSettingValue {
            key: key.slug().to_owned(),
            detail,
        };
        match key {
            SettingKey::VersionCheckEnabled => serde_json::from_str::<bool>(raw)
                .map(Self::Bool)
                .map_err(|err| corrupt(err.to_string())),
            SettingKey::LogRetentionDays | SettingKey::PlaytimeCheckpointSec => {
                let number =
                    serde_json::from_str::<i64>(raw).map_err(|err| corrupt(err.to_string()))?;
                let value = u32::try_from(number)
                    .map_err(|_| corrupt(format!("负数或超范围：{number}")))?;
                guard_range(key, value)?;
                Ok(Self::U32(value))
            }
        }
    }

    /// 写入前的类型检查（`set_setting` 用）。与 [`Self::parse_stored`] 是两条路：
    /// 一条管「写进去要合法」，一条管「读出来要能信」。
    fn ensure_matches(self, key: SettingKey) -> Result<(), DbError> {
        let mismatched = || DbError::InvalidSettingValue {
            key: key.slug(),
            detail: format!("该键期望{}，得到 {self:?}", key.expectation()),
        };
        match (key, self) {
            (SettingKey::VersionCheckEnabled, Self::Bool(_)) => Ok(()),
            (SettingKey::VersionCheckEnabled, Self::U32(_)) => Err(mismatched()),
            (
                SettingKey::LogRetentionDays | SettingKey::PlaytimeCheckpointSec,
                Self::U32(value),
            ) => {
                guard_range(key, value).map(|_| ())?;
                Ok(())
            }
            (SettingKey::LogRetentionDays | SettingKey::PlaytimeCheckpointSec, Self::Bool(_)) => {
                Err(mismatched())
            }
        }
    }
}

/// 取值范围护栏（契约 §3.9 / §5 `SETTING_INVALID_VALUE`）。
///
/// 这两个区间**不是**「宽到不可能冲突的兜底」，而是契约明文给出的产品口径：
/// `log.retention_days` 1–365（超过一年的日志保留没有意义）、
/// `playtime.checkpoint_sec` 10–300（小于 10 秒的落库间隔会让写放大到无意义，
/// 大于 300 秒则崩溃丢失的游戏时长过多，A6 要求「崩溃不丢」）。
///
/// 与前端参照实现（`src/api/mock.ts` 的 `setSetting`）保持一致：后端放宽而 mock 收紧
/// 会让同一个输入在两个环境下行为不同，这类不一致在联调时最难排查。
fn guard_range(key: SettingKey, value: u32) -> Result<(), DbError> {
    let (min, max) = match key {
        SettingKey::LogRetentionDays => (RETENTION_DAYS_MIN, RETENTION_DAYS_MAX),
        SettingKey::PlaytimeCheckpointSec => (CHECKPOINT_SEC_MIN, CHECKPOINT_SEC_MAX),
        // 布尔键没有范围概念，走到这里说明调用方用错了键
        SettingKey::VersionCheckEnabled => return Ok(()),
    };
    if !(min..=max).contains(&value) {
        return Err(DbError::InvalidSettingValue {
            key: key.slug(),
            detail: format!(
                "取值 {value} 超出护栏 {min}–{max}（该键期望{}）",
                key.expectation()
            ),
        });
    }
    Ok(())
}

/// 契约 §6 `AppSettings` 的 Rust 侧对应物。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSettings {
    pub version_check_enabled: bool,
    pub log_retention_days: u32,
    pub playtime_checkpoint_sec: u32,
}

/// schema v1 的完整 DDL —— **逐字取自 04 §6.1**（含注释，便于与文档对照）。
///
/// 不在本串里写 `PRAGMA user_version`：版本号由 [`migrate`] 在事务内单独设置，
/// 这样「DDL 生效」与「版本号前进」是同一个原子步骤，不会出现半迁移状态。
const DDL_V1: &str = r#"
CREATE TABLE installation (
  id              TEXT PRIMARY KEY,
  game_id         TEXT NOT NULL,
  region          TEXT NOT NULL,
  install_path    TEXT NOT NULL,
  executable_path TEXT NOT NULL UNIQUE,
  local_version   TEXT,
  version_norm    TEXT,
  version_source  TEXT,
  status          TEXT NOT NULL DEFAULT 'installed',
  added_via       TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
);

CREATE TABLE launch_profile (
  installation_id TEXT PRIMARY KEY REFERENCES installation(id) ON DELETE CASCADE,
  args            TEXT NOT NULL DEFAULT '',
  updated_at      INTEGER NOT NULL
);

CREATE TABLE playtime_session (
  id              TEXT PRIMARY KEY,
  installation_id TEXT NOT NULL REFERENCES installation(id) ON DELETE CASCADE,
  started_at      INTEGER NOT NULL,
  checkpoint_at   INTEGER NOT NULL,
  ended_at        INTEGER,
  duration_sec    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_session_install ON playtime_session(installation_id, started_at);

CREATE TABLE tool_state (
  tool_id            TEXT PRIMARY KEY,
  enabled            INTEGER NOT NULL DEFAULT 0,
  authorization_json TEXT,
  asset_json         TEXT,
  updated_at         INTEGER NOT NULL
);

CREATE TABLE backup (
  id              TEXT PRIMARY KEY,
  installation_id TEXT NOT NULL REFERENCES installation(id) ON DELETE CASCADE,
  game_id         TEXT NOT NULL,
  tool_id         TEXT,
  trigger         TEXT NOT NULL,
  file_count      INTEGER NOT NULL,
  total_bytes     INTEGER NOT NULL,
  manifest_json   TEXT NOT NULL,
  created_at      INTEGER NOT NULL
);
CREATE INDEX idx_backup_install ON backup(installation_id, created_at DESC);

CREATE TABLE app_setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

/// SQLite 连接（04 §6.1）。
///
/// `Connection` 是 `Send` 但 **不是** `Sync`：跨线程共享时由上层用锁包起来
/// （04 §5.12 的「按安装实例串行化」正是这个用途）。
pub struct Db {
    conn: Connection,
}

impl Db {
    /// 打开（必要时创建）数据库文件，建好父目录，并迁移到 [`SCHEMA_VERSION`]。
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| DbError::Open {
                path: path.to_path_buf(),
                detail: format!("无法创建数据目录：{err}"),
            })?;
        }
        let conn = Connection::open(path).map_err(|err| DbError::Open {
            path: path.to_path_buf(),
            detail: err.to_string(),
        })?;
        Self::from_connection(conn)
    }

    /// 内存库（单测用）。注意内存库没有 WAL，`journal_mode` 会停在 `memory`。
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, DbError> {
        // 迁移前先配置：WAL 与 foreign_keys 都是**每个连接**的设置，
        // 且 FK 约束要在建表/写入之前就打开，否则约束形同虚设。
        configure(&conn)?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// 只读借用底层连接（仓储方法落地前，测试与迁移检查用）。
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn schema_version(&self) -> Result<i32, DbError> {
        user_version(&self.conn)
    }

    /// 读取设置项；未设置过时返回该键的默认值（不是 `None`）。
    pub fn setting(&self, key: SettingKey) -> Result<SettingValue, DbError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = ?1",
                [key.slug()],
                |row| row.get(0),
            )
            .optional()?;
        match raw {
            Some(raw) => SettingValue::parse_stored(key, &raw),
            None => Ok(key.default_value()),
        }
    }

    /// 写入设置项。类型不符或越界一律拒绝（契约 §5 `SETTING_INVALID_VALUE`）。
    pub fn set_setting(&self, key: SettingKey, value: SettingValue) -> Result<(), DbError> {
        value.ensure_matches(key)?;
        self.conn.execute(
            "INSERT INTO app_setting(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key.slug(), value.to_storage()],
        )?;
        Ok(())
    }

    /// 一次性读出全部设置（壳在启动时用）。
    pub fn settings(&self) -> Result<AppSettings, DbError> {
        let read_bool = |key: SettingKey| -> Result<bool, DbError> {
            match self.setting(key)? {
                SettingValue::Bool(value) => Ok(value),
                other => Err(DbError::CorruptSettingValue {
                    key: key.slug().to_owned(),
                    detail: format!("期望布尔，得到 {other:?}"),
                }),
            }
        };
        let read_u32 = |key: SettingKey| -> Result<u32, DbError> {
            match self.setting(key)? {
                SettingValue::U32(value) => Ok(value),
                other => Err(DbError::CorruptSettingValue {
                    key: key.slug().to_owned(),
                    detail: format!("期望整数，得到 {other:?}"),
                }),
            }
        };
        Ok(AppSettings {
            version_check_enabled: read_bool(SettingKey::VersionCheckEnabled)?,
            log_retention_days: read_u32(SettingKey::LogRetentionDays)?,
            playtime_checkpoint_sec: read_u32(SettingKey::PlaytimeCheckpointSec)?,
        })
    }

    // ── 工具启停（`tool_state`，04 §6.1）────────────────────
    //
    // 本模块只提供**读取**：写入属于 setToolEnabled 的完整链路（04 §5.5
    // Gate → Precheck → Backup → Modify → Verify），它必须与备份、回滚、
    // 事件流一起落地，否则会出现「状态已翻转但没有任何落盘保护」的工具。

    /// 工具是否处于启用态。
    ///
    /// **无行 = `false`**，与 DDL 的 `enabled INTEGER NOT NULL DEFAULT 0` 以及
    /// 02 C3「L3 工具默认关闭」一致。读路径刻意**不 upsert**：一次查询顺手写入
    /// 会把「读设置/读状态」变成有副作用的操作，也让 `is_tool_enabled` 在只读场景
    /// （诊断、日志导出）里变得不可用。
    pub fn is_tool_enabled(&self, tool_id: &str) -> Result<bool, DbError> {
        let enabled: Option<i64> = self
            .conn
            .query_row(
                "SELECT enabled FROM tool_state WHERE tool_id = ?1",
                [tool_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(enabled.unwrap_or(0) != 0)
    }
}

fn configure(conn: &Connection) -> Result<(), DbError> {
    // `PRAGMA journal_mode = WAL` 会**返回一行**，所以要用 query_row；
    // 用 execute 会因「返回了结果」而报错。（内存库返回 "memory"，不报错。）
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;

    // 外键约束默认是**关**的，必须每连接打开，否则 ON DELETE CASCADE 不生效
    conn.pragma_update(None, "foreign_keys", "ON")?;

    // WAL 下多进程/多线程并发写会短暂 BUSY，给一个等待窗口而不是立刻失败
    conn.busy_timeout(Duration::from_millis(5_000))?;
    Ok(())
}

fn user_version(conn: &Connection) -> Result<i32, DbError> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn migrate(conn: &Connection) -> Result<(), DbError> {
    let current = user_version(conn)?;

    if current > SCHEMA_VERSION {
        // 宁可不开，也不按旧 schema 去动新版数据
        return Err(DbError::UnsupportedSchemaVersion {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }

    for target in (current + 1)..=SCHEMA_VERSION {
        apply_migration(conn, target)?;
    }
    Ok(())
}

fn apply_migration(conn: &Connection, target: i32) -> Result<(), DbError> {
    if target != 1 {
        return Err(DbError::Migration {
            to: target,
            detail: "没有对应的迁移脚本（新增 schema 版本时必须同时补迁移）".to_owned(),
        });
    }

    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(DDL_V1).map_err(|err| DbError::Migration {
        to: target,
        detail: err.to_string(),
    })?;
    tx.pragma_update(None, "user_version", target)
        .map_err(|err| DbError::Migration {
            to: target,
            detail: format!("设置 user_version 失败：{err}"),
        })?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;
    use crate::test_support::TempDir;
    use rusqlite::params;

    /// 冻结时间戳：schema 的 *_at 都是 INTEGER（04 §6.1），测试不需要真实时间。
    const TS: i64 = 1_758_000_000;

    fn insert_installation(
        conn: &Connection,
        id: &str,
        executable: &str,
    ) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO installation
               (id, game_id, region, install_path, executable_path, local_version, version_norm,
                version_source, added_via, created_at, updated_at)
             VALUES (?1, 'sample-game', 'cn', 'C:/sample', ?2, '1.2.3', '1.2',
                     'exe_versioninfo', 'scan', ?3, ?3)",
            params![id, executable, TS],
        )
    }

    fn table_exists(db: &Db, name: &str) -> bool {
        db.conn()
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |_| Ok(()),
            )
            .optional()
            .unwrap()
            .is_some()
    }

    fn index_exists(db: &Db, name: &str) -> bool {
        db.conn()
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [name],
                |_| Ok(()),
            )
            .optional()
            .unwrap()
            .is_some()
    }

    // ── 迁移与 schema ────────────────────────────────────

    #[test]
    fn migration_creates_the_documented_tables_and_indexes() {
        let db = Db::open_in_memory().expect("应能建库");
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);

        // 04 §6.1 的 6 张表（Game / GameVersion 不入库；其余实体不在 MVP）
        for table in [
            "installation",
            "launch_profile",
            "playtime_session",
            "tool_state",
            "backup",
            "app_setting",
        ] {
            assert!(table_exists(&db, table), "缺少表：{table}");
        }
        assert!(index_exists(&db, "idx_session_install"));
        assert!(index_exists(&db, "idx_backup_install"));
    }

    #[test]
    fn installation_columns_match_the_design() {
        // 列名与顺序是「逐字取自 04 §6.1」的可验证形式；
        // 将来若有人改动 DDL 而没同步文档，这里会先红。
        let db = Db::open_in_memory().unwrap();
        let stmt = db.conn().prepare("SELECT * FROM installation").unwrap();
        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(
            columns,
            vec![
                "id",
                "game_id",
                "region",
                "install_path",
                "executable_path",
                "local_version",
                "version_norm",
                "version_source",
                "status",
                "added_via",
                "created_at",
                "updated_at",
            ]
        );
    }

    #[test]
    fn default_status_is_installed() {
        // DDL 里 status 的 DEFAULT 'installed' 必须真的生效
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO installation
                   (id, game_id, region, install_path, executable_path, added_via, created_at, updated_at)
                 VALUES ('i1', 'sample-game', 'cn', 'C:/a', 'C:/a/g.exe', 'scan', ?1, ?1)",
                [TS],
            )
            .unwrap();
        let status: String = db
            .conn()
            .query_row(
                "SELECT status FROM installation WHERE id = 'i1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "installed");
    }

    #[test]
    fn migration_is_idempotent_across_reopens() {
        let tmp = TempDir::new("db-idempotent");
        let path = tmp.path().join("orbis.db");
        {
            let db = Db::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
        }
        // 二次打开：不应报错、版本不变、数据仍在
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(table_exists(&db, "app_setting"));
    }

    #[test]
    fn wal_is_enabled_on_a_file_database() {
        // 04 §6.1 要求 WAL 模式
        let tmp = TempDir::new("db-wal");
        let db = Db::open(&tmp.path().join("orbis.db")).unwrap();
        let mode: String = db
            .conn()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn newer_schema_version_is_refused_without_touching_the_file() {
        // 用户回退 Orbis 版本时的保护：宁可不开，也不按旧 schema 读写新库
        let tmp = TempDir::new("db-downgrade");
        let path = tmp.path().join("orbis.db");
        {
            let db = Db::open(&path).unwrap();
            db.conn()
                .pragma_update(None, "user_version", SCHEMA_VERSION + 98)
                .unwrap();
        }
        match Db::open(&path) {
            Err(DbError::UnsupportedSchemaVersion { found, supported }) => {
                assert_eq!(found, SCHEMA_VERSION + 98);
                assert_eq!(supported, SCHEMA_VERSION);
            }
            // Db 不实现 Debug（它持有一个连接），所以这里用 Display 而不是 {other:?}
            Err(other) => panic!("应因版本过高而拒绝，实际错误：{other}"),
            Ok(db) => panic!(
                "应拒绝打开更高版本的库，却在 schema v{} 下成功了",
                db.schema_version().unwrap()
            ),
        }
    }

    // ── 约束 ────────────────────────────────────────────

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Db::open_in_memory().unwrap();
        // 没有对应的 installation → 必须被拒（说明 PRAGMA foreign_keys 确实打开了）
        let result = db.conn().execute(
            "INSERT INTO launch_profile(installation_id, args, updated_at) VALUES ('ghost', '', ?1)",
            [TS],
        );
        assert!(result.is_err(), "悬空外键必须被拒绝");
    }

    #[test]
    fn executable_path_is_unique() {
        // 04 §5.12：单实例/同路径去重靠这条约束兜底
        let db = Db::open_in_memory().unwrap();
        insert_installation(db.conn(), "i1", "C:/a/g.exe").unwrap();
        assert!(
            insert_installation(db.conn(), "i2", "C:/a/g.exe").is_err(),
            "同一 exe 路径不得重复入库"
        );
    }

    #[test]
    fn deleting_an_installation_cascades_to_dependents() {
        let db = Db::open_in_memory().unwrap();
        insert_installation(db.conn(), "i1", "C:/a/g.exe").unwrap();
        db.conn()
            .execute(
                "INSERT INTO launch_profile(installation_id, args, updated_at) VALUES ('i1', '--x', ?1)",
                [TS],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO playtime_session(id, installation_id, started_at, checkpoint_at)
                 VALUES ('s1', 'i1', ?1, ?1)",
                [TS],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO backup(id, installation_id, game_id, trigger, file_count, total_bytes,
                                    manifest_json, created_at)
                 VALUES ('b1', 'i1', 'sample-game', 'manual', 0, 0, '[]', ?1)",
                [TS],
            )
            .unwrap();

        db.conn()
            .execute("DELETE FROM installation WHERE id = 'i1'", [])
            .unwrap();

        for (table, clause) in [
            ("launch_profile", "installation_id = 'i1'"),
            ("playtime_session", "installation_id = 'i1'"),
            ("backup", "installation_id = 'i1'"),
        ] {
            let left: i64 = db
                .conn()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {clause}"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(left, 0, "{table} 的从属行应随 installation 级联删除");
        }
    }

    #[test]
    fn tool_state_defaults_to_disabled() {
        // C3 验收：L3 工具默认关闭 —— 这条默认值就落在 schema 上
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO tool_state(tool_id, updated_at) VALUES ('sample-ns/sample-tool', ?1)",
                [TS],
            )
            .unwrap();
        let enabled: i64 = db
            .conn()
            .query_row(
                "SELECT enabled FROM tool_state WHERE tool_id = 'sample-ns/sample-tool'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(enabled, 0);
    }

    #[test]
    fn reading_tool_enabled_state_never_writes() {
        // 读路径不得留下行：否则「查一次状态」会污染 tool_state，并让只读诊断场景有副作用
        let db = Db::open_in_memory().unwrap();
        assert!(!db.is_tool_enabled("sample-ns/sample-tool").unwrap());

        let rows: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM tool_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0, "读取不应创建 tool_state 行");
    }

    #[test]
    fn tool_enabled_reflects_the_stored_flag() {
        let db = Db::open_in_memory().unwrap();
        let tool = "sample-ns/sample-tool";
        db.conn()
            .execute(
                "INSERT INTO tool_state(tool_id, enabled, updated_at) VALUES (?1, 1, ?2)",
                params![tool, TS],
            )
            .unwrap();
        assert!(db.is_tool_enabled(tool).unwrap());

        // 显式 0 与「无行」都必须读成 false
        db.conn()
            .execute(
                "UPDATE tool_state SET enabled = 0 WHERE tool_id = ?1",
                [tool],
            )
            .unwrap();
        assert!(!db.is_tool_enabled(tool).unwrap());
        assert!(!db.is_tool_enabled("absent-ns/absent-tool").unwrap());
    }

    // ── 设置项 ──────────────────────────────────────────

    #[test]
    fn setting_keys_are_a_closed_whitelist() {
        for key in SettingKey::ALL {
            assert_eq!(SettingKey::from_slug(key.slug()), Some(key));
        }
        // 04 §7.4：设置页只允许出现预置键；非白名单键必须解析不出
        for bogus in [
            "auto_backup.enabled",
            "safety.disabled",
            "version_check.ENABLED",
            "log.retention_days ",
            "",
        ] {
            assert_eq!(
                SettingKey::from_slug(bogus),
                None,
                "不应认作设置键：{bogus:?}"
            );
        }
        // 三个键的字面量互不重复
        let mut slugs: Vec<&str> = SettingKey::ALL.iter().map(|k| k.slug()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), SettingKey::ALL.len());
    }

    #[test]
    fn defaults_match_the_documented_values() {
        // 空库时读设置必须拿到文档值，而不是 None / 0
        let db = Db::open_in_memory().unwrap();
        let settings = db.settings().unwrap();
        assert_eq!(
            settings,
            AppSettings {
                version_check_enabled: true,
                log_retention_days: 14,      // 04 §11 Q4 拍板
                playtime_checkpoint_sec: 30, // 04 §5.10
            }
        );
        // 什么也没写入
        let rows: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM app_setting", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0, "读默认值不应产生写入");
    }

    #[test]
    fn settings_roundtrip_and_survive_reopen() {
        let tmp = TempDir::new("db-settings");
        let path = tmp.path().join("orbis.db");
        {
            let db = Db::open(&path).unwrap();
            db.set_setting(SettingKey::VersionCheckEnabled, SettingValue::Bool(false))
                .unwrap();
            db.set_setting(SettingKey::LogRetentionDays, SettingValue::U32(7))
                .unwrap();
            // 同一个键再写一次：应覆盖而不是报冲突
            db.set_setting(SettingKey::LogRetentionDays, SettingValue::U32(3))
                .unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(
            db.settings().unwrap(),
            AppSettings {
                version_check_enabled: false,
                log_retention_days: 3,
                playtime_checkpoint_sec: 30,
            }
        );
        // 存库形式是 JSON 文本（04 §6.1）
        let raw: String = db
            .conn()
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'log.retention_days'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, "3");
    }

    #[test]
    fn type_mismatch_is_rejected() {
        let db = Db::open_in_memory().unwrap();
        // 用布尔去写「天数」——类型化设计应在这里拦住
        let err = db
            .set_setting(SettingKey::LogRetentionDays, SettingValue::Bool(true))
            .unwrap_err();
        assert!(matches!(err, DbError::InvalidSettingValue { .. }), "{err}");

        // 用整数去写开关
        let err = db
            .set_setting(SettingKey::VersionCheckEnabled, SettingValue::U32(1))
            .unwrap_err();
        assert!(matches!(err, DbError::InvalidSettingValue { .. }), "{err}");

        // 拒绝之后不得留下任何行
        let rows: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM app_setting", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        let db = Db::open_in_memory().unwrap();
        for (key, value) in [
            (SettingKey::LogRetentionDays, 0u32),
            (SettingKey::LogRetentionDays, RETENTION_DAYS_MAX + 1),
            (SettingKey::PlaytimeCheckpointSec, 0u32),
            (SettingKey::PlaytimeCheckpointSec, CHECKPOINT_SEC_MAX + 1),
        ] {
            let err = db.set_setting(key, SettingValue::U32(value)).unwrap_err();
            assert!(
                matches!(err, DbError::InvalidSettingValue { .. }),
                "{key:?}={value} 应被拒绝，实际：{err}"
            );
        }
        // 边界值本身合法
        db.set_setting(
            SettingKey::LogRetentionDays,
            SettingValue::U32(RETENTION_DAYS_MIN),
        )
        .unwrap();
    }

    #[test]
    fn setting_bounds_match_the_contract() {
        // 契约 §3.9 明文给出的区间，不是本模块自选的兜底值：
        // 放宽会让「后端接受、前端 mock 拒绝」的同输入不同行为在联调时冒出来。
        let db = Db::open_in_memory().unwrap();

        // 先把「文档写的区间」钉死，防止有人顺手改常量而没回契约
        assert_eq!((RETENTION_DAYS_MIN, RETENTION_DAYS_MAX), (1, 365));
        assert_eq!((CHECKPOINT_SEC_MIN, CHECKPOINT_SEC_MAX), (10, 300));

        for value in [RETENTION_DAYS_MIN, RETENTION_DAYS_MAX] {
            db.set_setting(SettingKey::LogRetentionDays, SettingValue::U32(value))
                .unwrap_or_else(|e| panic!("保留天数 {value} 应合法：{e}"));
        }
        for value in [CHECKPOINT_SEC_MIN, CHECKPOINT_SEC_MAX] {
            db.set_setting(SettingKey::PlaytimeCheckpointSec, SettingValue::U32(value))
                .unwrap_or_else(|e| panic!("checkpoint {value} 秒应合法：{e}"));
        }
        for (key, value) in [
            (SettingKey::LogRetentionDays, RETENTION_DAYS_MAX + 1),
            (SettingKey::PlaytimeCheckpointSec, CHECKPOINT_SEC_MIN - 1),
            (SettingKey::PlaytimeCheckpointSec, CHECKPOINT_SEC_MAX + 1),
        ] {
            assert!(
                db.set_setting(key, SettingValue::U32(value)).is_err(),
                "{key:?}={value} 应在契约区间之外被拒绝"
            );
        }

        // 默认值必须落在自己的区间内 —— 否则「新装用户第一次改设置」就会报错
        for key in [
            SettingKey::LogRetentionDays,
            SettingKey::PlaytimeCheckpointSec,
        ] {
            let SettingValue::U32(default) = key.default_value() else {
                panic!("{key:?} 应为整数键");
            };
            db.set_setting(key, SettingValue::U32(default))
                .unwrap_or_else(|e| panic!("{key:?} 的默认值 {default} 越界：{e}"));
        }
    }

    #[test]
    fn externally_corrupted_setting_value_is_reported_not_defaulted() {
        // 有人手改了 db 文件 / 旧版本写入了异物 → 必须报错，不得静默回落成默认值
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO app_setting(key, value) VALUES ('log.retention_days', '\"十四天\"')",
                [],
            )
            .unwrap();
        let err = db.setting(SettingKey::LogRetentionDays).unwrap_err();
        assert!(matches!(err, DbError::CorruptSettingValue { .. }), "{err}");

        // 负数也一样（JSON 合法但语义非法）
        db.conn()
            .execute(
                "UPDATE app_setting SET value = '-5' WHERE key = 'log.retention_days'",
                [],
            )
            .unwrap();
        assert!(matches!(
            db.setting(SettingKey::LogRetentionDays).unwrap_err(),
            DbError::CorruptSettingValue { .. }
        ));
    }

    // ── 路径 ────────────────────────────────────────────

    #[test]
    fn open_creates_missing_parent_directories() {
        // 首次启动时 %APPDATA%\orbis 还不存在
        let tmp = TempDir::new("db-mkdir");
        let nested = tmp.path().join("a").join("b").join("orbis.db");
        let db = Db::open(&nested).expect("应自动创建父目录");
        assert!(nested.exists());
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn default_db_path_matches_the_documented_layout() {
        assert_eq!(
            paths::db_path().unwrap(),
            paths::data_dir().unwrap().join("orbis.db")
        );
    }
}
