//! 备份记录的仓储与文件操作（`pm/04-技术设计.md` §5.4 / §6.1，契约 §3.8）。
//!
//! # 本模块只负责「已经存在的备份」
//!
//! 04 §5.4 把 `BackupManager` 分成四件事：`create` / `restore` / `delete` / 存储布局。
//! 其中 **`create` 与 `restore` 依赖 `declared_paths`**（provider 声明「该备份哪些文件」），
//! 而那组路径属于实测项 T3/T6 的范围 —— 凭推测填出来的文件清单会**备份不到该备份的东西**，
//! 比不备份更危险（用户以为有备份）。因此：
//!
//! - **已落地**：记录的读取与统计、文件删除（数据来源 = `backup` 表 + §5.4 的存储布局，两者均已定稿）
//! - **未落地**：`create` / `restore`（等 T3/T6）
//!
//! # 为什么读取路径从严
//!
//! `trigger` 列解析失败一律报 [`DbError::CorruptBackupRow`]，不退回默认值 ——
//! 与 `installation` 同一立场：这些行是我们自己写的，读不出来只可能是外部改动或版本回退，
//! 猜一个值等于把损坏伪装成正常。
//!
//! # `manifest_json` 暂不解析
//!
//! 列表投影只需要 `file_count` / `total_bytes`（契约 §3.8 的 `BackupSummary`），
//! 不需要逐个文件。而 `primaryFile` 的**选中口径尚未定稿**（见 [`Db::backups`] 的调用方），
//! 所以现在解析 `manifest_json` 只会得到一个没有依据的结论。`restore` 落地时再解析它，
//! 那时会连带做逐文件哈希校验。

use std::path::Path;

use rusqlite::{OptionalExtension, Row};

use crate::db::{Db, DbError};

/// 备份触发方式（契约 §2 `BackupTrigger` / 04 §5.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackupTrigger {
    /// 用户手动创建
    Manual,
    /// 修改配置前的自动备份（B2：无备份不修改）
    PreModify,
    /// 恢复前的自动备份（02 A8：任何恢复本身可撤销）
    PreRestore,
}

impl BackupTrigger {
    pub const ALL: [Self; 3] = [Self::Manual, Self::PreModify, Self::PreRestore];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::PreModify => "pre_modify",
            Self::PreRestore => "pre_restore",
        }
    }

    /// 严格解析；未知取值 → `None`（调用方转成 `CorruptBackupRow`）。
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.slug() == slug)
    }
}

/// `backup` 表的一行（04 §6.1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRecord {
    pub id: String,
    pub installation_id: String,
    pub game_id: String,
    /// `None` = 手动备份（契约 §6 `toolId: string | null`）
    pub tool_id: Option<String>,
    pub trigger: BackupTrigger,
    pub file_count: i64,
    pub total_bytes: i64,
    /// 原始 manifest（`files[]: rel_path / sha256 / size`）。见模块文档：列表投影不消费它。
    pub manifest_json: String,
    pub created_at: i64,
}

const BACKUP_COLUMNS: &str = "id, installation_id, game_id, tool_id, trigger, \
     file_count, total_bytes, manifest_json, created_at";

fn backup_from_row(row: &Row<'_>) -> Result<BackupRecord, DbError> {
    let id: String = row.get(0)?;
    let trigger_raw: String = row.get(4)?;
    let trigger =
        BackupTrigger::from_slug(&trigger_raw).ok_or_else(|| DbError::CorruptBackupRow {
            id: id.clone(),
            column: "trigger",
            detail: format!("未知取值 {trigger_raw:?}"),
        })?;
    Ok(BackupRecord {
        id,
        installation_id: row.get(1)?,
        game_id: row.get(2)?,
        tool_id: row.get(3)?,
        trigger,
        file_count: row.get(5)?,
        total_bytes: row.get(6)?,
        manifest_json: row.get(7)?,
        created_at: row.get(8)?,
    })
}

