/**
 * S2b · Game Dashboard · 原神（D2 / B7 / B8 / B6）
 *
 * 03 §5.3 规格：与 S2a 同结构；Enhancement 卡片带「实验性」角标、风险 L3（粉紫描边 + 红点）、
 * 按钮为「了解并启用」而非直接启用；**无配置历史卡**（L3 运行时解锁不落盘，A8 显示「不适用」）。
 * 另按 B8 提供「本次不解锁」开关。
 *
 * 已移除（docs/05 §3 C3）：外部无痕挂接（Zero-Trace）、mhyprot2 隔离观察、客户端完整度 100%
 * ——「无痕」字面语义是规避检测，与 00 §8.12 透明原则正面对立；后两者是不存在的指标。
 */
import React, { useEffect, useState } from 'react';
import { useAppStore } from '../store/useAppStore';
import { useGamesStore } from '../store/useGamesStore';
import { useToolsStore } from '../store/useToolsStore';
import { CoverArt } from '../components/common/CoverArt';
import { PageHeader } from '../components/layout/PageHeader';
import { CompatPill, GameStatePill, Pill } from '../components/common/Pill';
import {
  formatDurationLong,
  formatRegion,
  formatRiskLabel,
} from '../utils/format';

const GENSHIN_TOOL_ID = 'orbis-bundled/genshin-fps-unlock';

