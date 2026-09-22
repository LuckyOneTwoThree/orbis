/**
 * Orbis Tauri IPC 实现（真实后端接入点）
 *
 * 本文件只做**接线与错误规整**：命令名、参数名、错误形状与 `docs/ipc-contract.md`
 * 严格一一对应。契约 §7.1 把「未实现命令抛 `NOT_IMPLEMENTED`」的责任明确放在本文件，
 * 因此这里还持有 [`IMPLEMENTED_COMMANDS`] 白名单。
 *
 * 接入一条新命令时：
 *   1. Rust 侧实现 `#[tauri::command]` 并注册进 `src-tauri/src/lib.rs` 的 `generate_handler!`
 *   2. 把命令名加进 [`IMPLEMENTED_COMMANDS`]
 *   3. 下面的调用点不必改 —— 参数名已固化
 *   两步的一致性由 `npm run check:commands` 断言，防止「白名单与 Core 实际注册」漂移。
 *
 * 禁止在此文件出现业务逻辑：它只做「转发 + 错误规整」（00 §7.9）。
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { OrbisApi } from './contract';
import {
  OrbisInvokeError,
  type OrbisError,
  type OrbisEventMap,
  type OrbisEventName,
} from './types';

/**
 * 已由 Rust 侧注册的命令 —— 必须与 `src-tauri/src/lib.rs` 的 `generate_handler!` 一致
 * （由 `npm run check:commands` 断言，否则这里就是又一处静默漂移）。
 *
 * 为什么需要这份列表：Tauri 对**未注册**的命令会拒绝一个字符串，而 `normalizeError`
 * 只能把它归为 `INTERNAL` → UI 显示「出了点问题 + 重试」。契约 §7.1 与 §5 要求的语义是
 * `NOT_IMPLEMENTED`（开发期占位，**发布前必须为 0 处**）：UI 据此显示「功能开发中」，
 * 而不是让用户去点一个注定失败的「重试」。先查表还能省掉一次注定失败的进程间往返。
 *
 * `tauri-specta`（Q3 已拍板、尚未执行）落地后，这份手写列表可由生成物取代。
 */
const IMPLEMENTED_COMMANDS: readonly string[] = [
  'windowControl',
  'listGames',
  'listTools',
  'getCompatibility',
  'listInstallations',
  'getInstallationDetail',
  'removeInstallation',
  'getRuntimeStates',
  'terminateGame',
  'getPlaytime',
  'getLaunchProfile',
  'setLaunchProfile',
  'resetLaunchProfile',
  'listBackups',
  'deleteBackup',
  'getBackupStorageInfo',
  'getSettings',
  'setSetting',
];

/** 把任意 rejecting 值规整为契约 §5 的 OrbisError 形状 */
function normalizeError(raw: unknown): OrbisInvokeError {
  if (raw instanceof OrbisInvokeError) return raw;
  if (raw && typeof raw === 'object' && 'code' in raw) {
    return new OrbisInvokeError(raw as OrbisError);
  }
  return new OrbisInvokeError({
    code: 'INTERNAL',
    message: typeof raw === 'string' ? raw : 'unknown ipc failure',
    detail: null,
    retryable: false,
  });
}

/** 从 OrbisApi 接口反推命令返回类型，避免任何类型断言 */
type ApiReturn<K extends keyof OrbisApi> = Awaited<ReturnType<OrbisApi[K]>>;

async function invokeCmd<K extends keyof OrbisApi>(
  cmd: K,
  args?: Record<string, unknown>,
): Promise<ApiReturn<K>> {
  // 未实现的命令在发出 IPC 之前就给出契约规定的错误码（见 IMPLEMENTED_COMMANDS 的文档）
  if (!IMPLEMENTED_COMMANDS.includes(cmd as string)) {
    throw new OrbisInvokeError({
      code: 'NOT_IMPLEMENTED',
      message: `命令 ${String(cmd)} 尚未在 Core 实现`,
      detail: null,
      retryable: false,
    });
  }
  try {
    return await invoke<ApiReturn<K>>(cmd as string, args);
  } catch (e) {
    throw normalizeError(e);
  }
}

