//! 契约错误模型（`docs/ipc-contract.md` §5）—— 壳层对前端唯一的错误形状。
//!
//! # 为什么错误模型在壳层
//!
//! 这是**IPC 序列化形状**，不是业务概念：领域 crate 有自己的错误类型
//! （`DbError` / `SeedError` / `ManifestError` …），它们表达「出了什么事」；
//! 本模块表达「这件事对前端叫什么名字、能不能重试」。把领域错误翻译成契约错误码
//! 是壳层（唯一组装根）的职责，领域 crate 不该为了 IPC 而长出自己的错误码表。
//!
//! # 两条硬约束
//!
//! 1. **`retryable` 由错误码决定**，不由调用点决定。契约 §5 的表里每一行都写明了
//!    retryable 与对应的 UI 行为；若允许在调用点随手传，同一个码在不同命令里就会
//!    出现「有时能重试有时不能」，UI 的「重试」按钮随之失真。因此
//!    [`OrbisError::new`] 从 [`ErrorCode::retryable`] 取值，字段对外只读。
//! 2. **拒绝时必须是对象，不能是字符串**。前端 `src/api/tauri.ts` 的
//!    `normalizeError` 靠 `'code' in raw` 判定契约错误；返回字符串会被归成
//!    `INTERNAL`，丢掉全部行动建议（契约 §5「Tauri 侧约定」）。

use std::collections::BTreeMap;

use orbis_platform::db::DbError;
use serde::Serialize;
use serde_json::Value;

/// 契约 §5 的全部 30 个错误码。
///
/// 一次性全部列出（而不是只写当前用到的几个）是有意的：这张表是 UI 文案映射的
/// 唯一依据，提前收敛能让「新命令引入了什么新错误」变成一次 diff 而不是一次考古。
///
/// 因此未构造的变体**刻意保留**（它们对应的命令还没落地，如 `COMPAT_*`）：
/// 删掉它们，下一个实现者就得回契约 §5 把表重新抄一遍。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    GameNotFound,
    GameAlreadyRunning,
    GameNotRunning,
    ExecutableMissing,
    ExecutableInvalid,
    LooksLikeLauncher,
    GameMismatch,
    InstallationDuplicate,
    PathNotFound,
    PathPermissionDenied,
    GameProcessActive,
    ConfigSourceUnsupported,
    CompatBlocked,
    CompatUnknownL3,
    CompatOverrideRequired,
    ConsentRequired,
    BackupNotFound,
    BackupFailed,
    BackupSpaceInsufficient,
    RestoreHashMismatch,
    ToolNotFound,
    ToolAssetNotConfigured,
    ToolAssetDownloadFailed,
    ToolAssetHashMismatch,
    NetworkUnreachable,
    VersionSourceUnsupported,
    SettingUnknownKey,
    SettingInvalidValue,
    NotImplemented,
    Internal,
}

impl ErrorCode {
    /// 契约 §5 表格的 `retryable` 列，逐行对照。
    ///
    /// 语义是「**重试一次是否有意义**」：路径不存在、进程仍在运行、空间不足、
    /// 网络抖动属于外部条件，用户处理后重试可能成功；而「工具不存在」「兼容性
    /// 硬阻止」「选错了 exe」重试多少次都是同一个结果，给重试按钮只会误导。
    pub const fn retryable(self) -> bool {
        match self {
            // ✓ 外部条件可变化
            Self::ExecutableMissing
            | Self::PathNotFound
            | Self::GameProcessActive
            | Self::BackupFailed
            | Self::BackupSpaceInsufficient
            | Self::ToolAssetDownloadFailed
            | Self::NetworkUnreachable => true,
            // ✗ 重试不改变结果
            Self::GameNotFound
            | Self::GameAlreadyRunning
            | Self::GameNotRunning
            | Self::ExecutableInvalid
            | Self::LooksLikeLauncher
            | Self::GameMismatch
            | Self::InstallationDuplicate
            | Self::PathPermissionDenied
            | Self::ConfigSourceUnsupported
            | Self::CompatBlocked
            | Self::CompatUnknownL3
            | Self::CompatOverrideRequired
            | Self::ConsentRequired
            | Self::BackupNotFound
            | Self::RestoreHashMismatch
            | Self::ToolNotFound
            | Self::ToolAssetNotConfigured
            | Self::ToolAssetHashMismatch
            | Self::VersionSourceUnsupported
            | Self::SettingUnknownKey
            | Self::SettingInvalidValue
            | Self::NotImplemented
            | Self::Internal => false,
        }
    }
}

/// 契约 `OrbisError`。
///
/// 字段刻意**私有**：`retryable` 若可被外部赋值，就等于允许绕过上表的约束。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrbisError {
    code: ErrorCode,
    /// 开发者可读，**仅日志/调试**，不直接展示（契约 §1）
    message: String,
    /// 结构化上下文；`null` 而不是空对象，UI 用 `detail === null` 判空
    detail: Option<BTreeMap<String, Value>>,
    retryable: bool,
}

impl OrbisError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            retryable: code.retryable(),
        }
    }

    /// 补一项结构化上下文（可链式多次调用）。
    pub fn with_detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.detail
            .get_or_insert_with(BTreeMap::new)
            .insert(key.to_owned(), value.into());
        self
    }

    // 以下三个访问器当前只有单测在读（命令侧只构造、不读）。保留它们不是仪式：
    // 没有它们，测试只能靠匹配 JSON 字符串来断言错误码，序列化实现一改就会
    // 让一批测试以「格式变了」为由集体失败。
    #[allow(dead_code)]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    #[allow(dead_code)]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[allow(dead_code)]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// 命令统一返回类型。
