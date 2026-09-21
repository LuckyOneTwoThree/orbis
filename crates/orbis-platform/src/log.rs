//! D3 统一日志（`pm/00-产品基石.md` §9.1 schema / `pm/04-技术设计.md` §5.11 落地）。
//!
//! # 行结构（逐字对齐 00 §9.1，不得增删字段）
//!
//! ```json
//! {"ts":"2026-09-21T15:45:31+08:00","source":"tool","category":"detect",
//!  "level":"WARN","message":"…","context":{…},"related_game":"wuthering-waves"}
//! ```
//!
//! | 字段 | 含义 | 取值 |
//! |------|------|------|
//! | `ts` | 时间戳 | RFC 3339，**带本地 UTC 偏移**（00 §9.1「ISO 8601 带时区」） |
//! | `source` | 来源模块 | `game` / `tool` / `launcher` / `backup` / `update` |
//! | `category` | 动作类别 | `detect` / `launch` / `backup` / `modify` / `verify` / `rollback` / `version` / `auth` |
//! | `level` | 级别 | `INFO` / `WARN` / `ERROR` |
//! | `message` | 人类可读描述 | 字符串 |
//! | `context` | 结构化上下文 | 任意 JSON；无上下文为 `null` |
//! | `related_game` | 关联游戏 | slug；全局动作为 `null` |
//!
//! # 时区口径 = 本地时区（Q11 已关闭）
//!
//! `ts` 带**本地 UTC 偏移**，日志文件名（`orbis.YYYY-MM-DD.log`）里的日期按
//! **本地日界**切换（04 §5.11）。
//!
//! 此前一律按 UTC，因为 04 §2.1 选型表没有时区依赖、标准库只给 UTC 时钟。
//! 代价是 UTC+8 用户的文件要到本地 08:00 才滚动 —— 名为「今天的日志」的文件里
//! 混着昨天晚上那部分。2026-09-21 裁决改本地时区，§2.1 随之增补 `chrono`
//! （同一依赖也解除了 A6「今日/本周」本地自然日界的阻塞）。
//!
//! 改动落在本模块的 [`local_date`] / [`format_rfc3339_local`] 两处，
//! 其余逻辑（清理、反解文件名）都建立在它们的输出之上，不感知时区。
//!
//! # 不变量
//!
//! - **日志失败绝不 panic、绝不中断业务**：写文件失败降级到 stderr（日志系统自身的
//!   故障不能反过来拖垮被测对象）。
//! - **只记录 `orbis` 命名空间的 tracing 事件**，且必须带合法的 `source` + `category`。
//!   第三方 crate 的日志不会污染 JSONL 的行结构；缺失字段的本项目事件会打到 stderr
//!   提示 —— 让「忘记用 [`LogRecord`]」这件事可见，而不是静默丢日志。
//! - [`cleanup_old_logs`] **只可能删除**形如 `orbis.YYYY-MM-DD.log` 的**普通文件**，
//!   不做行级裁剪（04 §5.11），不递归、不碰目录、不碰其它任何文件。

use chrono::TimeZone;
use std::borrow::Cow;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

/// 日志保留天数默认值（04 §11 Q4 已拍板：14 天）。
///
/// 运行期可由 `app_setting.log.retention_days` 覆盖（04 §5.11）—— 本模块因此把
/// 保留天数做成**参数**而非常量读取，避免与尚不存在的 DB 层耦合。
pub const DEFAULT_RETENTION_DAYS: u32 = 14;

/// 日志文件命名：`orbis.<UTC 日期>.log`（04 §6.3）。
pub const LOG_FILE_PREFIX: &str = "orbis.";
pub const LOG_FILE_SUFFIX: &str = ".log";

/// 只处理本项目命名空间下的事件；第三方 crate 的日志不进 JSONL（行结构纯净性）。
const TARGET_PREFIX: &str = "orbis";

/// 本项目统一日志的 tracing target。
const LOG_TARGET: &str = "orbis";

// ── 枚举（00 §9.1 / 04 §5.11 的封闭取值）────────────────────

/// 来源模块（00 §9.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Game,
    Tool,
    Launcher,
    Backup,
    Update,
}

impl LogSource {
    pub const ALL: [Self; 5] = [
        Self::Game,
        Self::Tool,
        Self::Launcher,
        Self::Backup,
        Self::Update,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Tool => "tool",
            Self::Launcher => "launcher",
            Self::Backup => "backup",
            Self::Update => "update",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "game" => Some(Self::Game),
            "tool" => Some(Self::Tool),
            "launcher" => Some(Self::Launcher),
            "backup" => Some(Self::Backup),
            "update" => Some(Self::Update),
            _ => None,
        }
    }
}

