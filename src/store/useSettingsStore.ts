/**
 * 设置状态（E1 可关 / D5）
 *
 * 铁律（04 §7.4）：设置页**只允许**出现 §6.1 的三个预置键。
 * 安全模型（修改前强制备份 / 修改后验证 / 失败回滚 / L3 授权门槛 / 兼容门控）
 * 一律恒为开启，不得以开关形式暴露。
 */
import { create } from 'zustand';
import { api } from '../api';
import type { AppSettings, OrbisInvokeError, SettingKey } from '../api/types';
import { useAppStore } from './useAppStore';
import { toErrorDisplay } from '../utils/errorMessages';

interface SettingsState {
  settings: AppSettings | null;
  loading: boolean;
  load: () => Promise<void>;
  update: (key: SettingKey, value: boolean | number) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  loading: false,

  load: async () => {
    set({ loading: true });
    try {
      set({ settings: await api.getSettings(), loading: false });
    } catch (e) {
      set({ loading: false });
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({ kind: 'error', title: d.title, hint: d.hint });
    }
  },

  update: async (key, value) => {
    try {
      set({ settings: await api.setSetting(key, value) });
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
        code: (e as OrbisInvokeError)?.code,
      });
      // 值未生效，回读权威值，避免 UI 停在一个假状态
      await get().load();
    }
  },
}));