export const tauriApi: OrbisApi = {
  // ── 游戏目录 ────────────────────────────────────────
  listGames: () => invokeCmd('listGames'),

  // ── 发现与实例管理 ──────────────────────────────────
  scanGames: (input) => invokeCmd('scanGames', { input }),
  listInstallations: () => invokeCmd('listInstallations'),
  getInstallationDetail: (installationId) =>
    invokeCmd('getInstallationDetail', { installationId }),
  validateExecutable: (gameId, executablePath) =>
    invokeCmd('validateExecutable', { gameId, executablePath }),
  addInstallation: (input) => invokeCmd('addInstallation', { input }),
  removeInstallation: (installationId) =>
    invokeCmd('removeInstallation', { installationId }),

  // ── 启动与运行状态 ──────────────────────────────────
  launchGame: (installationId, opts) => invokeCmd('launchGame', { installationId, opts }),
  terminateGame: (installationId) => invokeCmd('terminateGame', { installationId }),
  getRuntimeStates: () => invokeCmd('getRuntimeStates'),

  // ── 启动参数 ────────────────────────────────────────
  getLaunchProfile: (installationId) => invokeCmd('getLaunchProfile', { installationId }),
  setLaunchProfile: (installationId, args) =>
    invokeCmd('setLaunchProfile', { installationId, args }),
  resetLaunchProfile: (installationId) =>
    invokeCmd('resetLaunchProfile', { installationId }),

  // ── 时长 ────────────────────────────────────────────
  getPlaytime: (scope, installationId) =>
    invokeCmd('getPlaytime', { scope, installationId }),

  // ── 版本与更新 ──────────────────────────────────────
  refreshRemoteVersions: (input) => invokeCmd('refreshRemoteVersions', { input }),
  getUpdateStatus: (gameId) => invokeCmd('getUpdateStatus', { gameId }),

  // ── 工具 ────────────────────────────────────────────
  listTools: (gameId) => invokeCmd('listTools', { gameId }),
  getTool: (toolId) => invokeCmd('getTool', { toolId }),
  setToolEnabled: (toolId, enabled, opts) =>
    invokeCmd('setToolEnabled', { toolId, enabled, opts }),
  grantToolAuthorization: (toolId, consentTextHash) =>
    invokeCmd('grantToolAuthorization', { toolId, consentTextHash }),
  revokeToolAuthorization: (toolId) => invokeCmd('revokeToolAuthorization', { toolId }),
  getToolAssetStatus: (toolId) => invokeCmd('getToolAssetStatus', { toolId }),
  downloadToolAsset: (toolId) => invokeCmd('downloadToolAsset', { toolId }),

  // ── 兼容性 ──────────────────────────────────────────
  getCompatibility: (gameId, toolId) => invokeCmd('getCompatibility', { gameId, toolId }),

  // ── 备份与恢复 ──────────────────────────────────────
  createBackup: (installationId, trigger) =>
    invokeCmd('createBackup', { installationId, trigger }),
  listBackups: (installationId) => invokeCmd('listBackups', { installationId }),
  restoreBackup: (backupId) => invokeCmd('restoreBackup', { backupId }),
  deleteBackup: (backupId) => invokeCmd('deleteBackup', { backupId }),
  getBackupStorageInfo: (installationId) =>
    invokeCmd('getBackupStorageInfo', { installationId }),

  // ── 设置 ────────────────────────────────────────────
  getSettings: () => invokeCmd('getSettings'),
  setSetting: (key, value) => invokeCmd('setSetting', { key, value }),

  // ── 壳层 ────────────────────────────────────────────
  windowControl: (action) => invokeCmd('windowControl', { action }),

  // ── 事件订阅 ────────────────────────────────────────
  subscribe<K extends OrbisEventName>(
    event: K,
    handler: (payload: OrbisEventMap[K]) => void,
  ): () => void {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void listen<OrbisEventMap[K]>(event, (e) => handler(e.payload)).then((fn) => {
      // 订阅回调可能在组件卸载后才 resolve，此时立即注销，避免事件泄漏
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  },
};