/// 动作类别（04 §5.11）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategory {
    Detect,
    Launch,
    Backup,
    Modify,
    Verify,
    Rollback,
    Version,
    Auth,
}

impl LogCategory {
    pub const ALL: [Self; 8] = [
        Self::Detect,
        Self::Launch,
        Self::Backup,
        Self::Modify,
        Self::Verify,
        Self::Rollback,
        Self::Version,
        Self::Auth,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Launch => "launch",
            Self::Backup => "backup",
            Self::Modify => "modify",
            Self::Verify => "verify",
            Self::Rollback => "rollback",
            Self::Version => "version",
            Self::Auth => "auth",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "detect" => Some(Self::Detect),
            "launch" => Some(Self::Launch),
            "backup" => Some(Self::Backup),
            "modify" => Some(Self::Modify),
            "verify" => Some(Self::Verify),
            "rollback" => Some(Self::Rollback),
            "version" => Some(Self::Version),
            "auth" => Some(Self::Auth),
            _ => None,
        }
    }
}

/// 级别（00 §9.1 只有三档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// 大写字面量（与 00 §9.1 的 `INFO / WARN / ERROR` 一致）。
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    /// 把 tracing 的 5 档收敛到 00 §9.1 的 3 档（TRACE / DEBUG → INFO）。
    pub const fn from_tracing(level: Level) -> Self {
        match level {
            Level::ERROR => Self::Error,
            Level::WARN => Self::Warn,
            _ => Self::Info,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

// ── 日期换算（仅 UTC，无外部依赖）──────────────────────────

/// 公历日期（UTC）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CivilDate {
    pub year: i64,
    pub month: u32,
    pub day: u32,
}

impl CivilDate {
    /// 由「Unix 纪元起的天数」还原公历日期（Howard Hinnant 的 `civil_from_days`）。
    pub fn from_unix_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097; // [0, 146096]
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
        let mp = (5 * doy + 2) / 153; // [0, 11]
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = if month <= 2 { y + 1 } else { y };
        Self { year, month, day }
    }

    /// 反向：公历日期 → Unix 纪元起的天数（`days_from_civil`）。
    pub fn to_unix_days(self) -> i64 {
        let y = if self.month <= 2 {
            self.year - 1
        } else {
            self.year
        };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.month as i64;
        let d = self.day as i64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// 前后平移若干天。
    pub fn add_days(self, delta: i64) -> Self {
        Self::from_unix_days(self.to_unix_days() + delta)
    }

    /// `YYYY-MM-DD`。
    pub fn to_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// 严格解析 `YYYY-MM-DD`；非法日期（如 `2026-02-30`）返回 `None`。
    pub fn parse_iso(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let digits = |s: &str| -> Option<i64> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            s.parse::<i64>().ok()
        };
        let year = digits(&text[0..4])?;
        let month = digits(&text[5..7])? as u32;
        let day = digits(&text[8..10])? as u32;
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Self { year, month, day })
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iso())
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

pub fn now_unix_seconds() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        // 系统时钟早于 1970 属病态环境，按 0 处理而不是 panic
        Err(_) => 0,
    }
}

/// Unix 秒 → **本地时区**的日历日（04 §5.11，Q11 关闭后的口径）。
///
/// 此前按 UTC 计算，代价是 UTC+8 用户的日志文件要到**本地 08:00** 才滚动 ——
/// 名为「今天的日志」的文件里混着昨天晚上那部分，与直觉不符。
pub fn local_date(unix_seconds: i64) -> CivilDate {
    let Some(moment) = chrono::Local.timestamp_opt(unix_seconds, 0).single() else {
        // 越界时间戳（超出 chrono 可表示范围）退回 epoch，而不是 panic
        return CivilDate::from_unix_days(0);
    };
    let days = moment
        .date_naive()
        .signed_duration_since(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default())
        .num_days();
    CivilDate::from_unix_days(days)
}

/// Unix 秒 → RFC 3339，**带本地 UTC 偏移**（秒精度，如 `2026-09-21T15:45:31+08:00`）。
///
/// 00 §9.1 要求日志 `ts` 是「ISO 8601 带时区」—— 本地偏移同样满足该要求，
/// 而且人直接读文件时不必再做一次心算。
pub fn format_rfc3339_local(unix_seconds: i64) -> String {
    match chrono::Local.timestamp_opt(unix_seconds, 0).single() {
        Some(moment) => moment.to_rfc3339(),
        None => "1970-01-01T00:00:00Z".to_owned(),
    }
}

