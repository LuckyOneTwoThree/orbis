//! 游戏时长记录（A6 / `pm/04-技术设计.md` §5.10 / 契约 §3.4）。
//!
//! # 口径（02 A6 边界，UI 必须明示）
//!
//! 时长 = **游戏进程存活时长**，不是「Orbis 运行时长」也不是「启动次数」。
//! 因此本模块只在进程快照表明实例在跑的时候累积，与窗口是否打开无关。
//!
//! # 三件容易做错的事
//!
//! 1. **崩溃不丢**：§5.10 要求每 30s 把进行中的会话落库。会话的 `ended_at` 为 `NULL`
//!    表示「仍在进行」；若 Orbis 被杀，重启后 [`close_orphaned_sessions`] 会把它们
//!    按 `checkpoint_at` 结算 —— 否则那些会话会一直「进行中」，聚合时算出一个
//!    从崩溃前到现在的巨大时长。
//! 2. **日界归属**：会话可能跨日（22:00 玩到次日 2:00）。简单的做法是按
//!    `started_at` 整段归到开始那天，但那样「今日时长」在跨日时会明显失真。
//!    这里用**区间重叠**（[`overlap_sec`]）把会话切成落到统计窗口内的那部分。
//! 3. **本地时区**：「今日 / 本周」按本地自然日界（04 §5.10 / 契约 §3.4）。
//!    「本周」起点 = **周一**（ISO 8601，04 §5.10 定稿）。

use std::collections::HashMap;

use chrono::{Datelike, Duration, Local, TimeZone};

use crate::db::{Db, DbError};
use crate::installation::InstallationRecord;

/// 一次会话在观测中的状态变化（供调用方写 D3 日志）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackedEvent {
    Started {
        installation_id: String,
        session_id: String,
    },
    Ended {
        installation_id: String,
        session_id: String,
        duration_sec: i64,
    },
}

#[derive(Debug)]
struct OpenSession {
    id: String,
    started_at: i64,
    checkpoint_at: i64,
}

/// 进行中的会话（内存态）。
///
/// **内存态是有意的**：进程是否在跑这件事只有当前进程知道，落库的会话行只是
/// 崩溃恢复用的检查点。因此重启后内存态从空开始，由孤儿会话封存逻辑接管。
#[derive(Debug, Default)]
pub struct PlaytimeTracker {
    open: HashMap<String, OpenSession>,
}

impl PlaytimeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前进行中的会话数（诊断用）。
    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    /// 观察一次运行态，推进会话状态。
    ///
    /// - `is_running`：该 `installation_id` 此刻是否有进程（由 A5 的进程快照得出）
    /// - `checkpoint_sec`：落库间隔，来自 `app_setting.playtime.checkpoint_sec`（默认 30）
    ///
    /// 返回本轮发生的开 / 关事件。**数据库写失败会中断本轮并上报** —— 时长是用户
    /// 可感知的数据，静默丢弃等于欺骗（04 §8）。
    pub fn observe(
        &mut self,
        records: &[InstallationRecord],
        is_running: impl Fn(&str) -> bool,
        db: &Db,
        now_ms: i64,
        checkpoint_sec: u32,
    ) -> Result<Vec<TrackedEvent>, DbError> {
        let mut events = Vec::new();
        let checkpoint_ms = i64::from(checkpoint_sec.max(1)) * 1_000;

        for record in records {
            let running = is_running(&record.id);
            match (running, self.open.contains_key(&record.id)) {
                // 进程出现 → 开新会话
                (true, false) => {
                    let session = OpenSession {
                        id: session_id(&record.id, now_ms),
                        started_at: now_ms,
                        checkpoint_at: now_ms,
                    };
                    db.insert_playtime_session(
                        &session.id,
                        &record.id,
                        session.started_at,
                        session.checkpoint_at,
                    )?;
                    events.push(TrackedEvent::Started {
                        installation_id: record.id.clone(),
                        session_id: session.id.clone(),
                    });
                    self.open.insert(record.id.clone(), session);
                }
                // 仍在跑 → 到点就推进检查点
                (true, true) => {
                    if let Some(session) = self.open.get_mut(&record.id) {
                        if now_ms - session.checkpoint_at >= checkpoint_ms {
                            session.checkpoint_at = now_ms;
                            db.checkpoint_playtime_session(
                                &session.id,
                                now_ms,
                                duration_sec(session.started_at, now_ms),
                            )?;
                        }
                    }
                }
                // 进程消失 → 结算
                (false, true) => {
                    if let Some(session) = self.open.remove(&record.id) {
                        let seconds = duration_sec(session.started_at, now_ms);
                        db.end_playtime_session(&session.id, now_ms, seconds)?;
                        events.push(TrackedEvent::Ended {
                            installation_id: record.id.clone(),
                            session_id: session.id,
                            duration_sec: seconds,
                        });
                    }
                }
                (false, false) => {}
            }
        }

        Ok(events)
    }

    /// 放弃内存中的会话（不写库）。仅用于测试与极端降级路径。
    pub fn forget_open_sessions(&mut self) {
        self.open.clear();
    }
}

