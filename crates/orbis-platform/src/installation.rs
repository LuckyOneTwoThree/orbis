//! 安装实例与启动参数的仓储（`pm/04-技术设计.md` §6.1 / 契约 §3.1、§3.3、§6）。
//!
//! # 为什么单独成模块
//!
//! `db.rs` 负责**建库 + 迁移 + 设置**（schema 与版本的单一来源）；实体仓储随对应域
//! 落地时再加。安装实例是 A 域（游戏管理）的落点，与 schema 演进是两件事，
//! 因此独立成模块，用 `impl Db` 扩展同一个连接上的操作。
//!
//! # 读路径从严：不猜、不兜底
//!
//! 行里的枚举列（`region` / `status` / `version_source` / `added_via`）与
//! `version_norm` 解析失败一律报 [`DbError::CorruptInstallationRow`]，
//! **不**退回默认值。理由见该错误的文档：这些值是我们自己写进去的，
//! 读不出来只可能是外部改动或版本回退，猜一个会让损坏看起来很正常。
//! 这条与内置数据（Manifest / seed）的降级策略刻意相反 —— 那两者是随包分发的数据文件。
//!
//! # `status` 为什么没有 `running`
//!
//! 04 §6.4.1 把「运行态」定为**互斥主状态**，但 `running` 是**会话派生**的：
//! 它来自进程快照（A5），一旦进程退出或应用崩溃，落库的 `running` 立刻成为谎言。
//! 因此本模块用 [`PersistedStatus`] 表达可落库的四态，`running` 在读取时**根本无法表示**
//! （`PersistedStatus::from_slug("running")` 返回 `None`）。
//! 未来若库里真出现 `status = 'running'`（旧版本写入 / 手改），读取会报错而不是
//! 把它当成「正在运行」展示 —— 后者会误导用户以为游戏开着。

use orbis_core::{GameId, GameRuntimeStatus, Region, Version};
use rusqlite::{OptionalExtension, Row};

use crate::db::{Db, DbError};

/// 可落库的运行态（04 §6.1 `installation.status`）。
///
/// 刻意不含 `Running`：见模块文档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PersistedStatus {
    Installed,
    Updating,
    Repairing,
    Broken,
}

impl PersistedStatus {
    pub const ALL: [Self; 4] = [
        Self::Installed,
        Self::Updating,
        Self::Repairing,
        Self::Broken,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Updating => "updating",
            Self::Repairing => "repairing",
            Self::Broken => "broken",
        }
    }

    /// 严格解析；`"running"` 一律 `None`（见模块文档）。
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }

    /// 投影成契约 `GameRuntimeStatus`（契约 §2）。
    ///
    /// **不返回 `Running`**：当前无进程快照（A5 未落地），把落库状态直接当成运行态
    /// 才是唯一诚实的表达。A5 落地后由 `getRuntimeStates` 覆盖这一字段。
    pub const fn to_runtime(self) -> GameRuntimeStatus {
        match self {
            Self::Installed => GameRuntimeStatus::Installed,
            Self::Updating => GameRuntimeStatus::Updating,
            Self::Repairing => GameRuntimeStatus::Repairing,
            Self::Broken => GameRuntimeStatus::Broken,
        }
    }
}

/// 本地版本识别方式（契约 §6 `InstallationDto.versionSource`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSource {
    ExeVersionInfo,
    FeatureFile,
    Directory,
}

impl VersionSource {
    pub const ALL: [Self; 3] = [Self::ExeVersionInfo, Self::FeatureFile, Self::Directory];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::ExeVersionInfo => "exe_versioninfo",
            Self::FeatureFile => "feature_file",
            Self::Directory => "directory",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }
}

/// 实例的来源（契约 §6 `InstallationDto.addedVia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddedVia {
    /// A1 自动发现
    Scan,
    /// A2 手动添加
    Manual,
}

impl AddedVia {
    pub const ALL: [Self; 2] = [Self::Scan, Self::Manual];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Manual => "manual",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }
}