/// 日志文件名（04 §6.3 目录布局）。
pub fn log_file_name(date: CivilDate) -> String {
    format!("{LOG_FILE_PREFIX}{}{LOG_FILE_SUFFIX}", date.to_iso())
}

/// 从文件名反解日期。**严格**：任何不符合 `orbis.YYYY-MM-DD.log` 的名字都返回 `None`
/// —— 这是清理逻辑「绝不误删」的基础。
pub fn parse_log_file_date(file_name: &str) -> Option<CivilDate> {
    let inner = file_name
        .strip_prefix(LOG_FILE_PREFIX)?
        .strip_suffix(LOG_FILE_SUFFIX)?;
    CivilDate::parse_iso(inner)
}

// ── 日志记录（调用方入口）──────────────────────────────────

/// 一条结构化日志。用 [`LogRecord::emit`] 落盘。
///
/// 用类型而不是裸 `tracing` 宏，是为了让「字段必须齐全」由编译器保证 ——
/// 缺 `source` / `category` 的事件会被 [`JsonlLayer`] 拒绝（并打到 stderr），
/// 那样等于日志静默丢失。
#[derive(Debug, Clone)]
pub struct LogRecord<'a> {
    source: LogSource,
    category: LogCategory,
    level: LogLevel,
    message: Cow<'a, str>,
    related_game: Option<Cow<'a, str>>,
    context: Option<Value>,
}

impl<'a> LogRecord<'a> {
    pub fn new(
        source: LogSource,
        category: LogCategory,
        level: LogLevel,
        message: impl Into<Cow<'a, str>>,
    ) -> Self {
        Self {
            source,
            category,
            level,
            message: message.into(),
            related_game: None,
            context: None,
        }
    }

    /// 关联游戏（00 §9.1：全局动作为空 → JSON 里写 `null`）。
    pub fn with_game(mut self, game_id: impl Into<Cow<'a, str>>) -> Self {
        self.related_game = Some(game_id.into());
        self
    }

    /// 结构化上下文。
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }

    pub fn source(&self) -> LogSource {
        self.source
    }

    pub fn category(&self) -> LogCategory {
        self.category
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// 发出事件。未安装 subscriber 时是廉价空操作（tracing 的既定语义）。
    pub fn emit(self) {
        let source = self.source.slug();
        let category = self.category.slug();
        let message = self.message.as_ref();
        let game = self.related_game.as_deref().unwrap_or("");
        let context = self
            .context
            .map(|c| c.to_string())
            .unwrap_or_else(|| "null".to_owned());

        match self.level {
            LogLevel::Info => tracing::info!(
                target: LOG_TARGET,
                source = %source, category = %category,
                message = %message, related_game = %game, context = %context
            ),
            LogLevel::Warn => tracing::warn!(
                target: LOG_TARGET,
                source = %source, category = %category,
                message = %message, related_game = %game, context = %context
            ),
            LogLevel::Error => tracing::error!(
                target: LOG_TARGET,
                source = %source, category = %category,
                message = %message, related_game = %game, context = %context
            ),
        }
    }
}

// ── JSONL Layer ────────────────────────────────────────────

/// 按 UTC 日滚动的 JSONL 写入层。
pub struct JsonlLayer {
    dir: PathBuf,
    state: Mutex<WriterState>,
}

struct WriterState {
    date: CivilDate,
    file: File,
}

impl JsonlLayer {
    /// 打开（必要时创建）`dir`，并指向今天的日志文件。
    pub fn new(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        let today = local_date(now_unix_seconds());
        let file = open_append(&dir, today)?;
        Ok(Self {
            dir,
            state: Mutex::new(WriterState { date: today, file }),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 追加一行。跨日自动切换文件；任何 I/O 失败降级到 stderr，**不 panic、不上抛**。
    ///
    /// `today` 由调用方给出，因此「跨日切换」这个分支可以用注入的日期直接测到。
    fn append(&self, line: &str, today: CivilDate) {
        // 锁中毒不能让日志系统瘫痪 —— 取回内部数据继续用
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if state.date != today {
            match open_append(&self.dir, today) {
                Ok(file) => {
                    state.date = today;
                    state.file = file;
                }
                Err(err) => {
                    eprintln!(
                        "{LOG_TARGET}: 日志跨日切换失败（沿用 {}）：{err}",
                        state.date
                    );
                    return;
                }
            }
        }

        if let Err(err) = writeln!(state.file, "{line}") {
            eprintln!("{LOG_TARGET}: 日志写入失败：{err}");
        }
    }
}

fn open_append(dir: &Path, date: CivilDate) -> io::Result<File> {
    fs::create_dir_all(dir)?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(log_file_name(date)))
}

impl<S> Layer<S> for JsonlLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // 只收本项目命名空间的事件：第三方 crate 的日志不污染 JSONL 行结构
        if !metadata.target().starts_with(TARGET_PREFIX) {
            return;
        }

        let level = LogLevel::from_tracing(*metadata.level());
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let now = now_unix_seconds();
        match build_line(level, now, &visitor.fields) {
            Some(line) => self.append(&line, local_date(now)),
            None => eprintln!(
                "{LOG_TARGET}: 丢弃一条缺少合法 source/category 的事件（target={}）——\
                 本项目的日志请一律经 LogRecord 发出",
                metadata.target()
            ),
        }
    }
}

