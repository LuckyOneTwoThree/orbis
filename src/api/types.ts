/**
 * Orbis 前后端契约 · DTO 定义（过渡期手写单源）
 *
 * 来源：docs/ipc-contract.md v1.0（status: frozen）
 * 引入 tauri-specta 后，本文件由 `bindings.gen.ts` 取代（04 §2.1 / Q3）；
 * `contract.ts` 的方法签名保持不变，因此 UI 代码不受影响。
 *
 * 约定（契约 §1）：
 *  - 字段 camelCase；枚举值 snake_case；时间 = epoch 毫秒；时长 = 秒
 *  - 路径为 OS 原生分隔符，UI 只展示不拼接
 *  - 未知一律 null（不用空串、不用 "Unknown" 字面量）
 *  - 版本比较只用 versionNorm（major.minor）
 */

// ── 枚举（契约 §2）────────────────────────────────────────

export type GameId =
  | 'genshin-impact'
  | 'honkai-star-rail'
  | 'zenless-zone-zero'
  | 'wuthering-waves'
  | 'arknights-endfield';

/** 库内与 IPC 一律小写；「国服」仅为 UI 展示映射，禁止入库 */
export type Region = 'cn' | 'global' | 'bili';

/** 游戏运行态：互斥主状态（04 §6.4.1 ①） */
export type GameRuntimeStatus =
  | 'installed'
  | 'running'
  | 'updating'
  | 'repairing'
  | 'broken';

/** 工具兼容态五态（00 §8.8 / P0-C §2.1，权威 = seed.json） */
export type ToolCompatStatus =
  | 'verified'
  | 'compatible'
  | 'unknown'
  | 'incompatible'
  | 'deprecated';

export type RiskLevel = 'L0' | 'L1' | 'L2' | 'L3';

export type ToolType = 'config_modify' | 'external_process';

export type ToolPermission =
  | 'read_config'
  | 'write_config'
  | 'launch_external'
  | 'process_attach';

export type BackupTrigger = 'manual' | 'pre_modify' | 'pre_restore';

export type VersionSourceId =
  | 'hyp:getGamePackages'
  | 'kuro:index.json'
  | 'degraded';

/** L1 执行链路步骤（04 §5.5 状态机） */
export type ApplyStep =
  | 'gate'
  | 'precheck'
  | 'backup'
  | 'modify'
  | 'verify'
  | 'rollback';

export type ApplyStepState =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'skipped';

export type SettingKey =
  | 'version_check.enabled'
  | 'log.retention_days'
  | 'playtime.checkpoint_sec';

/** 04 §6.4.3 —— 需处理判定，由 Core 计算，UI 只消费 */
export type AttentionReason =
  | 'update_available'
  | 'broken'
  | 'version_unknown'
  | 'tool_unknown'
  | 'tool_incompatible';

// ── 错误模型（契约 §5）──────────────────────────────────────

export type ErrorCode =
  | 'GAME_NOT_FOUND'
  | 'GAME_ALREADY_RUNNING'
  | 'GAME_NOT_RUNNING'
  | 'EXECUTABLE_MISSING'
  | 'EXECUTABLE_INVALID'
  | 'LOOKS_LIKE_LAUNCHER'
  | 'GAME_MISMATCH'
  | 'INSTALLATION_DUPLICATE'
  | 'PATH_NOT_FOUND'
  | 'PATH_PERMISSION_DENIED'
  | 'GAME_PROCESS_ACTIVE'
  | 'CONFIG_SOURCE_UNSUPPORTED'
  | 'COMPAT_BLOCKED'
  | 'COMPAT_UNKNOWN_L3'
  | 'COMPAT_OVERRIDE_REQUIRED'
  | 'CONSENT_REQUIRED'
  | 'BACKUP_NOT_FOUND'
  | 'BACKUP_FAILED'
  | 'BACKUP_SPACE_INSUFFICIENT'
  | 'RESTORE_HASH_MISMATCH'
  | 'TOOL_NOT_FOUND'
  | 'TOOL_ASSET_NOT_CONFIGURED'
  | 'TOOL_ASSET_DOWNLOAD_FAILED'
  | 'TOOL_ASSET_HASH_MISMATCH'
  | 'NETWORK_UNREACHABLE'
  | 'VERSION_SOURCE_UNSUPPORTED'
  | 'SETTING_UNKNOWN_KEY'
  | 'SETTING_INVALID_VALUE'
  | 'NOT_IMPLEMENTED'
  | 'INTERNAL';

export interface OrbisError {
  code: ErrorCode;
  /** 开发者可读，仅日志/调试用，**不直接展示给用户**（契约 §1） */
  message: string;
  detail: Record<string, string | number | boolean> | null;
  retryable: boolean;
}