pub type OrbisResult<T> = Result<T, OrbisError>;

/// `INTERNAL` 的简写（未归类异常一律走这里，提醒同时把原因写进日志）。
pub fn internal(message: impl Into<String>) -> OrbisError {
    OrbisError::new(ErrorCode::Internal, message)
}

impl From<DbError> for OrbisError {
    /// 领域错误 → 契约错误码。只有设置项相关的两种情形有专门的码，
    /// 其余（打开失败 / 迁移失败 / SQLite 错误）都是运行环境缺陷，归 `INTERNAL`。
    fn from(err: DbError) -> Self {
        let code = match &err {
            DbError::InvalidSettingValue { .. } => ErrorCode::SettingInvalidValue,
            // 其余（打开 / 迁移 / SQLite 失败、存库值被外部改坏）都是运行环境缺陷：
            // 重试不会变好，也不该给用户一个「再试一次」的按钮
            _ => ErrorCode::Internal,
        };
        OrbisError::new(code, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_flags_match_the_contract_table() {
        // 契约 §5 的 retryable 列：✓ 的 7 个，其余一律 ✗
        let retryable = [
            ErrorCode::ExecutableMissing,
            ErrorCode::PathNotFound,
            ErrorCode::GameProcessActive,
            ErrorCode::BackupFailed,
            ErrorCode::BackupSpaceInsufficient,
            ErrorCode::ToolAssetDownloadFailed,
            ErrorCode::NetworkUnreachable,
        ];
        for code in retryable {
            assert!(code.retryable(), "{code:?} 应可重试");
        }

        let not_retryable = [
            ErrorCode::GameNotFound,
            ErrorCode::GameAlreadyRunning,
            ErrorCode::GameNotRunning,
            ErrorCode::ExecutableInvalid,
            ErrorCode::LooksLikeLauncher,
            ErrorCode::GameMismatch,
            ErrorCode::InstallationDuplicate,
            ErrorCode::PathPermissionDenied,
            ErrorCode::ConfigSourceUnsupported,
            ErrorCode::CompatBlocked,
            ErrorCode::CompatUnknownL3,
            ErrorCode::CompatOverrideRequired,
            ErrorCode::ConsentRequired,
            ErrorCode::BackupNotFound,
            ErrorCode::RestoreHashMismatch,
            ErrorCode::ToolNotFound,
            ErrorCode::ToolAssetNotConfigured,
            ErrorCode::ToolAssetHashMismatch,
            ErrorCode::VersionSourceUnsupported,
            ErrorCode::SettingUnknownKey,
            ErrorCode::SettingInvalidValue,
            ErrorCode::NotImplemented,
            ErrorCode::Internal,
        ];
        for code in not_retryable {
            assert!(!code.retryable(), "{code:?} 不应给出重试按钮");
        }

        // 表必须被完整覆盖（30 个码）：漏掉一行意味着某个码的 retryable 无人断言
        assert_eq!(retryable.len() + not_retryable.len(), 30);
    }

    #[test]
    fn serializes_to_the_shape_the_frontend_expects() {
        // 前端靠 'code' in raw 判定契约错误 → 必须是对象且带 code
        let err = OrbisError::new(ErrorCode::ToolNotFound, "tool x not found")
            .with_detail("toolId", "sample-ns/sample-tool");
        let json = serde_json::to_value(&err).unwrap();

        assert_eq!(json["code"], "TOOL_NOT_FOUND");
        assert_eq!(json["message"], "tool x not found");
        assert_eq!(json["retryable"], false);
        assert_eq!(json["detail"]["toolId"], "sample-ns/sample-tool");
        // 字段名必须是 camelCase，且 detail 存在于键集里（不是被 skip 掉）
        assert!(json.get("detail").is_some());
        assert_eq!(json.as_object().unwrap().len(), 4);
    }

    #[test]
    fn detail_is_null_when_absent_not_an_empty_object() {
        // UI 用 detail === null 判空；空对象会让它走进「有详情」的分支
        let json = serde_json::to_value(OrbisError::new(ErrorCode::Internal, "boom")).unwrap();
        assert!(json["detail"].is_null());
    }

    #[test]
    fn retryable_cannot_be_set_from_outside_the_code() {
        let err = OrbisError::new(ErrorCode::PathNotFound, "gone");
        assert!(err.retryable());
        assert_eq!(err.code(), ErrorCode::PathNotFound);
    }

    #[test]
    fn db_errors_map_to_the_documented_codes() {
        let invalid = OrbisError::from(DbError::InvalidSettingValue {
            key: "log.retention_days",
            detail: "0 超出范围".into(),
        });
        assert_eq!(invalid.code(), ErrorCode::SettingInvalidValue);
        assert!(
            !invalid.retryable(),
            "设置越界是开发期/输入错误，重试无意义"
        );

        let corrupt = OrbisError::from(DbError::CorruptSettingValue {
            key: "log.retention_days".into(),
            detail: "非法 JSON".into(),
        });
        assert_eq!(corrupt.code(), ErrorCode::Internal);

        let unsupported = OrbisError::from(DbError::UnsupportedSchemaVersion {
            found: 99,
            supported: 1,
        });
        assert_eq!(unsupported.code(), ErrorCode::Internal);
        assert!(
            unsupported.message().contains("99"),
            "开发者可读的 message 应带上事实：{}",
            unsupported.message()
        );
    }
}