/// `installation` 表的一行（04 §6.1）。
///
/// 字段与列一一对应，**不做展示格式化**（契约 §1：时长/时间的格式化归 UI）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationRecord {
    /// UUID v4（由调用方生成 —— 本层不引入 uuid 依赖，保持纯粹）
    pub id: String,
    pub game_id: String,
    pub region: Region,
    pub install_path: String,
    pub executable_path: String,
    /// 原始识别值；`None` = Unknown（02 A3：不猜）
    pub local_version: Option<String>,
    /// `major.minor`；查询与比较只用它（04 §5.1）
    pub version_norm: Option<Version>,
    pub version_source: Option<VersionSource>,
    pub status: PersistedStatus,
    pub added_via: AddedVia,
    pub created_at: i64,
    pub updated_at: i64,
}

impl InstallationRecord {
    /// 版本未知 = 契约 `InstallationDto.versionUnknown`（02 A3 的判定基础）。
    pub const fn version_unknown(&self) -> bool {
        self.version_norm.is_none()
    }
}

/// `launch_profile` 表的一行（04 §6.1 / A7）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchProfileRecord {
    pub installation_id: String,
    /// 单个字符串；**切分归 Core**（契约 §3.3：引号/空格规则单点实现）
    pub args: String,
    pub updated_at: i64,
}

/// `installation` 的列清单（读取与测试共用一份，避免 `SELECT *` 的列序漂移）。
const INSTALLATION_COLUMNS: &str = "id, game_id, region, install_path, executable_path, \
     local_version, version_norm, version_source, status, added_via, created_at, updated_at";

impl Db {
    /// 全部安装实例。
    ///
    /// 顺序固定为 `created_at, id`：没有任何文档规定 UI 的展示顺序，但**接口输出必须确定**，
    /// 否则同一份数据两次调用可能给出不同顺序，UI 的 diff 与测试都会随机抖动。
    /// 排序口径（按游戏分组、按最近使用等）属于 UI 的决定。
    pub fn installations(&self) -> Result<Vec<InstallationRecord>, DbError> {
        let sql =
            format!("SELECT {INSTALLATION_COLUMNS} FROM installation ORDER BY created_at, id");
        let mut stmt = self.conn().prepare(&sql)?;
        // 两层 Result：外层是 SQLite 的失败，内层是「这一行的内容不合法」。
        // 刻意让内容错误走内层而不是塞进 rusqlite::Error，这样报错能带上是哪一列。
        let rows = stmt.query_map([], |row| Ok(installation_from_row(row)))?;
        let mut out = Vec::new();
        for row in rows {
            let parsed = row?;
            out.push(parsed?);
        }
        Ok(out)
    }

    /// 单个安装实例；不存在 → `None`（调用方据此返回 `GAME_NOT_FOUND`）。
    pub fn installation(&self, id: &str) -> Result<Option<InstallationRecord>, DbError> {
        let sql = format!("SELECT {INSTALLATION_COLUMNS} FROM installation WHERE id = ?1");
        let parsed = self
            .conn()
            .query_row(&sql, [id], |row| Ok(installation_from_row(row)))
            .optional()?;
        parsed.transpose()
    }

    /// 某款游戏**最近添加**的实例（契约 §3.7：`getCompatibility` 的本地版本来源）。
    ///
    /// 「最近」= `created_at` 最大；同一时间戳时用 `id` 兜底，保证结果确定。
    /// 同款多区服（A1）会有多条实例，而 `getCompatibility` 的入参只有 gameId，
    /// 因此需要一个约定的取舍口径 —— 取最近添加的那条。
    pub fn latest_installation_for_game(
        &self,
        game_id: &str,
    ) -> Result<Option<InstallationRecord>, DbError> {
        let sql = format!(
            "SELECT {INSTALLATION_COLUMNS} FROM installation WHERE game_id = ?1 \
             ORDER BY created_at DESC, id DESC LIMIT 1"
        );
        let parsed = self
            .conn()
            .query_row(&sql, [game_id], |row| Ok(installation_from_row(row)))
            .optional()?;
        parsed.transpose()
    }