/// 把 tracing 事件字段收集成 JSON 对象。
#[derive(Default)]
struct FieldVisitor {
    fields: Map<String, Value>,
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), Value::String(value.to_owned()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_owned(), Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_owned(), Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_owned(), Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), Value::Bool(value));
    }

    /// `%value`（Display）走这条路径：tracing 的 `DisplayValue` 其 Debug 即委托 Display，
    /// 因此 `{value:?}` 拿到的正是展示文本。
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), Value::String(format!("{value:?}")));
    }
}

/// 组装一行 JSON。字段顺序固定为 00 §9.1 的表序，便于人读与 diff；
/// 因此不用 `serde_json::json!`（其 Map 默认按字典序重排键）。
fn build_line(level: LogLevel, unix_seconds: i64, fields: &Map<String, Value>) -> Option<String> {
    let source = fields.get("source").and_then(Value::as_str)?;
    let category = fields.get("category").and_then(Value::as_str)?;
    LogSource::from_slug(source)?;
    LogCategory::from_slug(category)?;

    let message = fields
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();

    // 空串 = 全局动作 → null（00 §9.1「关联游戏（全局动作为空）」）
    let related_game = fields
        .get("related_game")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| Value::String(s.to_owned()))
        .unwrap_or(Value::Null);

    // context 约定为「已序列化的 JSON 文本」，解析后原样嵌入以保持结构化
    let context = match fields.get("context").and_then(Value::as_str) {
        Some(raw) => {
            serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
        }
        None => Value::Null,
    };

    Some(json_line(&[
        ("ts", Value::String(format_rfc3339_local(unix_seconds))),
        ("source", Value::String(source.to_owned())),
        ("category", Value::String(category.to_owned())),
        ("level", Value::String(level.slug().to_owned())),
        ("message", Value::String(message.to_owned())),
        ("context", context),
        ("related_game", related_game),
    ]))
}

/// 按给定顺序拼一行 JSON（键值都用 serde_json 转义，不会产出非法 JSON）。
fn json_line(fields: &[(&str, Value)]) -> String {
    let mut out = String::from("{");
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&Value::String((*key).to_owned()).to_string());
        out.push(':');
        out.push_str(&value.to_string());
    }
    out.push('}');
    out
}

// ── 初始化与保留期清理 ─────────────────────────────────────

/// 安装全局 subscriber（JSONL 落盘）。重复调用返回 `Err` 而非 panic
/// —— 启动路径不应因为日志重复初始化而崩掉。
pub fn init(dir: &Path) -> io::Result<()> {
    let layer = JsonlLayer::new(dir)?;
    registry()
        .with(layer)
        .try_init()
        .map_err(|err| io::Error::other(err.to_string()))
}

/// 构造一个只写文件的 subscriber（测试用；不触碰全局状态）。
pub fn file_only_subscriber(
    dir: impl Into<PathBuf>,
) -> io::Result<impl Subscriber + Send + Sync + 'static> {
    Ok(registry().with(JsonlLayer::new(dir)?))
}

/// 清理结果（04 §5.11：整文件删除，不做行级裁剪）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
    /// 仍在保留期内、未删除的文件数
    pub retained: usize,
}

impl CleanupReport {
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty()
    }
}

