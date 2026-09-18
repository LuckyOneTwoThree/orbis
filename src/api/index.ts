/**
 * Orbis API 入口 —— UI 唯一依赖点
 *
 * 选择规则：
 *  - 浏览器 / 非 Tauri 环境 → mock（行为对齐的参照实现，契约 §7.2）
 *  - Tauri 环境 → 真实 IPC；可用 `VITE_ORBIS_API=mock` 强制回退 mock
 *
 * `VITE_ORBIS_API=mock` 在打包时被固化，因此可用于产出「UI 预览包」——在 Rust Core
 * 尚未实现命令的阶段，这是唯一能让安装包真正可用（而非每一步都报错）的方式。
 * `apiKind` 对外暴露该事实，界面必须据此显示预览标记（不允许把演示数据当真实数据展示）。
 *
 * 禁止在 views/ 与 store/ 中直接 import mock 或 tauri —— 一律经此入口（00 §7.9）。
 */
import type { OrbisApi } from './contract';
import { mockApi } from './mock';
import { tauriApi } from './tauri';

export type ApiKind = 'mock' | 'tauri';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function pickImplementation(): { api: OrbisApi; kind: ApiKind } {
  const forced = import.meta.env.VITE_ORBIS_API;
  if (forced === 'mock') return { api: mockApi, kind: 'mock' };
  if (forced === 'tauri') return { api: tauriApi, kind: 'tauri' };
  return isTauriRuntime()
    ? { api: tauriApi, kind: 'tauri' }
    : { api: mockApi, kind: 'mock' };
}

const picked = pickImplementation();

export const api: OrbisApi = picked.api;

/** 当前生效的实现。UI 用它判断是否需要显示「预览构建」标记。 */
export const apiKind: ApiKind = picked.kind;

export * from './types';
export type { OrbisApi } from './contract';