/// 会话 ID：`<installation_id>@<started_at_ms>`。
///
/// 不用 UUID：同一个实例在同一毫秒不可能开出两个会话（会话是进程存活的映射），
/// 因此这个组合天然唯一，而且出问题时**一眼能看出是哪个实例哪一刻**。
fn session_id(installation_id: &str, started_at_ms: i64) -> String {
    format!("{installation_id}@{started_at_ms}")
}

fn duration_sec(started_at: i64, ended_at: i64) -> i64 {
    ((ended_at - started_at).max(0)) / 1_000
}

/// 封存孤儿会话（启动时调用）。
///
/// `ended_at IS NULL` 的会话意味着「上次进程没能正常收尾」（崩溃 / 被杀）。
/// 按 `checkpoint_at` 结算 —— 这是 30s 检查点的意义所在：**最多丢 30 秒**，
/// 而不是整场时长。返回被封存的条数。
pub fn close_orphaned_sessions(db: &Db) -> Result<usize, DbError> {
    db.close_orphaned_playtime_sessions()
}

/// 统计窗口内每个实例的时长。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaytimeRow {
    pub installation_id: String,
    pub game_id: String,
    pub seconds: i64,
}

/// 会话与统计窗口的重叠秒数。
///
/// 这是「自然日界聚合」的准确含义：跨日会话只贡献落在窗口内的那部分。
fn overlap_sec(session_start: i64, session_end: i64, window_start: i64, window_end: i64) -> i64 {
    let lo = session_start.max(window_start);
    let hi = session_end.min(window_end);
    if hi <= lo {
        0
    } else {
        (hi - lo) / 1_000
    }
}

/// 聚合 `[since_ms, now_ms)` 内各实例的时长（`since_ms = None` 表示全部历史）。
///
/// 进行中的会话按 `now_ms` 截断；已结算的用 `ended_at`。
pub fn playtime_rows(
    db: &Db,
    since_ms: Option<i64>,
    now_ms: i64,
    installation_id: Option<&str>,
) -> Result<Vec<PlaytimeRow>, DbError> {
    let sessions = db.playtime_sessions(installation_id)?;
    let mut totals: Vec<PlaytimeRow> = Vec::new();

    for session in sessions {
        let ended = session.ended_at.unwrap_or(now_ms);
        let seconds = match since_ms {
            Some(window_start) => overlap_sec(session.started_at, ended, window_start, now_ms),
            // 全部历史：整段计入（不再截断到 now，因为 ended 已经处理过）
            None => duration_sec(session.started_at, ended),
        };
        if seconds <= 0 {
            continue;
        }
        match totals
            .iter_mut()
            .find(|row| row.installation_id == session.installation_id)
        {
            Some(row) => row.seconds += seconds,
            None => totals.push(PlaytimeRow {
                installation_id: session.installation_id.clone(),
                game_id: session.game_id.clone(),
                seconds,
            }),
        }
    }

    Ok(totals)
}

// ── 仓储（`playtime_session`，04 §6.1）────────────────────

/// `playtime_session` 的一行（含 JOIN 出来的 `game_id`，聚合时要用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaytimeSessionRecord {
    pub id: String,
    pub installation_id: String,
    pub game_id: String,
    pub started_at: i64,
    pub checkpoint_at: i64,
    /// `None` = 会话仍在进行（04 §6.1 允许 NULL）
    pub ended_at: Option<i64>,
    pub duration_sec: i64,
}

