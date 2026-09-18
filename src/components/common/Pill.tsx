/**
 * 状态徽标（03 §2.2 状态语义映射）
 *
 * 两维状态独立呈现（00 §5.7 / 04 §6.4）：
 *  - GameStatePill   → 互斥主状态（running / broken / update_available / installed）
 *  - CompatPill      → 工具兼容五态（verified / compatible / unknown / incompatible / deprecated）
 * 两者不得合并成单一枚举。
 *
 * 颜色铁律（03 §1.4）：青 = 可用/运行；粉紫 = 工具/需注意；红**仅**留给危险态。
 */
import React from 'react';
import type { GameRuntimeStatus, ToolCompatStatus } from '../../api/types';
import {
  compatAdvice,
  compatTone,
  formatCompatLabel,
  formatRuntimeStatus,
  runtimeTone,
  type Tone,
} from '../../utils/format';

type Size = 'sm' | 'md';

const TONE_STYLE: Record<Tone, { text: string; bg: string; border: string }> = {
  accent: {
    text: 'text-primary-container',
    bg: 'bg-primary-container/10',
    border: 'border-primary-container/30',
  },
  success: {
    text: 'text-primary-fixed-dim',
    bg: 'bg-primary-container/10',
    border: 'border-primary-container/25',
  },
  warn: {
    text: 'text-secondary',
    bg: 'bg-secondary-container/20',
    border: 'border-secondary/30',
  },
  danger: {
    text: 'text-error',
    bg: 'bg-error/10',
    border: 'border-error/30',
  },
  neutral: {
    text: 'text-text-secondary',
    bg: 'bg-surface-2',
    border: 'border-border-hairline',
  },
};

interface BasePillProps {
  tone: Tone;
  label: string;
  /** 前置点 / 图标 */
  dot?: 'pulse' | 'static' | 'none';
  icon?: string;
  size?: Size;
  className?: string;
  title?: string;
}

export const Pill: React.FC<BasePillProps> = ({
  tone,
  label,
  dot = 'none',
  icon,
  size = 'md',
  className = '',
  title,
}) => {
  const s = TONE_STYLE[tone];
  const pad = size === 'sm' ? 'px-2 py-0.5 text-[11px]' : 'px-2.5 py-1 text-xs';
  return (
    <div
      title={title}
      className={`inline-flex items-center gap-1.5 rounded-full border ${s.bg} ${s.border} ${pad} ${className}`}
    >
      {dot === 'pulse' && (
        <span
          className={`w-1.5 h-1.5 rounded-full animate-pulse ${
            tone === 'danger'
              ? 'bg-error'
              : tone === 'warn'
                ? 'bg-secondary'
                : 'bg-primary-container'
          }`}
        />
      )}
      {dot === 'static' && (
        <span
          className={`w-1.5 h-1.5 rounded-full ${
            tone === 'danger'
              ? 'bg-error'
              : tone === 'warn'
                ? 'bg-secondary'
                : tone === 'neutral'
                  ? 'bg-outline'
                  : 'bg-primary-container'
          }`}
        />
      )}
      {icon && (
        <span className="material-symbols-outlined text-[14px] leading-none">{icon}</span>
      )}
      <span className={`font-label-sm ${s.text}`}>{label}</span>
    </div>
  );
};

/** 游戏互斥主状态徽标（模型带由 Core 决定，见 04 §6.4.2） */
export const GameStatePill: React.FC<{
  status: GameRuntimeStatus;
  updateAvailable: boolean;
  size?: Size;
  className?: string;
}> = ({ status, updateAvailable, size = 'md', className }) => {
  const tone = runtimeTone(status);
  const label = formatRuntimeStatus(status);
  const icon =
    status === 'broken'
      ? 'error'
      : status === 'installed' && updateAvailable
        ? 'arrow_circle_up'
        : status === 'running'
          ? undefined
          : status === 'updating' || status === 'repairing'
            ? 'sync'
            : 'check_circle';

  // running 用发光点，其余用图标（03 §2.2）
  if (status === 'running') {
    return (
      <Pill
        tone="accent"
        dot="pulse"
        label={label}
        size={size}
        className={`backdrop-blur-md ${className}`}
      />
    );
  }
  if (status === 'installed' && updateAvailable) {
    return (
      <Pill
        tone="warn"
        dot="static"
        label="可更新"
        size={size}
        className={`backdrop-blur-md ${className}`}
      />
    );
  }
  return (
    <Pill
      tone={tone}
      icon={icon}
      label={label}
      size={size}
      className={`backdrop-blur-md ${className}`}
    />
  );
};

/** 工具兼容态徽标（唯一权威 = seed.json，02 C4） */
export const CompatPill: React.FC<{
  status: ToolCompatStatus;
  size?: Size;
  /** 是否附带行动建议（02 B6 要求 Unknown/Incompatible 必须给建议） */
  withAdvice?: boolean;
  className?: string;
}> = ({ status, size = 'md', withAdvice = false, className }) => {
  const tone = compatTone(status);
  const icon =
    status === 'unknown'
      ? 'help'
      : status === 'incompatible' || status === 'deprecated'
        ? 'block'
        : 'check_circle';
  const advice = withAdvice ? compatAdvice(status) : null;

  return (
    <div className={`inline-flex flex-col gap-1 ${className}`}>
      <Pill
        tone={tone}
        icon={icon}
        label={formatCompatLabel(status)}
        size={size}
        title={advice ?? undefined}
      />
      {advice && (
        <span className="text-[11px] text-text-disabled leading-snug">{advice}</span>
      )}
    </div>
  );
};
