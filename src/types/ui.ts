/**
 * UI 层专有类型（不属于 IPC 契约）
 *
 * 契约 DTO 在 src/api/types.ts；本文件只放「视图路由」与「UI 视图态」。
 */
import type { ApplyStep, ApplyStepDetail, ApplyStepState } from '../api/types';

/** S0–S6 页面路由（03 §3） */
export type ActiveView =
  | 'onboarding' // S6
  | 'home' // S1
  | 'wuthering-waves' // S2a
  | 'genshin-impact' // S2b
  | 'generic' // S2c 通用型 Dashboard（无 Enhancement 游戏）
  | 'config-history' // S5 配置历史
  | 'settings'; // D5（P1）

/** 通用型 Dashboard 的目标游戏（S2c 复用同一视图） */
export const GENERIC_DASHBOARD_GAMES = [
  'honkai-star-rail',
  'zenless-zone-zero',
  'arknights-endfield',
] as const;

/** D4 透明卡片的一行步骤（由 tool:apply-step 事件流驱动） */
export interface ApplyStepView {
  step: ApplyStep;
  state: ApplyStepState;
  detail: ApplyStepDetail;
  /** 事件到达时刻，用于排序与「刚刚完成」提示 */
  updatedAt: number;
}
