/**
 * 页面标题栏：返回首页 + 标题 + 右侧插槽
 * 用于 S2a/S2b/S2c/S5/D5 等二级页面，保证返回路径与间距一致。
 */
import React from 'react';
import { useAppStore } from '../../store/useAppStore';

interface PageHeaderProps {
  /** 页面主标题（不传则只显示返回按钮） */
  title?: string;
  subtitle?: string;
  right?: React.ReactNode;
}

export const PageHeader: React.FC<PageHeaderProps> = ({ title, subtitle, right }) => {
  const goHome = useAppStore((s) => s.goHome);

  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex flex-col gap-2 min-w-0">
        <button
          type="button"
          onClick={goHome}
          className="inline-flex items-center gap-1 text-text-secondary hover:text-text-primary transition-colors font-label-md text-label-md group cursor-pointer self-start"
        >
          <span className="material-symbols-outlined text-[18px] group-hover:-translate-x-0.5 transition-transform">
            arrow_back
          </span>
          <span>首页</span>
        </button>
        {title && (
          <div className="flex flex-col">
            <h1 className="font-headline-xl text-headline-xl text-text-primary tracking-tight">
              {title}
            </h1>
            {subtitle && (
              <p className="font-body-sm text-body-sm text-text-secondary mt-0.5">
                {subtitle}
              </p>
            )}
          </div>
        )}
      </div>
      {right && <div className="shrink-0 pt-1">{right}</div>}
    </div>
  );
};
