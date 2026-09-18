/**
 * 备份与恢复状态（A8 / B2 / B4 / B5）
 *
 * 契约要点：
 *  - 恢复前 Core 会自动创建 pre_restore 备份（02 A8：任何恢复本身可撤销）
 *  - 空间不足 / 游戏运行中都被 Core 阻止（04 §5.4 / §5.12），本 store 只呈现结果
 */
import { create } from 'zustand';
import { api } from '../api';
import type {
  BackupStorageInfo,
  BackupSummary,
  BackupTrigger,
  OrbisInvokeError,
  RestoreResult,
} from '../api/types';
import { useAppStore } from './useAppStore';
import { toErrorDisplay } from '../utils/errorMessages';

interface BackupsState {
  backups: BackupSummary[];
  storage: BackupStorageInfo | null;
  loading: boolean;
  /** 正在执行恢复/备份的安装实例，用于禁用按钮 */
  busy: boolean;

  load: (installationId: string) => Promise<void>;
  create: (installationId: string, trigger?: BackupTrigger) => Promise<BackupSummary | null>;
  restore: (backupId: string, installationId: string) => Promise<RestoreResult | null>;
  remove: (backupId: string, installationId: string) => Promise<void>;
}

export const useBackupsStore = create<BackupsState>((set, get) => ({
  backups: [],
  storage: null,
  loading: false,
  busy: false,

  load: async (installationId) => {
    set({ loading: true });
    try {
      const [backups, storage] = await Promise.all([
        api.listBackups(installationId),
        api.getBackupStorageInfo(installationId),
      ]);
      set({ backups, storage, loading: false });
    } catch (e) {
      set({ loading: false });
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
      });
    }
  },

  create: async (installationId, trigger) => {
    set({ busy: true });
    try {
      const created = await api.createBackup(installationId, trigger);
      await get().load(installationId);
      useAppStore.getState().pushNotice({
        kind: 'success',
        title: '备份已完成',
        hint: `已保存 ${created.fileCount} 个文件。`,
      });
      return created;
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
        code: (e as OrbisInvokeError)?.code,
      });
      return null;
    } finally {
      set({ busy: false });
    }
  },

  restore: async (backupId, installationId) => {
    set({ busy: true });
    try {
      const res = await api.restoreBackup(backupId);
      await get().load(installationId);
      useAppStore.getState().pushNotice({
        kind: 'success',
        title: '已恢复到此前的配置',
        hint: res.verified
          ? '校验通过。恢复前的状态也已备份，可以再退回来。'
          : '恢复完成，但校验未完全通过，建议查看历史记录。',
      });
      return res;
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({
        kind: 'error',
        title: d.title,
        hint: d.hint,
        code: (e as OrbisInvokeError)?.code,
      });
      return null;
    } finally {
      set({ busy: false });
    }
  },

  remove: async (backupId, installationId) => {
    try {
      await api.deleteBackup(backupId);
      await get().load(installationId);
    } catch (e) {
      const d = toErrorDisplay(e);
      useAppStore.getState().pushNotice({ kind: 'error', title: d.title, hint: d.hint });
    }
  },
}));