export const GenshinDashboardView: React.FC = () => {
  const requestTerminate = useAppStore((s) => s.requestTerminate);
  const openL3Modal = useAppStore((s) => s.openL3Modal);
  const openExecutionPanel = useAppStore((s) => s.openExecutionPanel);
  const install = useGamesStore((s) => s.byGameId('genshin-impact'));
  const catalogOf = useGamesStore((s) => s.catalogOf);
  const launchingId = useGamesStore((s) => s.launchingId);
  const launch = useGamesStore((s) => s.launch);

  const tool = useToolsStore((s) => s.tools.find((t) => t.id === GENSHIN_TOOL_ID));
  const loadTools = useToolsStore((s) => s.load);
  const disable = useToolsStore((s) => s.disable);

  const [skipUnlock, setSkipUnlock] = useState(false);

  useEffect(() => {
    void loadTools();
  }, [loadTools]);

  const game = catalogOf('genshin-impact');
  const running = install?.status === 'running';
  const isLaunching = Boolean(install && launchingId === install.id);

  if (!install || !game) {
    return (
      <main className="w-full pt-14 px-margin pb-margin">
        <PageHeader title="原神" subtitle="本机未检出这款游戏" />
      </main>
    );
  }

  const assetNotReady = Boolean(tool?.asset && !tool.asset.downloadUrlConfigured);

  return (
    <main className="w-full pt-14 px-margin pb-margin select-none">
      <div className="flex flex-col w-full max-w-5xl mx-auto py-space-md gap-space-xl">
        <PageHeader />

        {/* 英雄区 */}
        <div className="bg-surface-1 rounded-xl p-space-lg flex flex-col md:flex-row items-start md:items-center justify-between gap-space-lg relative overflow-hidden border border-border-hairline">
          <div className="absolute right-0 top-0 w-96 h-full bg-gradient-to-l from-primary-container/5 via-transparent to-transparent pointer-events-none" />

          <div className="flex flex-col sm:flex-row items-start sm:items-center gap-space-lg z-10">
            <div className="w-28 h-28 rounded-lg overflow-hidden bg-surface-2 shadow-md shrink-0">
              <CoverArt gameId={install.gameId} name={game.name} />
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center gap-space-sm flex-wrap">
                <h1 className="font-headline-xl text-headline-xl text-text-primary tracking-tight">
                  {game.name}
                </h1>
                <span className="px-2 py-0.5 rounded bg-surface-2 text-text-secondary font-label-sm text-label-sm border border-border-hairline">
                  {formatRegion(install.region)}
                </span>
                <span className="px-2 py-0.5 rounded bg-surface-container font-code-sm text-code-sm text-primary-fixed-dim">
                  {install.versionNorm ?? '版本未知'}
                </span>
                <GameStatePill
                  status={install.status}
                  updateAvailable={install.updateAvailable}
                  size="sm"
                />
              </div>

              <p className="font-code-md text-code-md text-text-secondary flex items-center gap-2 flex-wrap">
                {(
                  [
                    ['今日', install.playtime.todaySec],
                    ['本周', install.playtime.weekSec],
                    ['总计', install.playtime.totalSec],
                  ] as const
                ).map(([label, sec], i) => (
                  <React.Fragment key={label}>
                    {i > 0 && <span className="text-text-disabled">/</span>}
                    <span>
                      {label}{' '}
                      <strong className="text-text-primary font-medium">
                        {formatDurationLong(sec)}
                      </strong>
                    </span>
                  </React.Fragment>
                ))}
              </p>

              <div className="flex items-center gap-space-sm mt-1">
                <span className="font-code-sm text-code-sm text-text-disabled">
                  {install.executablePath}
                </span>
              </div>
            </div>
          </div>

          <div className="flex flex-col gap-2 w-full md:w-auto z-10 shrink-0">
            {/* B8：本次不解锁开关（不改变工具启用状态） */}
            {tool?.enabled && (
              <label className="flex items-center justify-end gap-2 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={skipUnlock}
                  onChange={(e) => setSkipUnlock(e.target.checked)}
                  className="peer sr-only"
                />
                <span className="font-label-sm text-label-sm text-text-secondary">
                  本次不解锁
                </span>
                <span
                  className={`w-9 h-5 rounded-full p-0.5 flex items-center transition-colors ${
                    skipUnlock ? 'bg-secondary/40 justify-end' : 'bg-surface-3 justify-start'
                  }`}
                >
                  <span
                    className={`w-4 h-4 rounded-full ${
                      skipUnlock ? 'bg-secondary' : 'bg-outline'
                    }`}
                  />
                </span>
              </label>
            )}

            <button
              type="button"
              disabled={isLaunching}
              onClick={() =>
                running
                  ? requestTerminate(install.id)
                  : void launch(install.id, { skipUnlock })
              }
              className={`w-full md:w-48 h-12 rounded-lg font-headline-md text-headline-md flex items-center justify-center gap-space-sm transition-all active:scale-[0.98] cursor-pointer ${
                running
                  ? 'bg-surface-2 text-primary-container border border-primary-container/40'
                  : 'bg-primary-container text-bg-base hover:bg-primary-fixed-dim shadow-[0_0_24px_rgba(0,229,255,0.35)]'
              }`}
            >
              <span
                className={`material-symbols-outlined text-[22px] ${isLaunching ? 'animate-spin' : ''}`}
              >
                {isLaunching ? 'refresh' : running ? 'sports_esports' : 'play_arrow'}
              </span>
              <span>{isLaunching ? '正在调起…' : running ? '游戏中' : '启动'}</span>
            </button>
          </div>
        </div>

        {/* L3 运行时组件 */}
        <div className="flex flex-col gap-space-md">
          <div className="flex items-center justify-between px-1">
            <div className="flex items-center gap-space-sm">
              <span className="font-headline-md text-headline-md text-text-primary">
                运行时组件
              </span>
              <span className="font-code-sm text-code-sm text-text-disabled uppercase">
                L3 · 默认关闭
              </span>
            </div>
            <span className="font-body-sm text-body-sm text-text-secondary">
              不修改游戏文件
            </span>
          </div>

          <div className="bg-surface-1 rounded-xl p-space-lg relative overflow-hidden border border-border-hairline">
            <div className="absolute left-0 top-0 bottom-0 w-1 bg-secondary shadow-[0_0_12px_rgba(199,125,255,0.4)]" />

            <div className="flex flex-col gap-space-lg">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-space-md">
                <div className="flex items-center gap-space-md min-w-0">
                  <div className="w-10 h-10 rounded-lg bg-surface-2 flex items-center justify-center text-secondary shrink-0">
                    <span className="material-symbols-outlined text-[22px]">speed</span>
                  </div>
                  <div className="flex flex-col min-w-0">
                    <div className="flex items-center gap-space-sm flex-wrap">
                      <span className="font-headline-md text-headline-md text-text-primary">
                        {tool?.name ?? '120 FPS 解锁（运行时）'}
                      </span>
                      <span className="px-2 py-0.5 rounded bg-surface-2 text-text-secondary font-label-sm text-label-sm border border-border-hairline">
                        实验性
                      </span>
                    </div>
                    <span className="font-body-sm text-body-sm text-text-secondary mt-0.5">
                      {tool?.description ??
                        '在游戏运行时解除帧率上限，关闭游戏即恢复原状。'}
                    </span>
                  </div>
                </div>

                <div className="flex items-center gap-space-md self-start sm:self-auto shrink-0">
                  <Pill
                    tone={tool?.enabled ? 'warn' : 'neutral'}
                    dot={tool?.enabled ? 'pulse' : 'static'}
                    label={tool?.enabled ? '已就绪（随启动生效）' : '默认关闭'}
                    size="sm"
                  />
                </div>
              </div>

              {/* 元信息区 */}
              <div className="bg-surface-2 rounded-lg p-space-md flex flex-col gap-3 border border-border-hairline">
                <div className="flex flex-wrap items-center gap-space-md">
                  <Pill
                    tone="warn"
                    dot="static"
                    icon="warning"
                    label={formatRiskLabel(tool?.riskLevel ?? 'L3')}
                    size="sm"
                  />
                  {tool && <CompatPill status={tool.compat.status} withAdvice size="sm" />}
                </div>

                {/* 组件下载步骤（B7 发行包分离必须对用户可见） */}
                <div className="flex items-start gap-3">
                  <span className="material-symbols-outlined text-[16px] text-text-disabled mt-0.5">
                    download
                  </span>
                  <div className="flex flex-col min-w-0">
                    <span className="font-label-sm text-label-sm text-text-secondary">
                      解锁组件
                    </span>
                    <span className="font-body-sm text-body-sm text-text-disabled">
                      {assetNotReady
                        ? '组件尚未发布。启用后需要额外下载一次（主程序不包含它），不会自动下载。'
                        : `来自 Orbis Releases · 附 SHA-256 校验 · 版本 ${tool?.asset?.version ?? '—'}`}
                    </span>
                  </div>
                </div>

                {tool && tool.pendingVerifications.length > 0 && (
                  <span className="font-code-sm text-code-sm text-text-disabled">
                    部分参数待实测确认：{tool.pendingVerifications.join('、')}
                  </span>
                )}
              </div>

              {/* 说明与操作 */}
              <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-space-md pt-space-xs">
                <p className="font-body-sm text-body-sm text-text-secondary flex items-start gap-1.5">
                  <span className="material-symbols-outlined text-secondary text-[16px] mt-0.5">
                    info
                  </span>
                  <span>
                    游戏重启后自动恢复原状，不会修改任何游戏文件。这是社区多年的实践方式，
                    并非官方授权。
                  </span>
                </p>

                <div className="flex items-center gap-2 shrink-0">
                  {tool?.enabled && (
                    <button
                      type="button"
                      onClick={() => void disable(GENSHIN_TOOL_ID)}
                      className="h-10 px-4 rounded-lg bg-surface-2 hover:bg-surface-3 text-text-secondary hover:text-text-primary font-label-md text-label-md transition-colors cursor-pointer"
                    >
                      停用模块
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => {
                      if (tool?.enabled) {
                        openExecutionPanel(GENSHIN_TOOL_ID);
                      } else {
                        openL3Modal(GENSHIN_TOOL_ID);
                      }
                    }}
                    className="h-10 px-5 rounded-lg bg-surface-3 hover:bg-secondary-container/30 text-secondary font-label-md text-label-md flex items-center justify-center gap-space-xs transition-all cursor-pointer"
                  >
                    <span className="material-symbols-outlined text-[18px]">
                      {tool?.enabled ? 'terminal' : 'lock_open'}
                    </span>
                    <span>{tool?.enabled ? '查看执行记录' : '了解并启用'}</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* A8 范围：L3 不落盘 → 明确「不适用」，而不是悄悄没有这个功能（01 A8 验收） */}
        <div className="bg-surface-1 rounded-xl p-space-md flex items-center justify-between gap-4 border border-border-hairline">
          <div className="flex items-center gap-space-md min-w-0">
            <span className="material-symbols-outlined text-[18px] text-text-disabled">
              settings_backup_restore
            </span>
            <div className="flex flex-col min-w-0">
              <span className="font-label-md text-label-md text-text-secondary">
                配置备份
              </span>
              <span className="font-body-sm text-body-sm text-text-disabled">
                该游戏的解锁方式不修改任何配置文件，因此不需要备份与恢复。
              </span>
            </div>
          </div>
          <Pill tone="neutral" label="不适用" size="sm" />
        </div>
      </div>
    </main>
  );
};
