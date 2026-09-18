/**
 * Orbis API 入口 —— UI 唯一依赖点
 *
 * 选择规则：
 *  - 浏览器 / 非 Tauri 环境 → mock（行为对齐的参照实现，契约 §7.2）
 *  - Tauri 环境 → 真实 IPC；可用 `VITE_ORBIS_API=mock` 强制回退 mock 以便对比调试
 *
 * 禁止在 views/ 与 store/ 中直接 import mock 或 tauri —— 一律经此入口（00 §7.9）。
 */
import type { OrbisApi } from './contract';
import { mockApi } from './mock';
import { tauriApi } from './tauri';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function pickImplementation(): OrbisApi {
  const forced = import.meta.env.VITE_ORBIS_API;
  if (forced === 'mock') return mockApi;
  return isTauriRuntime() ? tauriApi : mockApi;
}

export const api: OrbisApi = pickImplementation();

export * from './types';
export type { OrbisApi } from './contract';
