/**
 * 工具状态与 L1 执行流（C1–C3 / B1–B4 / B7）
 *
 * D4 透明卡片由 `tool:apply-step` 事件逐步驱动（04 §5.5）：
 * 每收到一帧事件就更新对应步骤的 state 与两层 detail，UI 只做渲染。
 */
import { create } from 'zustand';
import { api } from '../api';
import type {
  GameId,
  OrbisInvokeError,
  ToolAssetDto,
  ToolDetail,
  ToolDto,
  ToolEnableResult,
} from '../api/types';
import type { ApplyStepView } from '../types/ui';
import { useAppStore } from './useAppStore';
import { toErrorDisplay } from '../utils/errorMessages';

interface ToolsState {
  tools: ToolDto[];
  /** 按 toolId 存 L1 执行流的当前步骤序列 */
  applySteps: Record<string, ApplyStepView[]>;
  /** 按 toolId 存最近一次启用结果（驱动成功 / 回滚提示） */
  lastOutcome: Record<string, ToolEnableResult['outcome'] | undefined>;
  /** 正在执行链路的工具 */
  applyingToolId: string | null;
  loading: boolean;
  error: OrbisInvokeError | null;

  load: (gameId?: GameId) => Promise<void>;
  initialize: () => Promise<void>;
  enable: (
    toolId: string,
    opts?: { overrideCompat?: boolean },
  ) => Promise<ToolEnableResult | null>;
  disable: (toolId: string) => Promise<ToolEnableResult | null>;
  clearApplySteps: (toolId: string) => void;
  getDetail: (toolId: string) => Promise<ToolDetail | null>;
  grantAuthorization: (toolId: string, consentTextHash: string) => Promise<boolean>;
  revokeAuthorization: (toolId: string) => Promise<void>;
  getAssetStatus: (toolId: string) => Promise<ToolAssetDto | null>;
  downloadAsset: (toolId: string) => Promise<ToolAssetDto | null>;
  toolsOfGame: (gameId: GameId) => ToolDto[];
}

let initialized = false;

export const useToolsStore = create<ToolsState>((set, get) => ({
  tools: [],
  applySteps: {},
  lastOutcome: {},
  applyingToolId: null,
  loading: false,
  error: null,

  load: async (gameId) => {
    set({ loading: true });
    try {
      const tools = await api.listTools(gameId);
      set((s) => ({
        // 按 gameId 过滤加载时，合并进已有列表而非替换
        tools: gameId
          ? [...s.tools.filter((t) => t.gameId !== gameId), ...tools]
          : tools,
        loading: false,
        error: null,
      }));
    } catch (e) {
      set({ error: e as OrbisInvokeError, loading: false });
    }
  },

  initialize: async () => {
    if (initialized) {
      await get().load();
      return;
    }
    initialized = true;

    api.subscribe('tool:apply-step', (p) => {
      set((s) => {
        const prev = s.applySteps[p.toolId] ?? [];
        const next: ApplyStepView = {
          step: p.step,
          state: p.state,
          detail: p.detail,
          updatedAt: Date.now(),
        };
        const idx = prev.findIndex((v) => v.step === p.step);
        const merged =
          idx >= 0
            ? prev.map((v, i) => (i === idx ? next : v))
            : [...prev, next];
        return { applySteps: { ...s.applySteps, [p.toolId]: merged } };
      });
    });

    await get().load();
  },

  enable: async (toolId, opts) => {
    set({ applyingToolId: toolId, applySteps: { ...get().applySteps, [toolId]: [] } });
    try {
      const res = await api.setToolEnabled(toolId, true, opts);
      set((s) => ({
        lastOutcome: { ...s.lastOutcome, [toolId]: res.outcome },
        tools: s.tools.map((t) => (t.id === toolId ? { ...t, enabled: res.enabled } : t)),
      }));
      if (res.outcome === 'rolled_back') {
        useAppStore.getState().pushNotice({
          kind: 'error',
          title: '修改失败，已自动恢复原配置',
          hint: '验证未通过，配置已还原到修改前的状态，游戏不受影响。',
        });
      }
      return res;
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: d.informational ? 'info' : 'error',
        title: d.title,
        hint: d.hint,
        code: (e as OrbisInvokeError)?.code,
      });
      return null;
    } finally {
      set({ applyingToolId: null });
    }
  },

  disable: async (toolId) => {
    try {
      const res = await api.setToolEnabled(toolId, false);
      set((s) => ({
        lastOutcome: { ...s.lastOutcome, [toolId]: res.outcome },
        tools: s.tools.map((t) => (t.id === toolId ? { ...t, enabled: false } : t)),
      }));
      return res;
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
      });
      return null;
    }
  },

  clearApplySteps: (toolId) =>
    set((s) => ({ applySteps: { ...s.applySteps, [toolId]: [] } })),

  getDetail: async (toolId) => {
    try {
      return await api.getTool(toolId);
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
      });
      return null;
    }
  },

  grantAuthorization: async (toolId, consentTextHash) => {
    try {
      await api.grantToolAuthorization(toolId, consentTextHash);
      return true;
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
      });
      return false;
    }
  },

  revokeAuthorization: async (toolId) => {
    try {
      await api.revokeToolAuthorization(toolId);
      set((s) => ({
        tools: s.tools.map((t) => (t.id === toolId ? { ...t, enabled: false } : t)),
      }));
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({ kind: 'error', title: d.title, hint: d.hint });
    }
  },

  getAssetStatus: async (toolId) => {
    try {
      return await api.getToolAssetStatus(toolId);
    } catch {
      return null;
    }
  },

  downloadAsset: async (toolId) => {
    try {
      const res = await api.downloadToolAsset(toolId);
      return res;
    } catch (e) {
      const d = toErrorDisplay(e);
      // TOOL_ASSET_NOT_CONFIGURED 是降级路径而非错误（契约 §5）
      useAppStore.getState().pushNotice({
        kind: d.informational ? 'info' : 'error',
        title: d.title,
        hint: d.hint,
        code: (e as OrbisInvokeError)?.code,
      });
      return null;
    }
  },

  toolsOfGame: (gameId) => get().tools.filter((t) => t.gameId === gameId),
}));
