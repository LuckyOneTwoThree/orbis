/**
 * S4 · 增强执行流（D4 透明卡片，B2–B4）
 *
 * 03 §5.6 规格：执行步骤行（已完成 ✓ / 进行中 进度 / 等待中 灰）+ 底部结果区 +
 * 底层信息收入弱化的「高级详情」+ 角落小字「所有操作已写入统一日志」。
 *
 * 本组件**不含任何模拟逻辑**：步骤序列完全由 `tool:apply-step` 事件驱动（04 §5.5），
 * 因此它同时是 Core 状态机的可视化，也是契约联调的验证工具。
 */
import React, { useState } from 'react';
import { useAppStore } from '../../store/useAppStore';
import { useGamesStore } from '../../store/useGamesStore';
import { useToolsStore } from '../../store/useToolsStore';
import type { ApplyStep, ApplyStepState } from '../../api/types';
import type { ApplyStepView } from '../../types/ui';

/** 步骤顺序与未到达时的占位文案（Core 会用 detail.default 覆盖） */
const STEP_ORDER: ApplyStep[] = ['gate', 'precheck', 'backup', 'modify', 'verify', 'rollback'];
const STEP_LABEL: Record<ApplyStep, string> = {
  gate: '检查兼容性',
  precheck: '检查运行环境',
  backup: '备份原配置',
  modify: '应用修改',
  verify: '验证修改结果',
  rollback: '回滚到备份',
};

/** 模块级常量：zustand 选择器必须返回稳定引用，否则每次 render 都是新数组，
 *  在 React 18 的 useSyncExternalStore 下会触发重复渲染甚至死循环。 */
const EMPTY_STEPS: ApplyStepView[] = [];

export const ExecutionFlowModal: React.FC = () => {
  const toolId = useAppStore((s) => s.executionToolId);
  const close = useAppStore((s) => s.closeExecutionPanel);
  const openConfigHistory = useAppStore((s) => s.openConfigHistory);
  const steps = useToolsStore((s) => (toolId ? s.applySteps[toolId] : undefined)) ?? EMPTY_STEPS;
  const outcome = useToolsStore((s) => (toolId ? s.lastOutcome[toolId] : undefined));
  const tool = useToolsStore((s) => (toolId ? s.tools.find((t) => t.id === toolId) : undefined));
  const installs = useGamesStore((s) => s.installations);

  const [showAdvanced, setShowAdvanced] = useState(false);

  if (!toolId) return null;

  const installation = installs.find((i) => i.gameId === tool?.gameId);
  const byStep = new Map(steps.map((s) => [s.step, s]));
  // 只展示已经出现的步骤 + 一条首个未到达的占位
  const visible = STEP_ORDER.map((step) => byStep.get(step)).filter(
    (s): s is ApplyStepView => Boolean(s),
  );
  const nextStep = STEP_ORDER.find((s) => !byStep.has(s));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-margin">
      <button
        type="button"
        aria-label="关闭"
        onClick={close}
        className="fixed inset-0 bg-bg-base/85 backdrop-blur-md z-40 cursor-default"
      />

      <div className="relative z-50 w-full max-w-[620px] bg-surface-1/95 backdrop-blur-xl rounded-xl shadow-2xl p-space-lg md:p-space-xl flex flex-col gap-space-lg border border-border-hairline animate-in fade-in zoom-in-95 duration-200">
        <div className="absolute -top-px left-1/2 -translate-x-1/2 w-32 h-[2px] bg-gradient-to-r from-transparent via-primary-container to-transparent" />

        {/* 头部 */}
        <div className="flex items-start justify-between gap-space-md">
          <div className="flex flex-col gap-space-xs min-w-0">
            <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-primary-container/10 text-primary-container font-code-sm text-code-sm uppercase tracking-wider self-start">
              <span className="w-1.5 h-1.5 rounded-full bg-primary-container animate-pulse" />
              增强执行
            </span>
            <h1 className="font-headline-xl text-headline-xl text-text-primary tracking-tight mt-1">
              正在执行 · 透明可回滚
            </h1>
            <p className="font-body-sm text-body-sm text-text-secondary">
              目标：{tool?.name ?? '工具'}
              {installation ? ` · ${tool?.gameId}` : ''}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setShowAdvanced((v) => !v)}
            className={`shrink-0 h-8 px-3 rounded-lg font-code-sm text-code-sm transition-colors cursor-pointer ${
              showAdvanced
                ? 'bg-surface-3 text-text-primary'
                : 'bg-surface-2 text-text-secondary hover:text-text-primary'
            }`}
          >
            {showAdvanced ? '收起高级详情' : '高级详情'}
          </button>
        </div>

        {/* 步骤流 */}
        <div className="flex flex-col gap-space-sm">
          {visible.length === 0 && (
            <div className="py-10 flex flex-col items-center gap-2">
              <span className="material-symbols-outlined text-[24px] text-text-disabled">
                hourglass_empty
              </span>
              <span className="font-body-sm text-body-sm text-text-disabled">
                还没有执行记录。在游戏面板上启用工具后，这里会逐步显示每一步动作。
              </span>
            </div>
          )}

          {visible.map((s, idx) => (
            <StepRow
              key={s.step}
              index={idx + 1}
              view={s}
              isLast={idx === visible.length - 1 && !nextStep}
              showAdvanced={showAdvanced}
            />
          ))}

          {/* 首个未到达步骤的占位（让用户知道还有几步） */}
          {nextStep && (
            <StepRow
              index={visible.length + 1}
              view={{
                step: nextStep,
                state: 'pending',
                detail: { default: STEP_LABEL[nextStep], advanced: null },
                updatedAt: Date.now(),
              }}
              isLast
              showAdvanced={showAdvanced}
            />
          )}
        </div>

        {/* 结果区 */}
        <div className="rounded-lg bg-surface-2 p-space-md flex flex-col gap-space-xs">
          <div className="flex items-center justify-between gap-space-sm">
            <span className="font-code-md text-code-md text-on-surface font-semibold">
              {outcome === 'applied'
                ? '已完成 · 配置已生效'
                : outcome === 'rolled_back'
                  ? '修改失败，已自动恢复原配置'
                  : outcome === 'blocked'
                    ? '已被安全门控阻止'
                    : '执行中…'}
            </span>
            <span className="text-primary-container font-code-sm text-code-sm whitespace-nowrap">
              AUTO-GUARD ON
            </span>
          </div>
          <p className="font-body-sm text-body-sm text-text-secondary">
            备份、修改与验证的每一步都会写入统一日志（Audit Log）。任何一步失败都会自动还原，
            你可以随时在配置历史里手动恢复到任一节点。
          </p>
        </div>

        {/* 操作 */}
        <div className="flex items-center justify-end gap-space-md pt-space-xs">
          {installation && (
            <button
              type="button"
              onClick={() => {
                close();
                openConfigHistory(installation.gameId);
              }}
              className="h-11 px-space-md rounded bg-surface-2 hover:bg-surface-3 text-text-secondary hover:text-text-primary font-label-md text-label-md flex items-center gap-space-xs transition-colors cursor-pointer"
            >
              <span className="material-symbols-outlined text-[18px]">history</span>
              <span>查看配置历史</span>
            </button>
          )}
          <button
            type="button"
            onClick={close}
            className="h-11 px-space-lg rounded bg-primary-container text-bg-base font-label-md text-label-md flex items-center gap-space-xs transition-all active:scale-[0.98] hover:brightness-110 cursor-pointer"
          >
            <span className="material-symbols-outlined text-[18px]">check</span>
            <span>完成并关闭</span>
          </button>
        </div>
      </div>
    </div>
  );
};