impl Db {
    /// 开一个会话（进程出现时）。
    pub fn insert_playtime_session(
        &self,
        id: &str,
        installation_id: &str,
        started_at: i64,
        checkpoint_at: i64,
    ) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO playtime_session(id, installation_id, started_at, checkpoint_at, duration_sec)
             VALUES (?1, ?2, ?3, ?4, 0)",
            rusqlite::params![id, installation_id, started_at, checkpoint_at],
        )?;
        Ok(())
    }

    /// 推进检查点（会话仍在进行）。
    pub fn checkpoint_playtime_session(
        &self,
        id: &str,
        checkpoint_at: i64,
        duration_sec: i64,
    ) -> Result<(), DbError> {
        self.conn().execute(
            "UPDATE playtime_session SET checkpoint_at = ?2, duration_sec = ?3 WHERE id = ?1",
            rusqlite::params![id, checkpoint_at, duration_sec],
        )?;
        Ok(())
    }

    /// 结算会话（进程消失时）。
    pub fn end_playtime_session(
        &self,
        id: &str,
        ended_at: i64,
        duration_sec: i64,
    ) -> Result<(), DbError> {
        self.conn().execute(
            "UPDATE playtime_session SET ended_at = ?2, checkpoint_at = ?2, duration_sec = ?3 WHERE id = ?1",
            rusqlite::params![id, ended_at, duration_sec],
        )?;
        Ok(())
    }

    /// 把所有仍未结算的会话按最后一个检查点封存，返回条数（崩溃恢复，见模块文档）。
    pub fn close_orphaned_playtime_sessions(&self) -> Result<usize, DbError> {
        let affected = self.conn().execute(
            "UPDATE playtime_session
                SET ended_at = checkpoint_at,
                    duration_sec = MAX(0, (checkpoint_at - started_at) / 1000)
              WHERE ended_at IS NULL",
            [],
        )?;
        Ok(affected)
    }

    /// 读取会话（可按实例过滤）。
    ///
    /// 这里**不做区间过滤**：跨日会话的部分归属需要按区间重叠计算
    /// （[`playtime_rows`]），在 SQL 里做会把它变成一条难以验证的表达式。
    pub fn playtime_sessions(
        &self,
        installation_id: Option<&str>,
    ) -> Result<Vec<PlaytimeSessionRecord>, DbError> {
        let mut stmt = self.conn().prepare(
            "SELECT s.id, s.installation_id, i.game_id, s.started_at, s.checkpoint_at,
                    s.ended_at, s.duration_sec
               FROM playtime_session s
               JOIN installation i ON i.id = s.installation_id
              WHERE (?1 IS NULL OR s.installation_id = ?1)
              ORDER BY s.started_at, s.id",
        )?;
        let rows = stmt.query_map([installation_id], |row| {
            Ok(PlaytimeSessionRecord {
                id: row.get(0)?,
                installation_id: row.get(1)?,
                game_id: row.get(2)?,
                started_at: row.get(3)?,
                checkpoint_at: row.get(4)?,
                ended_at: row.get(5)?,
                duration_sec: row.get(6)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

// ── 本地时区日界 ──────────────────────────────────────────

/// 本地时区下「今天 00:00」的 epoch 毫秒。
pub fn local_day_start_ms(now_ms: i64) -> i64 {
    let Some(moment) = Local.timestamp_millis_opt(now_ms).single() else {
        return now_ms;
    };
    let Some(midnight) = moment.date_naive().and_hms_opt(0, 0, 0) else {
        return now_ms;
    };
    // 夏令时切换的极罕见情形下午夜可能不存在 → 取最早的那个候选
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(now_ms)
}

/// 本地时区下「本周周一 00:00」的 epoch 毫秒（ISO 8601：一周从周一开始）。
pub fn local_week_start_ms(now_ms: i64) -> i64 {
    let Some(moment) = Local.timestamp_millis_opt(now_ms).single() else {
        return now_ms;
    };
    let monday =
        moment.date_naive() - Duration::days(i64::from(moment.weekday().num_days_from_monday()));
    let Some(midnight) = monday.and_hms_opt(0, 0, 0) else {
        return now_ms;
    };
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::installation::{AddedVia, PersistedStatus};
    use orbis_core::Region;

    const TS: i64 = 1_758_000_000_000;

    fn record(id: &str) -> InstallationRecord {
        InstallationRecord {
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
            created_at: TS,
            updated_at: TS,
        }
    }

    /// 只把给定 id 视为「在跑」（own 一份数据，避免给返回的闭包标注生命周期）。
    fn running_only(ids: &[&str]) -> impl Fn(&str) -> bool {
        let ids: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
        move |id: &str| ids.iter().any(|known| known == id)
    }

    #[test]
    fn a_process_appearing_opens_a_session_and_disappearing_closes_it() {
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&record("i1")).unwrap();
        let records = db.installations().unwrap();
        let mut tracker = PlaytimeTracker::new();

        // 出现 → 开会话
        let events = tracker
            .observe(&records, running_only(&["i1"]), &db, TS, 30)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TrackedEvent::Started { .. }));
        assert_eq!(tracker.open_count(), 1);

        // 消失（+90 秒）→ 结算
        let events = tracker
            .observe(&records, running_only(&[]), &db, TS + 90_000, 30)
            .unwrap();
        assert_eq!(
            events,
            vec![TrackedEvent::Ended {
                installation_id: "i1".to_owned(),
                session_id: session_id("i1", TS),
                duration_sec: 90,
            }]
        );
        assert_eq!(tracker.open_count(), 0);

        // 落库的是完整会话
        let rows = playtime_rows(&db, None, TS + 90_000, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].seconds, 90);
        assert_eq!(rows[0].game_id, "sample-game");
    }

    #[test]
    fn checkpoints_are_written_at_the_configured_interval() {
        // 04 §5.10：每 checkpoint_sec 落库一次，崩溃时最多丢一个间隔
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&record("i1")).unwrap();
        let records = db.installations().unwrap();
        let mut tracker = PlaytimeTracker::new();

        tracker
            .observe(&records, running_only(&["i1"]), &db, TS, 30)
            .unwrap();

        // 未到间隔 → 不写
        tracker
            .observe(&records, running_only(&["i1"]), &db, TS + 29_000, 30)
            .unwrap();
        assert_eq!(db.playtime_sessions(None).unwrap()[0].duration_sec, 0);

        // 到点 → 写
        tracker
            .observe(&records, running_only(&["i1"]), &db, TS + 30_000, 30)
            .unwrap();
        let session = &db.playtime_sessions(None).unwrap()[0];
        assert_eq!(session.duration_sec, 30);
        assert_eq!(session.checkpoint_at, TS + 30_000);
        assert!(session.ended_at.is_none(), "仍在进行中的会话不应有结束时间");
    }

    #[test]
    fn orphaned_sessions_are_closed_at_their_last_checkpoint() {
        // 崩溃恢复：Orbis 被杀 → 会话停在最后一个检查点，而不是「至今仍在进行」
        let db = Db::open_in_memory().unwrap();
        db.insert_installation(&record("i1")).unwrap();
        let records = db.installations().unwrap();
        let mut tracker = PlaytimeTracker::new();
        tracker
            .observe(&records, running_only(&["i1"]), &db, TS, 30)
            .unwrap();
        tracker
            .observe(&records, running_only(&["i1"]), &db, TS + 30_000, 30)
            .unwrap();
        // 模拟进程被杀：tracker 内存态丢掉，会话行留在库里
        tracker.forget_open_sessions();

        assert_eq!(close_orphaned_sessions(&db).unwrap(), 1);
        let session = &db.playtime_sessions(None).unwrap()[0];
        assert_eq!(session.ended_at, Some(TS + 30_000));
        assert_eq!(session.duration_sec, 30, "只能保到最后一个检查点");

        // 幂等：没有孤儿时是空操作
        assert_eq!(close_orphaned_sessions(&db).unwrap(), 0);

        // 聚合不再把崩溃后的时间算进来
        let rows = playtime_rows(&db, None, TS + 100 * 60_000, None).unwrap();
        assert_eq!(rows[0].seconds, 30);
    }

    #[test]
    fn a_session_spanning_midnight_only_counts_the_part_inside_the_window() {
        // 自然日界聚合的准确含义：跨日会话只贡献落在窗口内的部分。
        // 用重叠函数直接验证 —— 它不依赖真实时区，因此可在任何机器上跑。
        let start = TS;
        let end = TS + 4 * 3_600_000; // 4 小时
        let window_start = TS + 3_600_000; // 窗口从 1 小时后开始
        assert_eq!(overlap_sec(start, end, window_start, end + 1), 3 * 3_600);
        // 窗口完全在会话之外
        assert_eq!(overlap_sec(start, end, end, end + 1_000), 0);
        assert_eq!(overlap_sec(start, end, start - 10_000, start), 0);
        // 窗口完全包含会话
        assert_eq!(
            overlap_sec(start, end, start - 10_000, end + 10_000),
            4 * 3_600
        );
    }

    #[test]
    fn today_start_is_local_midnight_and_week_starts_on_monday() {
        let now = TS;
        let day_start = local_day_start_ms(now);
        assert!(day_start <= now, "今日起点不能晚于现在");
        assert!(now - day_start < 86_400_000, "今日起点应在 24 小时之内");

        let week_start = local_week_start_ms(now);
        assert!(week_start <= day_start, "本周起点不晚于今日起点");
        assert!(day_start - week_start < 7 * 86_400_000);
        // 周一：本地日期必须是 Monday
        let monday = Local
            .timestamp_millis_opt(week_start)
            .single()
            .expect("应可解析");
        assert_eq!(monday.weekday(), chrono::Weekday::Mon);
        assert_eq!(monday.time().to_string(), "00:00:00");
    }

    #[test]
    fn unknown_installations_are_ignored() {
        // 只有库里的实例才会被跟踪（进程匹配建立在 installation 记录上）
        let db = Db::open_in_memory().unwrap();
        let mut tracker = PlaytimeTracker::new();
        let events = tracker
            .observe(&[], running_only(&["ghost"]), &db, TS, 30)
            .unwrap();
        assert!(events.is_empty());
        assert_eq!(tracker.open_count(), 0);
    }
}