/** 命令拒绝时抛出；UI 按 code 映射文案（契约 §5 表） */
export class OrbisInvokeError extends Error {
  readonly code: ErrorCode;
  readonly detail: Record<string, string | number | boolean> | null;
  readonly retryable: boolean;

  constructor(err: OrbisError) {
    super(err.message);
    this.name = 'OrbisInvokeError';
    this.code = err.code;
    this.detail = err.detail;
    this.retryable = err.retryable;
  }
}

// ── DTO（契约 §6）──────────────────────────────────────────

/**
 * 游戏目录条目（编译期静态，不入库；00 §7.10 Game 实体）
 * UI 用它渲染「未安装」行、Dashboard 标题与 A1 空状态的官方入口引导。
 * 禁止在 UI 侧另行硬编码游戏目录。
 */
export interface GameCatalogEntry {
  id: GameId;
  name: string;
  enName: string;
  publisher: string;
  /** 该游戏已知区服（A1：同款多区服 = 多个 GameInstallation） */
  regions: Region[];
  /** 空状态引导用的官方入口；URL 正确性由实测项 T8 核实 */
  officialUrl: string | null;
  /** 存在随包分发的组件（B7 发行包分离需对用户可见） */
  hasBundledComponent: boolean;
}

export interface InstallationDto {
  id: string;
  gameId: GameId;
  region: Region;
  installPath: string;
  executablePath: string;
  /** 原始识别值；null = Unknown（02 A3：不猜、不显示过期缓存） */
  localVersion: string | null;
  /** major.minor；查询/比较只用它（04 §5.1） */
  versionNorm: string | null;
  versionSource: 'exe_versioninfo' | 'feature_file' | 'directory' | null;
  status: GameRuntimeStatus;
  /** 会话内派生（远程版本不落库） */
  updateAvailable: boolean;
  /** = versionNorm === null */
  versionUnknown: boolean;
  /** 04 §6.4.3 判定式结果 */
  needsAttention: boolean;
  attentionReasons: AttentionReason[];
  addedVia: 'scan' | 'manual';
  pid: number | null;
  playtime: { todaySec: number; weekSec: number; totalSec: number };
  /** Provider 是否声明配置路径（A8 范围依据） */
  hasConfigSource: boolean;
  /** 驱动 03 §5.4 的禁用态文案：provider_not_declared → 「暂不支持配置备份」；not_applicable → 「不适用」 */
  configUnsupportedReason: 'provider_not_declared' | 'not_applicable' | null;
  createdAt: number;
  updatedAt: number;
}

export interface AttentionSummary {
  total: number;
  needsAttention: number;
  updateAvailable: number;
  broken: number;
  toolAttention: number;
}

export interface InstallationListResult {
  installations: InstallationDto[];
  summary: AttentionSummary;
}

export interface InstallationDetail extends InstallationDto {
  tools: ToolDto[];
  latestBackup: BackupSummary | null;
  launchProfile: LaunchProfileDto;
}

export interface ScanResult {
  scanId: string;
  found: number;
  installations: InstallationDto[];
  /** 超时/权限不足 → true；不抛出（02 A1 边界） */
  partial: boolean;
  warnings: string[];
}

export interface ExecutableValidation {
  ok: boolean;
  detectedGameId: GameId | null;
  reason:
    | 'ok'
    | 'not_an_executable'
    | 'looks_like_launcher'
    | 'game_mismatch'
    | 'unreadable';
  versionRaw: string | null;
}

export interface RuntimeStateDto {
  installationId: string;
  status: GameRuntimeStatus;
  pid: number | null;
  updateAvailable: boolean;
  versionUnknown: boolean;
}

export interface LaunchResult {
  installationId: string;
  pid: number;
  unlock: {
    requested: boolean;
    attached: boolean;
    /** B8「本次不解锁」 */
    skipped: boolean;
    /** 非空 → UI 明示「本次未解锁」，但不阻塞游戏本体（02 B7 边界） */
    degradedReason: string | null;
  };
}

export interface LaunchProfileDto {
  installationId: string;
  args: string;
  updatedAt: number;
}

export interface PlaytimeResult {
  scope: 'today' | 'week' | 'total';
  totalSec: number;
  perInstallation: { installationId: string; gameId: GameId; seconds: number }[];
}

export interface RemoteVersionResult {
  gameId: GameId;
  region: Region;
  /** 归一化后 major.minor；null = 不可达/降级 */
  version: string | null;
  source: VersionSourceId;
  fetchedAt: number;
  /** 降级信息，非异常路径 */
  error: OrbisError | null;
}

export interface UpdateStatusDto {
  installationId: string;
  gameId: GameId;
  localVersionNorm: string | null;
  remoteVersion: string | null;
  status: 'up_to_date' | 'update_available' | 'unknown';
  checkedAt: number | null;
  source: VersionSourceId | null;
}

export interface ToolSource {
  kind: 'builtin' | 'bundled' | 'upstream';
  repo: string | null;
  license: string | null;
  version: string | null;
}

