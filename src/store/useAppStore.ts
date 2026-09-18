/**
 * UI 状态（导航 / 模态 / 全局提示）
 *
 * 职责边界：只存 UI 状态，不存后端数据（那些在 useGamesStore / useToolsStore / ...）。
 * 全局提示是「降级必须显式可见」的统一出口（04 §8 总原则）。
 */
import { create } from 'zustand';
import type { GameId } from '../api/types';
import type { ActiveView } from '../types/ui';

export interface Notice {
  kind: 'error' | 'info' | 'success';
  title: string;
  hint?: string;
  /** 原始错误码，便于用户反馈时定位；不直接展示给用户 */
  code?: string;
}

interface AppState {
  // ── 导航 ────────────────────────────────────────────
  activeView: ActiveView;
  /** S2c / S5 的目标游戏（Dashboard 与配置历史复用同一视图） */
  activeGameId: GameId | null;
  setActiveView: (view: ActiveView) => void;
  openGameDashboard: (gameId: GameId) => void;
  openConfigHistory: (gameId: GameId) => void;
  goHome: () => void;

  // ── S3：L3 风险授权模态 ──────────────────────────────
  l3ModalToolId: string | null;
  openL3Modal: (toolId: string) => void;
  closeL3Modal: () => void;

  // ── S4：D4 透明执行流面板 ────────────────────────────
  executionToolId: string | null;
  openExecutionPanel: (toolId: string) => void;
  closeExecutionPanel: () => void;

  // ── A4：结束进程的风险确认 ───────────────────────────
  terminateConfirmId: string | null;
  requestTerminate: (installationId: string) => void;
  cancelTerminate: () => void;

  // ── 全局提示 ────────────────────────────────────────
  notice: Notice | null;
  pushNotice: (n: Notice) => void;
  clearNotice: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  activeView: 'home',
  activeGameId: null,
  setActiveView: (view) => set({ activeView: view }),
  openGameDashboard: (gameId) => set({ activeView: 'generic', activeGameId: gameId }),
  openConfigHistory: (gameId) => set({ activeView: 'config-history', activeGameId: gameId }),
  goHome: () => set({ activeView: 'home', activeGameId: null }),

  l3ModalToolId: null,
  openL3Modal: (toolId) => set({ l3ModalToolId: toolId }),
  closeL3Modal: () => set({ l3ModalToolId: null }),

  executionToolId: null,
  openExecutionPanel: (toolId) => set({ executionToolId: toolId }),
  closeExecutionPanel: () => set({ executionToolId: null }),

  terminateConfirmId: null,
  requestTerminate: (installationId) => set({ terminateConfirmId: installationId }),
  cancelTerminate: () => set({ terminateConfirmId: null }),

  notice: null,
  pushNotice: (n) => set({ notice: n }),
  clearNotice: () => set({ notice: null }),
}));
