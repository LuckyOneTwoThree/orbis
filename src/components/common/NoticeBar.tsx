/**
 * 全局提示条
 *
 * 「任何降级必须显式可见，禁止静默失败」（04 §8 总原则）。
 * 所有 store 的错误 / 降级都经 useAppStore.pushNotice 汇聚到这里呈现，
 * 文案按错误码映射（utils/errorMessages），不展示 Core 的开发者信息。
 */
import React from 'react';
import { useAppStore } from '../../store/useAppStore';

export const NoticeBar: React.FC = () => {
  const notice = useAppStore((s) => s.notice);
  const clearNotice = useAppStore((s) => s.clearNotice);

  if (!notice) return null;

  const style = {
    error: {
      border: 'border-error/30',
      icon: 'error',
      iconColor: 'text-error',
      label: 'text-error',
    },
    info: {
      border: 'border-border-hairline',
      icon: 'info',
      iconColor: 'text-primary-container',
      label: 'text-text-primary',
    },
    success: {
      border: 'border-primary-container/30',
      icon: 'check_circle',
      iconColor: 'text-primary-container',
      label: 'text-text-primary',
    },
  }[notice.kind];

  return (
    <div className="fixed top-16 left-1/2 -translate-x-1/2 z-[60] w-full max-w-2xl px-margin">
      <div
        role="status"
        className={`flex items-start gap-3 rounded-xl border ${style.border} bg-surface-2/95 backdrop-blur-md px-4 py-3 shadow-xl`}
      >
        <span className={`material-symbols-outlined text-[18px] mt-0.5 ${style.iconColor}`}>
          {style.icon}
        </span>
        <div className="flex-1 min-w-0 flex flex-col gap-0.5">
          <span className={`font-label-md text-label-md ${style.label}`}>{notice.title}</span>
          {notice.hint && (
            <span className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
              {notice.hint}
            </span>
          )}
          {notice.code && (
            <span className="font-code-sm text-code-sm text-text-disabled mt-0.5">
              代码 {notice.code}
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={clearNotice}
          aria-label="关闭提示"
          className="shrink-0 w-7 h-7 rounded flex items-center justify-center text-text-disabled hover:text-text-primary hover:bg-surface-3 transition-colors cursor-pointer"
        >
          <span className="material-symbols-outlined text-[16px]">close</span>
        </button>
      </div>
    </div>
  );
};
