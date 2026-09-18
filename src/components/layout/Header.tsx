/**
 * 顶栏：字标 + 页签 + 窗口控制
 *
 * 注意：原先此处的扫描进度遥测（IPC 端口 / 延迟 / KERNEL_READY）已移除 ——
 * 那些是不存在于技术设计的虚构指标（见 docs/05 §3 C1/C4）。顶栏不放任何技术遥测。
 */
import React from 'react';
import { useAppStore } from '../../store/useAppStore';
import { OrbisLogo } from '../common/OrbisLogo';
import { WindowControls } from './WindowControls';
import type { ActiveView } from '../../types/ui';

export const Header: React.FC = () => {
  const activeView = useAppStore((s) => s.activeView);
  const setActiveView = useAppStore((s) => s.setActiveView);

  const navItems: { id: ActiveView; label: string }[] = [
    { id: 'home', label: '首页' },
    { id: 'wuthering-waves', label: '鸣潮' },
    { id: 'genshin-impact', label: '原神' },
    { id: 'settings', label: '设置' },
  ];

  return (
    <header className="fixed top-0 left-0 right-0 h-14 bg-surface-1/90 backdrop-blur-md z-50 flex items-center justify-between border-b border-border-hairline select-none">
      <div className="flex items-center h-full px-space-md gap-space-lg">
        <button
          type="button"
          onClick={() => setActiveView('home')}
          className="cursor-pointer"
          aria-label="返回首页"
        >
          <OrbisLogo />
        </button>

        <nav className="flex items-center h-full gap-space-xs">
          {navItems.map((item) => {
            const isActive =
              activeView === item.id ||
              (item.id === 'home' && activeView === 'onboarding');
            return (
              <button
                type="button"
                key={item.id}
                onClick={() => setActiveView(item.id)}
                className={`px-space-md py-1.5 rounded font-label-md text-label-md transition-colors cursor-pointer ${
                  isActive
                    ? 'text-text-primary bg-surface-2'
                    : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-2'
                }`}
              >
                {item.label}
              </button>
            );
          })}

          {/* 首次运行引导（S6）仅在开发构建下可直接进入，便于核对扫描态与空态 */}
          {import.meta.env.DEV && (
            <button
              type="button"
              onClick={() => setActiveView('onboarding')}
              title="开发预览：首次运行引导（不随发行版发布）"
              className="px-2 py-1 rounded text-[11px] text-text-disabled hover:text-text-secondary hover:bg-surface-2/60 transition-colors cursor-pointer"
            >
              预览引导
            </button>
          )}
        </nav>
      </div>

      <div className="flex items-center h-full">
        <WindowControls />
      </div>
    </header>
  );
};
