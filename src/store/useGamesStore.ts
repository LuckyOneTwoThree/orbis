/**
 * 游戏安装实例状态（A1–A7）
 *
 * 数据来源：api.listInstallations / getRuntimeStates，之后由 `game:state-changed`
 * 与 `version:refreshed` 事件增量驱动（A5：5 秒内反映）。
 *
 * 注意：`needsAttention` / `attentionReasons` / `updateAvailable` / `versionUnknown`
 * 全部由 Core 计算（04 §6.4.3），**本 store 不做任何业务判定**。
 */
import { create } from 'zustand';
import { api } from '../api';
import type {
  AttentionSummary,
  ExecutableValidation,
  GameCatalogEntry,
  GameId,
  InstallationDto,
  LaunchResult,
  OrbisInvokeError,
  RemoteVersionResult,
  ScanProgressPayload,
  ScanResult,
} from '../api/types';
import { useAppStore } from './useAppStore';
import { toErrorDisplay } from '../utils/errorMessages';

interface GamesState {
  /** 编译期静态游戏目录（来自 Core，UI 不得自带一份） */
  catalog: GameCatalogEntry[];
  installations: InstallationDto[];
  summary: AttentionSummary;
  loading: boolean;
  error: OrbisInvokeError | null;
  /** 启动中的安装实例（用于按钮 loading 态） */
  launchingId: string | null;
  /** 扫描进度（null = 未在扫描） */
  scanProgress: ScanProgressPayload | null;
  lastScan: ScanResult | null;
  /** 按 installationId 存启动参数（A7） */
  launchArgs: Record<string, string>;
  /** 添加游戏表单的提交中标记 */
  mutating: boolean;

  refresh: () => Promise<void>;
  initialize: () => Promise<void>;
  runScan: () => Promise<ScanResult | null>;
  validateExecutable: (
    gameId: GameId,
    executablePath: string,
  ) => Promise<ExecutableValidation | null>;
  addInstallation: (input: {
    gameId: GameId;
    executablePath: string;
  }) => Promise<boolean>;
  removeInstallation: (installationId: string) => Promise<void>;
  checkUpdates: () => Promise<RemoteVersionResult[] | null>;
  launch: (
    installationId: string,
    opts?: { skipUnlock?: boolean },
  ) => Promise<LaunchResult | null>;
  terminate: (installationId: string) => Promise<void>;
  loadLaunchProfile: (installationId: string) => Promise<void>;
  saveLaunchArgs: (installationId: string, args: string) => Promise<boolean>;
  resetLaunchArgs: (installationId: string) => Promise<void>;

  byId: (installationId: string) => InstallationDto | undefined;
  byGameId: (gameId: GameId) => InstallationDto | undefined;
  catalogOf: (gameId: GameId) => GameCatalogEntry | undefined;
}

const EMPTY_SUMMARY: AttentionSummary = {
  total: 0,
  needsAttention: 0,
  updateAvailable: 0,
  broken: 0,
  toolAttention: 0,
};

/** 统一的错误出口：把 Core 错误码转成用户可读提示（契约 §5） */
function reportError(e: unknown, kind: 'error' | 'info' = 'error'): void {
  const d = toErrorDisplay(e);
  useAppStore.getState().pushNotice({
    kind: d.informational ? 'info' : kind,
    title: d.title,
    hint: d.hint,
    code: (e as OrbisInvokeError)?.code,
  });
}

/** StrictMode 下 effect 会执行两次，用模块级标记避免重复订阅 */
let initialized = false;