// ── 步骤行 ──────────────────────────────────────────────────

const STATE_STYLE: Record<
  ApplyStepState,
  { ring: string; badge: string; text: string; icon: string }
> = {
  completed: {
    ring: 'bg-primary-container/15 text-primary-container',
    badge: 'bg-primary-container/10 text-primary-container',
    text: 'text-text-primary',
    icon: 'check',
  },
  running: {
    ring: 'bg-primary-container text-bg-base animate-pulse',
    badge: 'bg-primary-container/10 text-primary-container',
    text: 'text-text-primary',
    icon: 'sync',
  },
  failed: {
    ring: 'bg-error text-white',
    badge: 'bg-error/10 text-error',
    text: 'text-text-primary',
    icon: 'close',
  },
  pending: {
    ring: 'bg-surface-variant text-text-disabled',
    badge: 'bg-surface-3 text-text-disabled',
    text: 'text-text-disabled',
    icon: 'hourglass_empty',
  },
  skipped: {
    ring: 'bg-surface-variant text-text-disabled',
    badge: 'bg-surface-3 text-text-disabled',
    text: 'text-text-disabled',
    icon: 'remove',
  },
};

const STATE_LABEL: Record<ApplyStepState, string> = {
  completed: '已完成',
  running: '进行中',
  failed: '失败',
  pending: '等待中',
  skipped: '已跳过',
};

const StepRow: React.FC<{
  index: number;
  view: ApplyStepView;
  isLast: boolean;
  showAdvanced: boolean;
}> = ({ index, view, isLast, showAdvanced }) => {
  const st = STATE_STYLE[view.state];
  return (
    <div
      className={`relative flex items-start gap-space-md p-space-md rounded-lg transition-all ${
        view.state === 'failed'
          ? 'bg-surface-2 border border-error/30'
          : view.state === 'pending'
            ? 'bg-surface-2/40 opacity-60'
            : 'bg-surface-2'
      }`}
    >
      {view.state === 'running' && (
        <div className="absolute left-0 top-0 bottom-0 w-1 rounded-l-lg bg-primary-container" />
      )}

      <div className="flex flex-col items-center pt-0.5">
        <div
          className={`w-7 h-7 rounded-full flex items-center justify-center text-[15px] shrink-0 ${st.ring}`}
        >
          <span className="material-symbols-outlined text-[15px]">{st.icon}</span>
        </div>
        {!isLast && <div className="w-0.5 h-8 bg-surface-bright mt-1" />}
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between gap-space-sm">
          <div className="flex items-center gap-space-xs min-w-0">
            <span className="font-code-sm text-code-sm text-text-disabled font-medium">
              {String(index).padStart(2, '0')}
            </span>
            <span className={`font-headline-md text-headline-md truncate ${st.text}`}>
              {/* 默认层：语义描述，不含文件名 / 键名 / 哈希（03 U8） */}
              {view.detail.default}
            </span>
          </div>
          <span className={`px-2 py-0.5 rounded font-code-sm text-code-sm shrink-0 ${st.badge}`}>
            {STATE_LABEL[view.state]}
          </span>
        </div>

        {/* 高级详情：底层事实只在展开时出现（00 §12.2） */}
        {showAdvanced && view.detail.advanced && view.detail.advanced.length > 0 && (
          <div className="mt-2 rounded bg-surface-3/60 px-3 py-2 flex flex-col gap-0.5">
            {view.detail.advanced.map((line, i) => (
              <span key={i} className="font-code-sm text-code-sm text-text-secondary break-all">
                {line}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