    /// 插入一条实例。
    ///
    /// 写入路径当前只有测试覆盖：**A2 `addInstallation` 落地时才会被真正调用**，
    /// 而它被实测项 T5/T7（exe 特征 / 路径发现规则）阻塞 —— 先不实现那个命令，
    /// 是为了避免静默接受「用户选到官方启动器而非游戏本体」（02 A2 边界）。
    /// 本方法本身与检测规则无关，因此可以先行落地并测透。
    pub fn insert_installation(&self, record: &InstallationRecord) -> Result<(), DbError> {
        self.conn()
            .execute(
                "INSERT INTO installation
                   (id, game_id, region, install_path, executable_path, local_version,
                    version_norm, version_source, status, added_via, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    record.id,
                    record.game_id,
                    record.region.slug(),
                    record.install_path,
                    record.executable_path,
                    record.local_version,
                    record.version_norm.as_ref().map(ToString::to_string),
                    record.version_source.as_ref().map(|s| s.slug()),
                    record.status.slug(),
                    record.added_via.slug(),
                    record.created_at,
                    record.updated_at,
                ],
            )
            .map(|_| ())
            .map_err(|err| match &err {
                // 本表除了主键 id 之外只有 executable_path 是 UNIQUE，
                // 因此这里的唯一约束冲突只可能是「同路径重复添加」（契约 INSTALLATION_DUPLICATE）。
                // 显式翻译而不是把原始 SQLite 错误往上抛，是为了让壳层不必解析错误文本。
                rusqlite::Error::SqliteFailure(e, _)
                    if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
                {
                    DbError::ExecutablePathTaken {
                        path: record.executable_path.clone(),
                    }
                }
                _ => DbError::from(err),
            })
    }

    /// 删除一条实例。返回是否真的删掉了（`false` = 本来就不存在）。
    ///
    /// `launch_profile` / `playtime_session` / `backup` 的从属行经
    /// `ON DELETE CASCADE` 一并删除（04 §6.1，已有测试守护）。
    /// **游戏文件与存档完全不受影响**（02 A2 验收）—— 本层只碰数据库。
    pub fn delete_installation(&self, id: &str) -> Result<bool, DbError> {
        let affected = self
            .conn()
            .execute("DELETE FROM installation WHERE id = ?1", [id])?;
        Ok(affected > 0)
    }

    // ── 启动参数（A7 / 契约 §3.3）──────────────────────────