export interface ToolAssetDto {
  toolId: string;
  assetKey: string;
  state: 'not_required' | 'missing' | 'ready' | 'hash_mismatch';
  version: string | null;
  sha256: string | null;
  expectedSha256: string | null;
  installedPath: string | null;
  /** false → UI 显示「组件构建管道待定稿」（04 §11 Q1），不视为错误 */
  downloadUrlConfigured: boolean;
}

export interface CompatibilityDto {
  gameId: GameId;
  toolId: string;
  status: ToolCompatStatus;
  matchKind: 'exact' | 'prefix' | 'none';
  matchedVersionKey: string | null;
  /** YYYY-MM-DD（seed 原文，非 epoch） */
  verifiedAt: string | null;
  verifiedBy: string | null;
  notes: string | null;
  /** null → seed 缺失/损坏 */
  seedSchemaVersion: string | null;
}

export interface ToolDto {
  id: string;
  gameId: GameId;
  name: string;
  description: string;
  version: string;
  type: ToolType;
  riskLevel: RiskLevel;
  permissions: ToolPermission[];
  requiresAdmin: boolean;
  backupRequired: boolean;
  enabled: boolean;
  /** 实时查询 seed，非 Manifest 静态值 */
  compat: CompatibilityDto;
  asset: ToolAssetDto | null;
  source: ToolSource;
  /** 非空 → UI 显示「部分参数待实测」 */
  pendingVerifications: string[];
}

export interface ToolDetail extends ToolDto {
  consentText: string | null;
  consentTextHash: string | null;
  permissionsDetail: {
    permission: ToolPermission;
    label: string;
    detail: string;
  }[];
}

export interface ToolEnableResult {
  toolId: string;
  enabled: boolean;
  outcome: 'applied' | 'rolled_back' | 'blocked' | 'unchanged';
  compat: CompatibilityDto;
  backupId: string | null;
  rollbackBackupId: string | null;
  blockedReason: OrbisError | null;
}

export interface BackupSummary {
  id: string;
  installationId: string;
  gameId: GameId;
  /** null = 手动备份 */
  toolId: string | null;
  trigger: BackupTrigger;
  fileCount: number;
  totalBytes: number;
  /** 主文件相对路径，供「高级详情」单行展示 */
  primaryFile: string | null;
  createdAt: number;
}

export interface RestoreResult {
  backupId: string;
  restoredFileCount: number;
  verified: boolean;
  /** 自动生成的 pre_restore 备份（可撤销恢复本身） */
  preRestoreBackupId: string;
}

export interface BackupStorageInfo {
  installationId: string;
  backupCount: number;
  totalBytes: number;
  freeDiskBytes: number;
  estimatedNextSizeBytes: number | null;
}

export interface AppSettings {
  'version_check.enabled': boolean;
  'log.retention_days': number;
  'playtime.checkpoint_sec': number;
}

export type WindowAction = 'minimize' | 'maximize' | 'close';

// ── 事件（契约 §4）─────────────────────────────────────────

export interface ScanProgressPayload {
  scanId: string;
  phase: 'registry' | 'paths' | 'validate' | 'done';
  scanned: number;
  total: number;
  currentGameId: GameId | null;
}

export interface GameStateChangedPayload {
  installationId: string;
  status: GameRuntimeStatus;
  updateAvailable: boolean;
  versionUnknown: boolean;
  versionNorm: string | null;
  pid: number | null;
}

export interface PlaytimeUpdatedPayload {
  installationId: string;
  /** 当前会话累计 */
  sessionSec: number;
  /** 该实例今日累计（权威值，UI 直接覆盖不做加法） */
  todaySec: number;
}

export interface ApplyStepDetail {
  /** 默认层：语义描述。禁止出现文件名/键名/哈希（03 §5.6 U8） */
  default: string;
  /** 高级层：底层事实，仅在「高级详情」展开时展示（00 §12.2） */
  advanced: string[] | null;
}

export interface ToolApplyStepPayload {
  toolId: string;
  installationId: string;
  step: ApplyStep;
  state: ApplyStepState;
  detail: ApplyStepDetail;
}

export interface ToolAssetProgressPayload {
  toolId: string;
  phase: 'download' | 'verify' | 'extract';
  receivedBytes: number;
  totalBytes: number | null;
}

export interface VersionRefreshedPayload {
  fetchedAt: number;
  results: RemoteVersionResult[];
}

export interface OrbisEventMap {
  'scan:progress': ScanProgressPayload;
  'game:state-changed': GameStateChangedPayload;
  'playtime:updated': PlaytimeUpdatedPayload;
  'tool:apply-step': ToolApplyStepPayload;
  'tool:asset-progress': ToolAssetProgressPayload;
  'version:refreshed': VersionRefreshedPayload;
}

export type OrbisEventName = keyof OrbisEventMap;
