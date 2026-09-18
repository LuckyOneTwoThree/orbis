/**
 * Orbis Tauri IPC 实现（真实后端接入点）
 *
 * 现状：Rust Core 尚未开工（04 §4.1 的 crates/ 与 src-tauri/ 目前为骨架）。
 * 本文件先把**接线与错误规整**做对：命令名、参数名、错误形状与 `docs/ipc-contract.md`
 * 严格一一对应。Core 落地后无需改动本文件的调用约定 —— 只需 Rust 侧注册同名命令。
 *
 * 接入步骤（每个命令落地时）：
 *   1. Rust 侧实现 #[tauri::command] 并由 tauri-specta 导出（04 §2.1 / Q3）
 *   2. 无需改本文件（参数名已在 invokeCmd 调用中固化）
 *   3. 若 Core 未注册该命令，invoke 会拒绝并由 normalizeError 归为 INTERNAL
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