    /// 读取启动参数；未设置过 → `None`（**不**返回一条空记录：
    /// 「没有自定义参数」与「有一份空参数」对 UI 是同一件事，但前者不必编造 `updated_at`）。
    pub fn launch_profile(
        &self,
        installation_id: &str,
    ) -> Result<Option<LaunchProfileRecord>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT installation_id, args, updated_at FROM launch_profile WHERE installation_id = ?1",
                [installation_id],
                |row| {
                    Ok(LaunchProfileRecord {
                        installation_id: row.get(0)?,
                        args: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// 写入（或覆盖）启动参数。
    ///
    /// 不存在的 `installation_id` 会撞外键约束并报错 —— 这是**兜底**：
    /// 命令层应当先确认实例存在（否则会把 `GAME_NOT_FOUND` 变成 `INTERNAL`）。
    pub fn set_launch_profile(
        &self,
        installation_id: &str,
        args: &str,
        updated_at: i64,
    ) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO launch_profile(installation_id, args, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(installation_id) DO UPDATE SET args = excluded.args, updated_at = excluded.updated_at",
            rusqlite::params![installation_id, args, updated_at],
        )?;
        Ok(())
    }

    /// 清除自定义启动参数（回到「无参数」）。返回是否真的删掉了行。
    ///
    /// 语义是**删除行**而不是写入空串：契约 §3.3 的 `resetLaunchProfile` 表达的是
    /// 「恢复默认」，删行让「从未设置」与「恢复默认后」在库里是同一种状态。
    pub fn reset_launch_profile(&self, installation_id: &str) -> Result<bool, DbError> {
        let affected = self.conn().execute(
            "DELETE FROM launch_profile WHERE installation_id = ?1",
            [installation_id],
        )?;
        Ok(affected > 0)
    }
}

/// 行 → 记录。枚举列解析失败一律报错（模块文档：读路径从严）。
fn installation_from_row(row: &Row<'_>) -> Result<InstallationRecord, DbError> {
    let id: String = row.get(0)?;
    let corrupt = |column: &'static str, detail: String| DbError::CorruptInstallationRow {
        id: id.clone(),
        column,
        detail,
    };

    let game_id: String = row.get(1)?;
    if GameId::new(&game_id).is_none() {
        return Err(corrupt("game_id", format!("须为合法 slug：{game_id:?}")));
    }

    let region_raw: String = row.get(2)?;
    let region = Region::from_slug(&region_raw)
        .ok_or_else(|| corrupt("region", format!("未知区服：{region_raw:?}")))?;

    let version_norm_raw: Option<String> = row.get(6)?;
    let version_norm = match version_norm_raw {
        None => None,
        Some(raw) => Some(
            Version::parse(&raw)
                .ok_or_else(|| corrupt("version_norm", format!("须为 major.minor：{raw:?}")))?,
        ),
    };

    let version_source = match row.get::<_, Option<String>>(7)? {
        None => None,
        Some(raw) => Some(
            VersionSource::from_slug(&raw)
                .ok_or_else(|| corrupt("version_source", format!("未知识别方式：{raw:?}")))?,
        ),
    };

    let status_raw: String = row.get(8)?;
    let status = PersistedStatus::from_slug(&status_raw).ok_or_else(|| {
        corrupt(
            "status",
            format!("未知状态：{status_raw:?}（running 由进程快照派生，不落库）"),
        )
    })?;

    let added_via_raw: String = row.get(9)?;
    let added_via = AddedVia::from_slug(&added_via_raw)
        .ok_or_else(|| corrupt("added_via", format!("未知来源：{added_via_raw:?}")))?;

    Ok(InstallationRecord {
        id,
        game_id,
        region,
        install_path: row.get(3)?,
        executable_path: row.get(4)?,
        local_version: row.get(5)?,
        version_norm,
        version_source,
        status,
        added_via,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// 冻结时间戳（04 §6.1 的 `*_at` 都是 INTEGER）。
    const TS: i64 = 1_758_000_000;

    fn sample(id: &str, exe: &str) -> InstallationRecord {
        InstallationRecord {
            id: id.to_owned(),
            game_id: "sample-game".to_owned(),
            region: Region::Cn,
            install_path: "C:/sample".to_owned(),
            executable_path: exe.to_owned(),
            local_version: Some("3.5.0.128940".to_owned()),
            version_norm: Version::parse("3.5"),
            version_source: Some(VersionSource::ExeVersionInfo),
            status: PersistedStatus::Installed,
            added_via: AddedVia::Manual,
            created_at: TS,
            updated_at: TS,
        }
    }

    #[test]
    fn insert_then_read_roundtrips_every_column() {
        let db = Db::open_in_memory().unwrap();
        let record = sample("i1", "C:/sample/game.exe");
        db.insert_installation(&record).unwrap();

        let read = db.installation("i1").unwrap().expect("应能读回");
        assert_eq!(read, record, "逐列往返必须无损（含 Option 列）");
        assert!(!read.version_unknown());
    }

    #[test]
    fn unknown_versions_roundtrip_as_none_not_empty_string() {
        // 02 A3：无法识别就是 Unknown，不得用空串或 "Unknown" 字面量占位
        let db = Db::open_in_memory().unwrap();
        let mut record = sample("i1", "C:/sample/game.exe");
        record.local_version = None;
        record.version_norm = None;
        record.version_source = None;
        db.insert_installation(&record).unwrap();

        let read = db.installation("i1").unwrap().unwrap();
        assert_eq!(read.local_version, None);
        assert_eq!(read.version_norm, None);
        assert_eq!(read.version_source, None);
        assert!(read.version_unknown());
    }

    #[test]
    fn listing_is_ordered_by_created_at_then_id() {
        // 输出确定性：没有 ORDER BY 的查询会让 UI 与测试随机抖动
        let db = Db::open_in_memory().unwrap();
        let mut later = sample("i9", "C:/b/game.exe");
        later.created_at = TS + 100;
        db.insert_installation(&later).unwrap();
        db.insert_installation(&sample("i2", "C:/a/game.exe"))
            .unwrap();

        let ids: Vec<String> = db
            .installations()
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec!["i2", "i9"]);
    }

    #[test]
    fn latest_installation_is_picked_per_game() {
        // 契约 §3.7：getCompatibility 只收到 gameId，因此需要一个确定的取舍口径
        let db = Db::open_in_memory().unwrap();

        let mut older = sample("i1", "C:/a/game.exe");
        older.created_at = TS;
        let mut newer = sample("i2", "C:/b/game.exe");
        newer.created_at = TS + 100;
        newer.version_norm = Version::parse("4.0");
        let mut other = sample("i3", "C:/c/game.exe");
        other.game_id = "other-game".to_owned();
        other.created_at = TS + 999;

        for record in [&older, &newer, &other] {
            db.insert_installation(record).unwrap();
        }

        let latest = db
            .latest_installation_for_game("sample-game")
            .unwrap()
            .expect("应有实例");
        assert_eq!(latest.id, "i2", "应取 created_at 最大的那条");
        assert_eq!(latest.version_norm, Version::parse("4.0"));
        assert_eq!(
            db.latest_installation_for_game("other-game")
                .unwrap()
                .unwrap()
                .id,
            "i3",
            "另一款游戏的更晚实例不得串味"
        );
        assert!(db
            .latest_installation_for_game("absent-game")
            .unwrap()
            .is_none());
    }

    #[test]
    fn duplicate_executable_path_is_reported_as_taken() {
        // 契约 INSTALLATION_DUPLICATE：同路径重复添加必须能被识别，
        // 而不是抛一个壳层看不懂的原始 SQLite 错误
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&sample("i1", "C:/sample/game.exe"))
            .unwrap();
        let err = db
            .insert_installation(&sample("i2", "C:/sample/game.exe"))
            .unwrap_err();
        match err {
            DbError::ExecutablePathTaken { path } => assert_eq!(path, "C:/sample/game.exe"),
            other => panic!("应报路径被占用，实际：{other}"),
        }
    }

    #[test]
    fn delete_reports_whether_a_row_existed_and_cascades() {
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&sample("i1", "C:/sample/game.exe"))
            .unwrap();
        db.set_launch_profile("i1", "--x", TS).unwrap();

        assert!(db.delete_installation("i1").unwrap());
        assert!(
            !db.delete_installation("i1").unwrap(),
            "重复删除应返回 false，而不是报错"
        );
        assert!(db.installation("i1").unwrap().is_none());
        assert!(
            db.launch_profile("i1").unwrap().is_none(),
            "从属行应随实例级联删除"
        );
    }