impl Db {
    /// 某实例的全部备份，最近的在前（与 §6.1 的索引 `idx_backup_install` 同序）。
    ///
    /// 实例不存在或还没有备份 → 空列表。**不报错**：契约 §3.8 的错误码表里没有
    /// `GAME_NOT_FOUND`，而「这个实例没有备份」本身就是合法状态。
    ///
    /// 返回的记录里不含 `primaryFile` 所需的判断 —— 那是投影层的事，见
    /// `src-tauri/src/dto.rs` 的同名说明（口径未定稿，故先给 `null`）。
    pub fn backups(&self, installation_id: &str) -> Result<Vec<BackupRecord>, DbError> {
        let sql = format!(
            "SELECT {BACKUP_COLUMNS} FROM backup WHERE installation_id = ?1 \
             ORDER BY created_at DESC, id"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        // 两层 Result：外层是 SQLite 的失败，内层是「这一行的内容不合法」。
        // 让内容错误走内层，报错才能带上是哪一列（与 installation 仓储同构）。
        let rows = stmt.query_map([installation_id], |row| Ok(backup_from_row(row)))?;
        let mut out = Vec::new();
        for row in rows {
            let parsed = row?;
            out.push(parsed?);
        }
        Ok(out)
    }

    /// 单个备份；不存在 → `None`（调用方据此返回 `BACKUP_NOT_FOUND`）。
    pub fn backup(&self, id: &str) -> Result<Option<BackupRecord>, DbError> {
        let sql = format!("SELECT {BACKUP_COLUMNS} FROM backup WHERE id = ?1");
        let parsed = self
            .conn()
            .query_row(&sql, [id], |row| Ok(backup_from_row(row)))
            .optional()?;
        parsed.transpose()
    }

    /// 某实例的备份条数与总字节数（`getBackupStorageInfo` 的两个字段）。
    pub fn backup_totals(&self, installation_id: &str) -> Result<(i64, i64), DbError> {
        let totals = self.conn().query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_bytes), 0) FROM backup WHERE installation_id = ?1",
            [installation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(totals)
    }

    /// 删除一条备份记录。返回是否真的删掉了（`false` = 本来就不存在）。
    ///
    /// 调用方必须**先删文件、后删记录** —— 见 [`remove_dir_if_exists`] 的文档。
    pub fn delete_backup_row(&self, id: &str) -> Result<bool, DbError> {
        let affected = self
            .conn()
            .execute("DELETE FROM backup WHERE id = ?1", [id])?;
        Ok(affected > 0)
    }
}

/// 删除一个目录（含内容）；目录不存在 → `Ok(false)`。
///
/// **幂等是刻意的**：`deleteBackup` 的正确顺序是「先删文件、后删记录」，
/// 于是「文件删掉了但记录删除失败」的重试会走到这里，此时目录已经不在 ——
/// 若把 NotFound 当成错误，那条记录就永远删不掉了，而文件其实早已清理。
///
/// 反序（先删记录）则是错的：一旦文件删除失败，记录已经消失，
/// 重试只会得到 `BACKUP_NOT_FOUND`，那些文件就永远留在磁盘上、也不再出现在任何列表里。
pub fn remove_dir_if_exists(dir: &Path) -> std::io::Result<bool> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

