/**
 * 窗口控制按钮（最小化 / 最大化 / 关闭）
 *
 * 经由 api.windowControl 走 IPC（契约 §3.10）—— 不直接调用 Tauri API，
 * 保持「views/components 只依赖 api 接口」的约束（00 §7.9）。
 */
import React from 'react';
import { api } from '../../api';
import type { WindowAction } from '../../api/types';

const BUTTONS: { action: WindowAction; icon: string; label: string; danger?: boolean }[] = [
  { action: 'minimize', icon: 'remove', label: '最小化' },
  { action: 'maximize', icon: 'crop_square', label: '最大化' },
  { action: 'close', icon: 'close', label: '关闭', danger: true },
];

export const WindowControls: React.FC = () => {
  return (
    <div className="flex items-center h-full select-none">
      {BUTTONS.map((b) => (
        <button
          key={b.action}
          type="button"
          aria-label={b.label}
          title={b.label}
          onClick={() => {
            void api.windowControl(b.action);
          }}
          className={`w-12 h-14 flex items-center justify-center text-on-surface-variant transition-colors cursor-pointer ${
            b.danger
              ? 'hover:text-text-primary hover:bg-error-container'
              : 'hover:text-on-surface hover:bg-surface-2'
          }`}
        >
          <span className="material-symbols-outlined text-[18px]">{b.icon}</span>
        </button>
      ))}
    </div>
  );
};