    #[test]
    fn launch_profile_is_absent_until_written_then_overwritten() {
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&sample("i1", "C:/sample/game.exe"))
            .unwrap();

        // 未设置过 → None（不编造 updated_at）
        assert!(db.launch_profile("i1").unwrap().is_none());

        db.set_launch_profile("i1", "-windowed", TS).unwrap();
        let profile = db.launch_profile("i1").unwrap().unwrap();
        assert_eq!(profile.args, "-windowed");
        assert_eq!(profile.updated_at, TS);

        // 再写一次是覆盖，不是插入（主键冲突不得外泄）
        db.set_launch_profile("i1", "-fullscreen", TS + 5).unwrap();
        let profile = db.launch_profile("i1").unwrap().unwrap();
        assert_eq!(profile.args, "-fullscreen");
        assert_eq!(profile.updated_at, TS + 5);

        // reset 是删行，语义是「回到默认」而不是「存一个空串」
        assert!(db.reset_launch_profile("i1").unwrap());
        assert!(db.launch_profile("i1").unwrap().is_none());
        assert!(!db.reset_launch_profile("i1").unwrap());
    }

    #[test]
    fn launch_profile_for_an_unknown_installation_fails_the_foreign_key() {
        // 兜底：命令层应先判 GAME_NOT_FOUND；这里只确保库不会静默接受悬空行
        let db = Db::open_in_memory().unwrap();
        assert!(db.set_launch_profile("ghost", "--x", TS).is_err());
    }

    #[test]
    fn corrupt_enum_columns_are_reported_not_defaulted() {
        // 手改库 / 旧版本写入：必须报错。退回默认值会让损坏看起来像正常数据
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&sample("i1", "C:/sample/game.exe"))
            .unwrap();

        for (column, value) in [
            ("region", "mars"),
            ("status", "flying"),
            ("added_via", "teleport"),
            ("version_source", "crystal-ball"),
            ("version_norm", "3"),     // 不是 major.minor
            ("game_id", "Not A Slug"), // 非法 slug
        ] {
            db.conn()
                .execute(
                    &format!("UPDATE installation SET {column} = ?1 WHERE id = 'i1'"),
                    params![value],
                )
                .unwrap();
            match db.installation("i1") {
                Err(DbError::CorruptInstallationRow { column: c, .. }) => {
                    assert_eq!(c, column, "报错应指向出错的列");
                }
                other => panic!("列 {column}={value:?} 应被拒绝，实际：{other:?}"),
            }
            // 还原，避免影响下一轮
            db.conn()
                .execute(
                    &format!("UPDATE installation SET {column} = ?1 WHERE id = 'i1'"),
                    params![match column {
                        "region" => "cn",
                        "status" => "installed",
                        "added_via" => "manual",
                        "version_source" => "exe_versioninfo",
                        "version_norm" => "3.5",
                        _ => "sample-game",
                    }],
                )
                .unwrap();
        }
    }

    #[test]
    fn running_is_not_a_persistable_status() {
        // 04 §6.4.1：running 由进程快照派生。落库的 running 在重启后必然是谎言，
        // 因此解析不出来（旧版本写入时读路径会报错，而不是显示成「正在运行」）
        assert_eq!(PersistedStatus::from_slug("running"), None);
        for status in PersistedStatus::ALL {
            assert_eq!(PersistedStatus::from_slug(status.slug()), Some(status));
            assert_ne!(
                status.to_runtime(),
                GameRuntimeStatus::Running,
                "落库状态不得投影成运行态"
            );
        }

        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&sample("i1", "C:/sample/game.exe"))
            .unwrap();
        db.conn()
            .execute(
                "UPDATE installation SET status = 'running' WHERE id = 'i1'",
                [],
            )
            .unwrap();
        assert!(matches!(
            db.installation("i1"),
            Err(DbError::CorruptInstallationRow {
                column: "status",
                ..
            })
        ));
    }

    #[test]
    fn status_and_vocabulary_slugs_match_the_contract() {
        // 契约 §2 / §6 的字面量（入库值直接跨 IPC，写错会在前端静默落空）
        assert_eq!(PersistedStatus::Installed.slug(), "installed");
        assert_eq!(PersistedStatus::Broken.slug(), "broken");
        assert_eq!(VersionSource::ExeVersionInfo.slug(), "exe_versioninfo");
        assert_eq!(VersionSource::Directory.slug(), "directory");
        assert_eq!(AddedVia::Scan.slug(), "scan");
        assert_eq!(AddedVia::Manual.slug(), "manual");
        // 区服一律小写，禁止展示文案入库（契约 §2）
        for region in Region::ALL {
            assert_eq!(Region::from_slug(region.slug()), Some(region));
        }
    }
}