/// 指定路径所在磁盘的可用字节数；无法判定 → `None`。
///
/// 取**最长匹配**的挂载点：Windows 上挂载点是 `C:\` 这类卷根，而其它平台存在挂到子目录的
/// 卷，取最长匹配才不会把子卷的可用空间算成父卷的。
pub fn free_disk_bytes(path: &Path) -> Option<u64> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| disk.available_space())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::installation::{AddedVia, InstallationRecord, PersistedStatus};
    use orbis_core::Region;

    /// 独立临时目录（避免依赖 `%APPDATA%`，那会让测试互相干扰）。
    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let unique = format!(
            "orbis-backup-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        std::env::temp_dir().join(unique)
    }

    fn installation(id: &str) -> InstallationRecord {
        InstallationRecord {
            id: id.to_owned(),
            game_id: "wuthering-waves".to_owned(),
            region: Region::Cn,
            install_path: "C:/games/wuwa".to_owned(),
            executable_path: format!("C:/games/wuwa/{id}.exe"),
            local_version: None,
            version_norm: None,
            version_source: None,
            status: PersistedStatus::Installed,
            added_via: AddedVia::Manual,
            created_at: 1_758_000_000_000,
            updated_at: 1_758_000_000_000,
        }
    }

    fn insert_backup(db: &Db, id: &str, installation_id: &str, trigger: &str, bytes: i64) {
        db.conn()
            .execute(
                "INSERT INTO backup(id, installation_id, game_id, tool_id, trigger, \
                 file_count, total_bytes, manifest_json, created_at) \
                 VALUES (?1, ?2, 'wuthering-waves', NULL, ?3, 3, ?4, '{\"files\":[]}', 1758000000000)",
                rusqlite::params![id, installation_id, trigger, bytes],
            )
            .expect("插入备份记录应成功");
    }

    #[test]
    fn trigger_round_trips_through_its_slug() {
        for trigger in BackupTrigger::ALL {
            assert_eq!(BackupTrigger::from_slug(trigger.slug()), Some(trigger));
        }
        assert_eq!(BackupTrigger::from_slug("something_else"), None);
    }

    #[test]
    fn backups_are_listed_newest_first_and_scoped_to_the_installation() {
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&installation("i1")).unwrap();
        db.insert_installation(&installation("i2")).unwrap();
        insert_backup(&db, "b-old", "i1", "manual", 10);
        db.conn()
            .execute(
                "INSERT INTO backup(id, installation_id, game_id, trigger, file_count, \
                 total_bytes, manifest_json, created_at) \
                 VALUES ('b-new', 'i1', 'wuthering-waves', 'pre_modify', 1, 20, '{}', 1758000001000)",
                [],
            )
            .unwrap();
        insert_backup(&db, "b-other", "i2", "manual", 999);

        let listed = db.backups("i1").unwrap();
        let ids: Vec<&str> = listed.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["b-new", "b-old"], "最近的在前，且不得混入其它实例");

        assert_eq!(listed[0].trigger, BackupTrigger::PreModify);
        assert_eq!(listed[1].tool_id, None, "NULL tool_id = 手动备份");
    }

    #[test]
    fn a_corrupt_trigger_is_an_error_not_a_default() {
        // 与 installation 同一立场：这些行由我们写入，读不出来是损坏，不是「未设置」
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&installation("i1")).unwrap();
        insert_backup(&db, "b1", "i1", "not_a_trigger", 1);

        let err = db.backups("i1").unwrap_err();
        assert!(
            matches!(err, DbError::CorruptBackupRow { .. }),
            "非法 trigger 必须报错：{err}"
        );
    }

    #[test]
    fn totals_are_per_installation_and_zero_when_empty() {
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&installation("i1")).unwrap();
        db.insert_installation(&installation("i2")).unwrap();

        assert_eq!(
            db.backup_totals("i1").unwrap(),
            (0, 0),
            "没有备份时是 0，不是错误"
        );
        insert_backup(&db, "b1", "i1", "manual", 100);
        insert_backup(&db, "b2", "i1", "manual", 250);
        insert_backup(&db, "b3", "i2", "manual", 7);

        assert_eq!(db.backup_totals("i1").unwrap(), (2, 350));
        assert_eq!(db.backup_totals("i2").unwrap(), (1, 7));
    }

    #[test]
    fn deleting_a_row_reports_whether_it_existed() {
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&installation("i1")).unwrap();
        insert_backup(&db, "b1", "i1", "manual", 5);

        assert!(db.delete_backup_row("b1").unwrap());
        assert!(
            !db.delete_backup_row("b1").unwrap(),
            "第二次删除应报告「本就不存在」"
        );
        assert!(db.backups("i1").unwrap().is_empty());
    }

    #[test]
    fn removing_files_is_idempotent() {
        let dir = scratch_dir("remove");
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/manifest.json"), b"{}").unwrap();

        assert!(remove_dir_if_exists(&dir).unwrap(), "首次应真的删掉");
        assert!(!dir.exists(), "目录应已消失");
        assert!(
            !remove_dir_if_exists(&dir).unwrap(),
            "再次调用不得报错 —— 重试路径依赖这个幂等性"
        );
    }

    #[test]
    fn free_disk_space_is_reported_for_the_temp_directory() {
        // 不断言具体数值（各机器不同），只确认「能判定、且是个非零值」
        let space = free_disk_bytes(&std::env::temp_dir());
        assert!(
            space.is_some_and(|bytes| bytes > 0),
            "应能取到临时目录所在卷的可用空间"
        );
    }
}