/// 删除超出保留期的日志文件。
///
/// **安全边界**：只处理文件名严格匹配 `orbis.YYYY-MM-DD.log` 的**普通文件**，
/// 且在给定目录下**不递归**。目录不存在（首次启动）返回空报告，不是错误。
pub fn cleanup_old_logs(dir: &Path, retention_days: u32) -> CleanupReport {
    let mut report = CleanupReport::default();

    let Ok(entries) = fs::read_dir(dir) else {
        return report;
    };

    let cutoff = local_date(now_unix_seconds()).add_days(-(retention_days as i64));

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(date) = parse_log_file_date(name) else {
            continue;
        };
        if date >= cutoff {
            report.retained += 1;
            continue;
        }
        // 命名匹配也可能是目录（如 `orbis.2020-01-01.log/`）—— 绝不递归删目录
        match entry.file_type() {
            Ok(file_type) if file_type.is_file() => {}
            _ => continue,
        }
        let path = entry.path();
        match fs::remove_file(&path) {
            Ok(()) => report.removed.push(path),
            Err(err) => report.failed.push((path, err.to_string())),
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use serde_json::json;

    fn read_lines(dir: &Path, date: CivilDate) -> Vec<String> {
        match fs::read_to_string(dir.join(log_file_name(date))) {
            Ok(text) => text.lines().map(str::to_owned).collect(),
            Err(_) => Vec::new(),
        }
    }

    // ── 枚举 ────────────────────────────────────────────

    #[test]
    fn source_and_category_slugs_roundtrip_and_reject_unknown() {
        for s in LogSource::ALL {
            assert_eq!(LogSource::from_slug(s.slug()), Some(s));
        }
        for c in LogCategory::ALL {
            assert_eq!(LogCategory::from_slug(c.slug()), Some(c));
        }
        // 00 §9.1 只定义这 5 + 8 个取值；自造值必须被拒绝
        assert_eq!(LogSource::from_slug("app"), None);
        assert_eq!(LogSource::from_slug("Game"), None);
        assert_eq!(LogCategory::from_slug("Diagnose"), None);
        assert_eq!(LogCategory::from_slug(""), None);
    }

    #[test]
    fn log_level_collapses_tracing_levels_to_three() {
        assert_eq!(LogLevel::from_tracing(Level::ERROR), LogLevel::Error);
        assert_eq!(LogLevel::from_tracing(Level::WARN), LogLevel::Warn);
        assert_eq!(LogLevel::from_tracing(Level::INFO), LogLevel::Info);
        assert_eq!(LogLevel::from_tracing(Level::DEBUG), LogLevel::Info);
        assert_eq!(LogLevel::from_tracing(Level::TRACE), LogLevel::Info);
        // 00 §9.1 的档位是大写的
        assert_eq!(LogLevel::Error.slug(), "ERROR");
        assert_eq!(LogLevel::Warn.slug(), "WARN");
        assert_eq!(LogLevel::Info.slug(), "INFO");
    }

    // ── 日期换算 ────────────────────────────────────────

    #[test]
    fn civil_date_conversion_matches_known_dates() {
        // 锚点取自 Python 的 datetime.date（独立来源，避免用同一套算错两次）
        for (days, iso) in [
            (0i64, "1970-01-01"),
            (19_723, "2024-01-01"),
            (20_000, "2024-10-04"),
            (20_454, "2026-01-01"),
            (20_716, "2026-09-20"),
        ] {
            assert_eq!(CivilDate::from_unix_days(days).to_iso(), iso, "days={days}");
        }
        // 双向一致
        for days in [-1i64, 0, 1, 11_017, 20_716, 29_000] {
            assert_eq!(
                CivilDate::from_unix_days(days).to_unix_days(),
                days,
                "days={days}"
            );
        }
    }

    #[test]
    fn civil_date_handles_leap_years_and_epoch_boundary() {
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2024));
        // 2024-02-29 存在
        assert_eq!(
            CivilDate::parse_iso("2024-02-29").map(|d| d.to_iso()),
            Some("2024-02-29".into())
        );
        // 2023-02-29 不存在
        assert_eq!(CivilDate::parse_iso("2023-02-29"), None);
        // 纪元前一日落在 1969-12-31（floor 除法，不是截断除法）。
        // 这里刻意测 `from_unix_days` 而不是 `local_date` —— 后者受本地时区影响，
        // 断言固定日期会在不同时区的机器上随机失败。
        assert_eq!(CivilDate::from_unix_days(-1).to_iso(), "1969-12-31");
        assert_eq!(CivilDate::from_unix_days(0).to_iso(), "1970-01-01");
        // add_days 跨月跨年
        let d = CivilDate::parse_iso("2026-01-01").unwrap();
        assert_eq!(d.add_days(-1).to_iso(), "2025-12-31");
        // 保留期 14 天的边界
        let d = CivilDate::parse_iso("2026-03-01").unwrap();
        assert_eq!(d.add_days(-14).to_iso(), "2026-02-15");
    }

    #[test]
    fn parse_iso_is_strict() {
        for bad in [
            "2026-9-20",
            "2026/09/20",
            "26-09-20",
            "2026-13-01",
            "2026-00-10",
            "2026-09-00",
            "2026-09-31",
            "",
            "abcd-ef-gh",
        ] {
            assert_eq!(CivilDate::parse_iso(bad), None, "应拒绝：{bad:?}");
        }
    }

    #[test]
    fn rfc3339_carries_a_correct_local_offset() {
        // 不断言固定字符串：Q11 关闭后输出带**本地**偏移，固定值会随机器时区变化。
        // 真正的性质是「解析回来必须等于原 Unix 秒」—— 偏移算错会让它差几小时。
        for unix_seconds in [0_i64, 86_399, 86_400, -1, 1_758_000_000] {
            let text = format_rfc3339_local(unix_seconds);
            let parsed = chrono::DateTime::parse_from_rfc3339(&text)
                .unwrap_or_else(|e| panic!("{text:?} 必须是合法 RFC 3339：{e}"));
            assert_eq!(
                parsed.timestamp(),
                unix_seconds,
                "{text:?} 解析回 Unix 秒应一致（不一致说明偏移算错）"
            );
        }
    }

    #[test]
    fn local_date_follows_the_local_calendar_day() {
        // 与 chrono 的本地日期逐点对照 —— 这是「本地日界」这个口径的直接断言
        for unix_seconds in [0_i64, 1_758_000_000, 1_758_000_000 + 86_400] {
            let expected = chrono::Local
                .timestamp_opt(unix_seconds, 0)
                .single()
                .expect("时间戳应可解析")
                .format("%Y-%m-%d")
                .to_string();
            assert_eq!(local_date(unix_seconds).to_iso(), expected);
        }
    }

    // ── 文件名 ──────────────────────────────────────────

    #[test]
    fn log_file_name_roundtrips_and_rejects_decoys() {
        // 04 §5.11 / §6.3 的命名。断言「与 CivilDate 一致」而不是固定日期：
        // 文件名里的日期现在按本地日界取，固定值会随机器时区变化。
        let today = CivilDate::from_unix_days(0);
        assert_eq!(
            log_file_name(today),
            format!("orbis.{}.log", today.to_iso())
        );

        let date = CivilDate::parse_iso("2026-09-20").unwrap();
        assert_eq!(
            parse_log_file_date(&log_file_name(date)),
            Some(date),
            "命名必须可反解"
        );

        // 清理逻辑「绝不误删」的基础：非本命名一律不认
        for decoy in [
            "orbis.log",
            "orbis.2026-9-20.log",
            "orbis.2026-09-20.txt",
            "other.2026-09-20.log",
            "orbis.2026-09-20.log.old",
            "orbis.2026-13-01.log",
            "orbis.db",
            "",
        ] {
            assert_eq!(parse_log_file_date(decoy), None, "不应认作日志：{decoy:?}");
        }
    }

    // ── 写入 ────────────────────────────────────────────

    #[test]
    fn renders_the_documented_schema_with_fixed_key_order() {
        let tmp = TempDir::new("schema");
        let subscriber = file_only_subscriber(tmp.path()).expect("应能建立 subscriber");

        tracing::subscriber::with_default(subscriber, || {
            LogRecord::new(
                LogSource::Tool,
                LogCategory::Detect,
                LogLevel::Warn,
                "工具被拒，该工具不可见",
            )
            .with_game("sample-game")
            .with_context(json!({"source_file": "broken.json", "reason": "malformed"}))
            .emit();
        });

        let lines = read_lines(tmp.path(), local_date(now_unix_seconds()));
        assert_eq!(lines.len(), 1, "应恰好写入一行：{lines:?}");

        let value: Value = serde_json::from_str(&lines[0]).expect("行必须是合法 JSON");
        let object = value.as_object().expect("行必须是 JSON 对象");

        // 00 §9.1 的 7 个字段必须全部出现，且**在原始文本里按表序排列**。
        // 注意不能用解析后的对象断言顺序：serde_json 默认的 Map 会按字典序重排键，
        // 那样断言的是「恰好等于字典序」，与实现意图无关。
        let expected_keys = [
            "\"ts\"",
            "\"source\"",
            "\"category\"",
            "\"level\"",
            "\"message\"",
            "\"context\"",
            "\"related_game\"",
        ];
        let positions: Vec<usize> = expected_keys
            .iter()
            .map(|key| {
                lines[0]
                    .find(key)
                    .unwrap_or_else(|| panic!("缺少字段 {key}：{}", lines[0]))
            })
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "字段顺序应与 00 §9.1 表序一致：{}",
            lines[0]
        );
        assert_eq!(object.len(), expected_keys.len(), "不得有多余字段");

        assert_eq!(object["source"], json!("tool"));
        assert_eq!(object["category"], json!("detect"));
        assert_eq!(object["level"], json!("WARN"));
        assert_eq!(object["message"], json!("工具被拒，该工具不可见"));
        assert_eq!(object["related_game"], json!("sample-game"));
        // context 必须是结构化对象，而不是被转义成一坨字符串
        assert_eq!(object["context"]["source_file"], json!("broken.json"));
        assert_eq!(object["context"]["reason"], json!("malformed"));
        // ts 必须是 RFC 3339 **带时区**（00 §9.1）。Q11 关闭后带的是本地偏移，
        // 因此断言形态与可解析性，而不是固定的 `Z`。
        let ts = object["ts"].as_str().unwrap();
        assert!(
            chrono::DateTime::parse_from_rfc3339(ts).is_ok(),
            "ts 必须是合法 RFC 3339：{ts}"
        );
        assert!(
            ts.contains('+') || ts.ends_with('Z'),
            "ts 必须带时区（本地偏移或 Z）：{ts}"
        );
    }

    #[test]
    fn global_actions_and_missing_context_render_as_null() {
        let tmp = TempDir::new("nulls");
        let subscriber = file_only_subscriber(tmp.path()).unwrap();

        tracing::subscriber::with_default(subscriber, || {
            LogRecord::new(
                LogSource::Update,
                LogCategory::Version,
                LogLevel::Info,
                "版本探测批次完成",
            )
            .emit();
        });

        let lines = read_lines(tmp.path(), local_date(now_unix_seconds()));
        let value: Value = serde_json::from_str(&lines[0]).unwrap();
        // 00 §9.1：全局动作为空 → null（不是缺字段，也不是空串）
        assert_eq!(value["related_game"], Value::Null);
        assert_eq!(value["context"], Value::Null);
    }

    #[test]
    fn levels_map_to_the_three_documented_buckets() {
        let tmp = TempDir::new("levels");
        let subscriber = file_only_subscriber(tmp.path()).unwrap();

        tracing::subscriber::with_default(subscriber, || {
            for level in [LogLevel::Info, LogLevel::Warn, LogLevel::Error] {
                LogRecord::new(LogSource::Game, LogCategory::Launch, level, "x").emit();
            }
        });

        let lines = read_lines(tmp.path(), local_date(now_unix_seconds()));
        let levels: Vec<String> = lines
            .iter()
            .map(|line| {
                let value: Value = serde_json::from_str(line).unwrap();
                value["level"].as_str().unwrap().to_owned()
            })
            .collect();
        assert_eq!(levels, vec!["INFO", "WARN", "ERROR"]);
    }

    #[test]
    fn events_missing_source_or_category_are_rejected_visibly() {
        let tmp = TempDir::new("incomplete");
        let subscriber = file_only_subscriber(tmp.path()).unwrap();

        tracing::subscriber::with_default(subscriber, || {
            // 裸 tracing 宏（缺少 schema 字段）不得进 JSONL —— 否则行结构被破坏
            tracing::warn!(target: "orbis", message = "没有 source/category");
            // 非法枚举值同样拒绝
            tracing::warn!(target: "orbis", source = "app", category = "detect", message = "非法 source");
        });

        assert!(
            read_lines(tmp.path(), local_date(now_unix_seconds())).is_empty(),
            "不合法的事件不应写进日志文件"
        );
    }

    #[test]
    fn third_party_targets_are_ignored() {
        let tmp = TempDir::new("thirdparty");
        let subscriber = file_only_subscriber(tmp.path()).unwrap();

        tracing::subscriber::with_default(subscriber, || {
            // 第三方 crate 即便恰好带了同名字段，也不得污染行结构
            tracing::warn!(
                target: "hyper::proto",
                source = "tool", category = "detect", message = "来自第三方"
            );
        });

        assert!(read_lines(tmp.path(), local_date(now_unix_seconds())).is_empty());
    }

    #[test]
    fn appends_rather_than_truncating() {
        let tmp = TempDir::new("append");
        let subscriber = file_only_subscriber(tmp.path()).unwrap();

        tracing::subscriber::with_default(subscriber, || {
            LogRecord::new(
                LogSource::Game,
                LogCategory::Launch,
                LogLevel::Info,
                "第一次",
            )
            .emit();
            LogRecord::new(
                LogSource::Game,
                LogCategory::Launch,
                LogLevel::Info,
                "第二次",
            )
            .emit();
        });

        assert_eq!(
            read_lines(tmp.path(), local_date(now_unix_seconds())).len(),
            2
        );
    }

    #[test]
    fn rolls_over_to_a_new_file_when_the_local_date_changes() {
        let tmp = TempDir::new("rollover");
        let layer = JsonlLayer::new(tmp.path()).unwrap();

        let day_a = CivilDate::parse_iso("2026-01-01").unwrap();
        let day_b = CivilDate::parse_iso("2026-01-02").unwrap();

        layer.append(r#"{"ts":"a"}"#, day_a);
        layer.append(r#"{"ts":"b"}"#, day_a);
        // 跨日：必须换文件，不能续写在旧文件里
        layer.append(r#"{"ts":"c"}"#, day_b);

        assert_eq!(
            read_lines(tmp.path(), day_a),
            vec![r#"{"ts":"a"}"#, r#"{"ts":"b"}"#]
        );
        assert_eq!(read_lines(tmp.path(), day_b), vec![r#"{"ts":"c"}"#]);
    }

    // ── 清理 ────────────────────────────────────────────

    #[test]
    fn cleanup_removes_only_expired_own_logs() {
        let tmp = TempDir::new("cleanup");
        let today = local_date(now_unix_seconds());

        let fresh = log_file_name(today);
        let expired = log_file_name(today.add_days(-30));
        let boundary = log_file_name(today.add_days(-14)); // 恰好等于保留期 → 保留
        let slightly_old = log_file_name(today.add_days(-15));

        for name in [&fresh, &expired, &boundary, &slightly_old] {
            fs::write(tmp.path().join(name), "x\n").unwrap();
        }
        // 诱饵：命名不符的文件绝不能被动
        let decoys = [
            "other.2020-01-01.log",
            "orbis.not-a-date.log",
            "orbis.2020-01-01.txt",
            "notes.md",
        ];
        for name in decoys {
            fs::write(tmp.path().join(name), "keep me\n").unwrap();
        }

        let report = cleanup_old_logs(tmp.path(), DEFAULT_RETENTION_DAYS);

        let mut removed: Vec<String> = report
            .removed
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        removed.sort();
        assert_eq!(removed, vec![expired.clone(), slightly_old.clone()]);
        assert!(report.is_clean(), "不应有删除失败：{:?}", report.failed);
        // 保留期内 2 个（今天 + 第 14 天）
        assert_eq!(report.retained, 2);

        // 诱饵必须原封不动
        for name in decoys {
            assert!(
                tmp.path().join(name).exists(),
                "命名不符的文件不得被删：{name}"
            );
        }
        assert!(tmp.path().join(&boundary).exists(), "保留期边界应保留");
    }

    #[test]
    fn cleanup_never_recurses_into_directories() {
        let tmp = TempDir::new("cleanup-dir");
        let today = local_date(now_unix_seconds());
        // 同名但其实是目录 → 绝不递归删
        let dir_like_file = tmp.path().join(log_file_name(today.add_days(-99)));
        fs::create_dir_all(&dir_like_file).unwrap();
        fs::write(dir_like_file.join("inner.txt"), "inner").unwrap();

        let report = cleanup_old_logs(tmp.path(), DEFAULT_RETENTION_DAYS);

        assert!(
            report.removed.is_empty(),
            "不得删除目录：{:?}",
            report.removed
        );
        assert!(dir_like_file.join("inner.txt").exists());
    }

    #[test]
    fn cleanup_on_a_missing_directory_is_not_an_error() {
        let missing = std::env::temp_dir().join("orbis-log-does-not-exist-xyz");
        let report = cleanup_old_logs(&missing, DEFAULT_RETENTION_DAYS);
        assert_eq!(report, CleanupReport::default());
        assert!(report.is_clean());
    }

    #[test]
    fn zero_retention_keeps_only_today() {
        let tmp = TempDir::new("retention-zero");
        let today = local_date(now_unix_seconds());
        fs::write(tmp.path().join(log_file_name(today)), "x").unwrap();
        fs::write(tmp.path().join(log_file_name(today.add_days(-1))), "x").unwrap();

        let report = cleanup_old_logs(tmp.path(), 0);

        assert_eq!(report.removed.len(), 1);
        assert!(tmp.path().join(log_file_name(today)).exists());
        assert!(!tmp.path().join(log_file_name(today.add_days(-1))).exists());
    }
}