export const useGamesStore = create<GamesState>((set, get) => ({
  catalog: [],
  installations: [],
  summary: EMPTY_SUMMARY,
  loading: false,
  error: null,
  launchingId: null,
  scanProgress: null,
  lastScan: null,
  launchArgs: {},
  mutating: false,

  refresh: async () => {
    set({ loading: true });
    try {
      const res = await api.listInstallations();
      set({
        installations: res.installations,
        summary: res.summary,
        error: null,
        loading: false,
      });
    } catch (e) {
      set({ error: e as OrbisInvokeError, loading: false });
      reportError(e);
    }
  },

  initialize: async () => {
    if (initialized) {
      await get().refresh();
      return;
    }
    initialized = true;

    api.subscribe('scan:progress', (p) => {
      set({ scanProgress: p.phase === 'done' ? null : p });
    });

    // 运行态 / 更新态 / 版本未知态变化 → 局部更新，避免整表重拉
    api.subscribe('game:state-changed', (p) => {
      set((s) => ({
        installations: s.installations.map((i) =>
          i.id === p.installationId
            ? {
                ...i,
                status: p.status,
                pid: p.pid,
                updateAvailable: p.updateAvailable,
                versionUnknown: p.versionUnknown,
                versionNorm: p.versionNorm,
              }
            : i,
        ),
      }));
      // 需处理聚合与运行态相关，重拉一次摘要（Core 为权威）
      void get().refresh();
    });

    api.subscribe('playtime:updated', (p) => {
      set((s) => ({
        installations: s.installations.map((i) =>
          i.id === p.installationId
            ? { ...i, playtime: { ...i.playtime, todaySec: p.todaySec } }
            : i,
        ),
      }));
    });

    api.subscribe('version:refreshed', () => {
      void get().refresh();
    });

    // 游戏目录与安装列表并行拉取（目录是静态数据，失败不阻塞列表）
    const [catalog] = await Promise.all([
      api.listGames().catch(() => [] as GameCatalogEntry[]),
      get().refresh(),
    ]);
    set({ catalog });
  },

  runScan: async () => {
    set({ scanProgress: { scanId: 'pending', phase: 'registry', scanned: 0, total: 0, currentGameId: null } });
    try {
      const res = await api.scanGames();
      set({ lastScan: res, scanProgress: null });
      await get().refresh();
      return res;
    } catch (e) {
      set({ scanProgress: null });
      reportError(e);
      return null;
    }
  },

  validateExecutable: async (gameId, executablePath) => {
    try {
      return await api.validateExecutable(gameId, executablePath);
    } catch (e) {
      reportError(e);
      return null;
    }
  },

  addInstallation: async (input) => {
    set({ mutating: true });
    try {
      await api.addInstallation(input);
      await get().refresh();
      return true;
    } catch (e) {
      reportError(e);
      return false;
    } finally {
      set({ mutating: false });
    }
  },

  removeInstallation: async (installationId) => {
    try {
      await api.removeInstallation(installationId);
      await get().refresh();
    } catch (e) {
      reportError(e);
    }
  },

  checkUpdates: async () => {
    try {
      const results = await api.refreshRemoteVersions({ force: true });
      const degraded = results.filter((r) => r.version === null);
      useAppStore.getState().pushNotice({
        kind: 'info',
        title: '更新信息已刷新',
        hint:
          degraded.length > 0
            ? `其中 ${degraded.length} 款游戏暂时无法获取官方版本信息，状态显示为「未知」——我们不会用旧数据猜测。`
            : '所有游戏版本信息均已获取。',
      });
      await get().refresh();
      return results;
    } catch (e) {
      reportError(e);
      return null;
    }
  },

  launch: async (installationId, opts) => {
    set({ launchingId: installationId });
    try {
      const res = await api.launchGame(installationId, opts);
      if (res.unlock.degradedReason) {
        // B7 边界：解锁失败降级普通启动，必须显式告知（不阻塞游戏本体）
        useAppStore.getState().pushNotice({
          kind: 'info',
          title: '游戏已启动，但本次未应用解锁',
          hint: '解锁组件暂时不可用。游戏本身不受影响，你仍可正常游玩。',
          code: res.unlock.degradedReason,
        });
      }
      await get().refresh();
      return res;
    } catch (e) {
      reportError(e);
      return null;
    } finally {
      set({ launchingId: null });
    }
  },

  terminate: async (installationId) => {
    try {
      await api.terminateGame(installationId);
      await get().refresh();
    } catch (e) {
      reportError(e);
    }
  },

  loadLaunchProfile: async (installationId) => {
    try {
      const p = await api.getLaunchProfile(installationId);
      set((s) => ({ launchArgs: { ...s.launchArgs, [installationId]: p.args } }));
    } catch (e) {
      reportError(e);
    }
  },

  saveLaunchArgs: async (installationId, args) => {
    try {
      const p = await api.setLaunchProfile(installationId, args);
      set((s) => ({ launchArgs: { ...s.launchArgs, [installationId]: p.args } }));
      return true;
    } catch (e) {
      reportError(e);
      return false;
    }
  },

  resetLaunchArgs: async (installationId) => {
    try {
      await api.resetLaunchProfile(installationId);
      set((s) => ({ launchArgs: { ...s.launchArgs, [installationId]: '' } }));
    } catch (e) {
      reportError(e);
    }
  },

  byId: (installationId) => get().installations.find((i) => i.id === installationId),
  byGameId: (gameId) => get().installations.find((i) => i.gameId === gameId),
  catalogOf: (gameId) => get().catalog.find((g) => g.id === gameId),
}));
