/**
 * Orbis 前后端契约 · 命令面接口（UI 唯一依赖点）
 *
 * 来源：docs/ipc-contract.md v1.0 §3 / §4（status: frozen）
 *
 * 铁律（00 §7.9）：
 *  - `views/` 与 `store/` **只依赖本接口**，禁止出现 `invoke(...)`、路径拼接、SQL、哈希运算
 *  - 命令返回值不含面向用户的成品文案（文案按 code / status 在 UI 映射，契约 §7.3）
 *  - 拒绝时抛 `OrbisInvokeError`（含 code / retryable），UI 据此给行动建议
 */
import type {
  AppSettings,
  BackupStorageInfo,
  BackupSummary,
  BackupTrigger,
  CompatibilityDto,
  ExecutableValidation,
  GameCatalogEntry,
  GameId,
  InstallationDetail,
  InstallationDto,
  InstallationListResult,
  LaunchProfileDto,
  LaunchResult,
  OrbisEventMap,
  OrbisEventName,
  PlaytimeResult,
  Region,
  RemoteVersionResult,
  RestoreResult,
  RuntimeStateDto,
  ScanResult,
  SettingKey,
  ToolAssetDto,
  ToolDetail,
  ToolDto,
  ToolEnableResult,
  UpdateStatusDto,
  WindowAction,
} from './types';

export interface OrbisApi {
  // ── 游戏目录（静态 catalog，契约 §3.1）────────────────
  /** UI 唯一可用的游戏显示名来源；禁止在 UI 侧硬编码游戏目录 */
  listGames(): Promise<GameCatalogEntry[]>;

  // ── 发现与实例管理（A1–A3）────────────────────────────
  scanGames(input?: { rescan?: boolean }): Promise<ScanResult>;
  listInstallations(): Promise<InstallationListResult>;
  getInstallationDetail(installationId: string): Promise<InstallationDetail>;
  /** A2 边界：必须在「确认添加」前调用，用于识别「选到官方启动器而非游戏本体」 */
  validateExecutable(
    gameId: GameId,
    executablePath: string,
  ): Promise<ExecutableValidation>;
  addInstallation(input: {
    gameId: GameId;
    executablePath: string;
    region?: Region;
  }): Promise<InstallationDto>;
  /** 只移除条目与管理数据，不删游戏文件与存档（02 A2） */
  removeInstallation(installationId: string): Promise<void>;

  // ── 启动与运行状态（A4 / A5 / B8）──────────────────────
  launchGame(
    installationId: string,
    opts?: { skipUnlock?: boolean },
  ): Promise<LaunchResult>;
  terminateGame(installationId: string): Promise<void>;
  getRuntimeStates(): Promise<RuntimeStateDto[]>;

  // ── 启动参数（A7）────────────────────────────────────
  getLaunchProfile(installationId: string): Promise<LaunchProfileDto>;
  setLaunchProfile(
    installationId: string,
    args: string,
  ): Promise<LaunchProfileDto>;
  resetLaunchProfile(installationId: string): Promise<void>;

  // ── 时长（A6）───────────────────────────────────────
  getPlaytime(
    scope: 'today' | 'week' | 'total',
    installationId?: string,
  ): Promise<PlaytimeResult>;

  // ── 版本与更新（E1）──────────────────────────────────
  refreshRemoteVersions(input?: {
    force?: boolean;
  }): Promise<RemoteVersionResult[]>;
  getUpdateStatus(gameId?: GameId): Promise<UpdateStatusDto[]>;

  // ── 工具（C1–C3 / B7）───────────────────────────────
  listTools(gameId?: GameId): Promise<ToolDto[]>;
  getTool(toolId: string): Promise<ToolDetail>;
  /** overrideCompat 仅对 L1 在 unknown 下有效；L3 unknown 一律硬阻止（契约 §3.6） */
  setToolEnabled(
    toolId: string,
    enabled: boolean,
    opts?: { overrideCompat?: boolean },
  ): Promise<ToolEnableResult>;
  grantToolAuthorization(
    toolId: string,
    consentTextHash: string,
  ): Promise<void>;
  revokeToolAuthorization(toolId: string): Promise<void>;
  getToolAssetStatus(toolId: string): Promise<ToolAssetDto>;
  downloadToolAsset(toolId: string): Promise<ToolAssetDto>;

  // ── 兼容性（C4 / B6）────────────────────────────────
  getCompatibility(
    gameId: GameId,
    toolId: string,
  ): Promise<CompatibilityDto>;

  // ── 备份与恢复（A8 / B5）─────────────────────────────
  createBackup(
    installationId: string,
    trigger?: BackupTrigger,
  ): Promise<BackupSummary>;
  listBackups(installationId: string): Promise<BackupSummary[]>;
  restoreBackup(backupId: string): Promise<RestoreResult>;
  deleteBackup(backupId: string): Promise<void>;
  getBackupStorageInfo(installationId: string): Promise<BackupStorageInfo>;

  // ── 设置（D5 前置 / E1）─────────────────────────────
  getSettings(): Promise<AppSettings>;
  setSetting(key: SettingKey, value: boolean | number): Promise<AppSettings>;

  // ── 壳层 ────────────────────────────────────────────
  windowControl(action: WindowAction): Promise<void>;

  // ── 事件订阅 ─────────────────────────────────────────
  /** 返回取消订阅函数；组件卸载时必须调用 */
  subscribe<K extends OrbisEventName>(
    event: K,
    handler: (payload: OrbisEventMap[K]) => void,
  ): () => void;
}
