/**
 * S1 · 首页（D1：三问一处可见）
 *
 *   我有哪些游戏？ → 全部游戏网格（catalog 驱动，含未安装）
 *   能直接启动吗？ → 卡片上的主状态徽标 + 主 CTA
 *   哪些需要处理？ → 顶部摘要条 + 卡片上的「待处理」原因
 *
 * 全部判定来自 Core（04 §6.4.3）：本视图只渲染 summary 与 attentionReasons，不自行计算。
 *
 * 已移除（docs/05 §3）：核心环境监控（Kernel Bridge）、驱动重置、诊断报告、IPC 遥测、
 * Diff 体积、「更新并启动」——这些或不存在于技术设计，或属 P1/不做范围。
 */
import React, { useMemo, useState } from 'react';
import { useAppStore } from '../store/useAppStore';
import { useGamesStore } from '../store/useGamesStore';
import { CoverArt } from '../components/common/CoverArt';
import { GameStatePill, Pill } from '../components/common/Pill';
import { formatDuration, formatRegion } from '../utils/format';
import type { AttentionReason, GameCatalogEntry, InstallationDto } from '../api/types';

const REASON_LABEL: Record<AttentionReason, string> = {
  update_available: '有可用更新',
  broken: '游戏异常',
  version_unknown: '版本无法识别',
  tool_unknown: '工具未验证',
  tool_incompatible: '工具不兼容',
};

export const HomeView: React.FC = () => {
  const setActiveView = useAppStore((s) => s.setActiveView);
  const openGameDashboard = useAppStore((s) => s.openGameDashboard);
  const requestTerminate = useAppStore((s) => s.requestTerminate);
  const summary = useGamesStore((s) => s.summary);
  const catalog = useGamesStore((s) => s.catalog);
  const installations = useGamesStore((s) => s.installations);
  const launchingId = useGamesStore((s) => s.launchingId);
  const launch = useGamesStore((s) => s.launch);
  const checkUpdatesAction = useGamesStore((s) => s.checkUpdates);

  const [onlyAttention, setOnlyAttention] = useState(false);
  const [checking, setChecking] = useState(false);

  const installed = useMemo(
    () => catalog.filter((g) => installations.some((i) => i.gameId === g.id)),
    [catalog, installations],
  );

  const recent = useMemo(
    () =>
      [...installations]
        .filter((i) => i.playtime.totalSec > 0)
        .sort((a, b) => b.playtime.totalSec - a.playtime.totalSec)
        .slice(0, 3),
    [installations],
  );

  const visibleCatalog = onlyAttention
    ? catalog.filter((g) => {
        const inst = installations.find((i) => i.gameId === g.id);
        return inst?.needsAttention;
      })
    : catalog;

  const checkUpdates = async () => {
    setChecking(true);
    try {
      await checkUpdatesAction();
    } finally {
      setChecking(false);
    }
  };

  const openGame = (g: GameCatalogEntry) => {
    if (g.id === 'wuthering-waves' || g.id === 'genshin-impact') {
      setActiveView(g.id);
      return;
    }
    openGameDashboard(g.id);
  };

  return (
    <main className="w-full pt-14 px-margin pb-margin">
      <div className="flex flex-col w-full space-y-8">
        {/* 需处理摘要条（D1 验收：不进二级页可见） */}
        {summary.needsAttention > 0 && (
          <button
            type="button"
            onClick={() => setOnlyAttention((v) => !v)}
            className={`w-full flex items-center justify-between px-5 py-3 rounded-xl bg-surface-1 transition-all duration-200 cursor-pointer group border ${
              onlyAttention ? 'border-secondary/40' : 'border-border-hairline hover:border-outline-variant'
            }`}
          >
            <div className="flex items-center space-x-3">
              <div className="w-2 h-2 rounded-full bg-secondary shadow-[0_0_10px_rgba(199,125,255,0.8)]" />
              <div className="flex items-center space-x-2 flex-wrap">
                <span className="font-headline-md text-headline-md text-text-primary">
                  {summary.needsAttention} 个游戏需要处理
                </span>
                {summary.updateAvailable > 0 && (
                  <>
                    <span className="text-text-disabled font-body-sm">•</span>
                    <span className="font-body-md text-body-md text-text-secondary">
                      {summary.updateAvailable} 个有可用更新
                    </span>
                  </>
                )}
                {summary.toolAttention > 0 && (
                  <>
                    <span className="text-text-disabled font-body-sm">•</span>
                    <span className="font-body-md text-body-md text-text-secondary">
                      {summary.toolAttention} 个工具需关注
                    </span>
                  </>
                )}
              </div>
            </div>
            <div className="flex items-center space-x-1.5 text-secondary group-hover:text-primary transition-colors">
              <span className="font-label-md text-label-md tracking-wide">
                {onlyAttention ? '显示全部' : '只看待处理'}
              </span>
              <span className="material-symbols-outlined text-sm">
                {onlyAttention ? 'filter_list_off' : 'filter_list'}
              </span>
            </div>
          </button>
        )}

        {/* 概览标头 */}
        <div className="flex items-end justify-between py-2">
          <div className="flex items-baseline space-x-6">
            <div className="flex flex-col">
              <span className="font-code-sm text-code-sm text-text-disabled tracking-widest uppercase">
                Overview
              </span>
              <h1 className="font-headline-xl text-headline-xl text-text-primary tracking-tight">
                三问一处可见
              </h1>
            </div>
            <div className="hidden sm:flex items-center space-x-4 font-code-sm text-code-sm text-text-secondary">
              <span className="flex items-center space-x-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-primary-container" />
                <span>已安装 {installed.length}</span>
              </span>
              <span className="flex items-center space-x-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-outline" />
                <span>未安装 {Math.max(0, catalog.length - installed.length)}</span>
              </span>
              <span className="flex items-center space-x-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-secondary" />
                <span>待处理 {summary.needsAttention}</span>
              </span>
            </div>
          </div>

          <button
            type="button"
            disabled={checking}
            onClick={() => void checkUpdates()}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-1 border border-border-hairline font-label-sm text-label-sm text-text-secondary hover:text-text-primary transition-colors cursor-pointer disabled:opacity-50"
          >
            <span
              className={`material-symbols-outlined text-[16px] ${checking ? 'animate-spin' : ''}`}
            >
              {checking ? 'sync' : 'refresh'}
            </span>
            <span>{checking ? '检查中…' : '检查更新'}</span>
          </button>
        </div>

        {/* 最近游玩 */}
        {recent.length > 0 && !onlyAttention && (
          <section className="flex flex-col space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-1 h-3.5 bg-primary-container rounded-full" />
                <h2 className="font-headline-lg text-headline-lg text-text-primary">最近游玩</h2>
              </div>
              <span className="font-code-sm text-code-sm text-text-disabled uppercase tracking-wider">
                Recent Activity
              </span>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {recent.map((inst) => (
                <RecentCard
                  key={inst.id}
                  inst={inst}
                  game={catalog.find((g) => g.id === inst.gameId)}
                  launching={launchingId === inst.id}
                  onOpen={() => openGame(catalog.find((g) => g.id === inst.gameId)!)}
                  onPrimary={() => {
                    if (inst.status === 'running') requestTerminate(inst.id);
                    else void launch(inst.id);
                  }}
                />
              ))}
            </div>
          </section>
        )}

        {/* 全部游戏 */}
        <section className="flex flex-col space-y-4 pt-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-3">
              <div className="flex items-center space-x-2">
                <span className="w-1 h-3.5 bg-outline rounded-full" />
                <h2 className="font-headline-lg text-headline-lg text-text-primary">全部游戏</h2>
              </div>
              <span className="px-2 py-0.5 rounded-full bg-surface-2 font-code-sm text-code-sm text-text-secondary border border-border-hairline">
                {catalog.length} 款支持
              </span>
            </div>
            <span className="font-code-sm text-code-sm text-text-disabled">
              时长口径：游戏进程存活时长
            </span>
          </div>

          {visibleCatalog.length === 0 ? (
            <div className="w-full py-14 rounded-xl bg-surface-1 border border-border-hairline flex flex-col items-center gap-2">
              <span className="material-symbols-outlined text-[28px] text-text-disabled">
                check_circle
              </span>
              <span className="font-headline-md text-headline-md text-text-primary">
                没有需要处理的游戏
              </span>
              <button
                type="button"
                onClick={() => setOnlyAttention(false)}
                className="font-label-sm text-label-sm text-primary-container hover:underline cursor-pointer mt-1"
              >
                显示全部游戏
              </button>
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
              {visibleCatalog.map((g) => {
                const inst = installations.find((i) => i.gameId === g.id);
                return (
                  <AllGameCard
                    key={g.id}
                    game={g}
                    inst={inst}
                    launching={Boolean(inst && launchingId === inst.id)}
                    onOpen={() => openGame(g)}
                    onPrimary={() => {
                      if (!inst) return openGame(g);
                      if (inst.status === 'running') requestTerminate(inst.id);
                      else void launch(inst.id);
                    }}
                  />
                );
              })}
            </div>
          )}
        </section>
      </div>
    </main>
  );
};

// ── 卡片 ────────────────────────────────────────────────────

const RecentCard: React.FC<{
  inst: InstallationDto;
  game?: GameCatalogEntry;
  launching: boolean;
  onOpen: () => void;
  onPrimary: () => void;
}> = ({ inst, game, launching, onOpen, onPrimary }) => {
  const running = inst.status === 'running';
  return (
    <div className="group relative flex flex-col bg-surface-1 rounded-xl p-5 transition-all duration-300 hover:bg-surface-2 shadow-sm border border-border-hairline">
      <button
        type="button"
        onClick={onOpen}
        className="relative w-full aspect-video rounded-lg overflow-hidden bg-surface-container-lowest mb-4 cursor-pointer"
      >
        <CoverArt gameId={inst.gameId} name={game?.name ?? inst.gameId} />
        <div className="absolute inset-0 bg-gradient-to-t from-bg-base/90 via-transparent to-transparent" />
        <div className="absolute top-3 left-3 flex items-center space-x-2">
          <span className="px-2 py-0.5 rounded bg-surface-1/80 backdrop-blur-md font-label-sm text-label-sm text-text-secondary">
            {formatRegion(inst.region)}
          </span>
          <span className="px-2 py-0.5 rounded bg-surface-container-high/70 backdrop-blur-md font-code-sm text-code-sm text-text-secondary">
            {inst.localVersion ?? '版本未知'}
          </span>
        </div>
        <div className="absolute top-3 right-3">
          <GameStatePill status={inst.status} updateAvailable={inst.updateAvailable} />
        </div>
        <div className="absolute bottom-3 left-3 right-3 flex items-end justify-between">
          <span className="font-code-sm text-code-sm text-text-secondary">
            {running && inst.pid ? `PID ${inst.pid}` : '未运行'}
          </span>
          <span className="font-code-sm text-code-sm text-primary-fixed-dim">
            今日 {formatDuration(inst.playtime.todaySec)}
          </span>
        </div>
      </button>

      <div className="flex items-center justify-between">
        <button type="button" onClick={onOpen} className="cursor-pointer text-left min-w-0">
          <h3 className="font-headline-md text-headline-md text-text-primary group-hover:text-primary-container transition-colors truncate">
            {game?.name ?? inst.gameId}
          </h3>
          <p className="font-body-sm text-body-sm text-text-secondary mt-0.5 truncate">
            总计 {formatDuration(inst.playtime.totalSec)}
          </p>
        </button>

        <button
          type="button"
          disabled={launching}
          onClick={onPrimary}
          className={`h-10 px-4 rounded-lg transition-all font-label-md text-label-md font-bold flex items-center space-x-1.5 cursor-pointer active:scale-95 shrink-0 ${
            running
              ? 'bg-surface-2 text-primary-container'
              : 'bg-primary-container text-bg-base hover:brightness-110 shadow-[0_0_20px_rgba(0,229,255,0.3)]'
          }`}
        >
          <span
            className={`material-symbols-outlined text-base ${launching ? 'animate-spin' : ''}`}
          >
            {launching ? 'refresh' : running ? 'stop_circle' : 'play_arrow'}
          </span>
          <span>{launching ? '调起中…' : running ? '结束' : '启动'}</span>
        </button>
      </div>
    </div>
  );
};

const AllGameCard: React.FC<{
  game: GameCatalogEntry;
  inst?: InstallationDto;
  launching: boolean;
  onOpen: () => void;
  onPrimary: () => void;
}> = ({ game, inst, launching, onOpen, onPrimary }) => (
  <div className="group flex flex-col bg-surface-1 rounded-xl p-3.5 transition-all duration-200 hover:bg-surface-2 shadow-sm border border-border-hairline">
    <button
      type="button"
      onClick={onOpen}
      className="relative w-full aspect-[4/3] rounded-lg overflow-hidden bg-surface-container-lowest mb-3 cursor-pointer"
    >
      <CoverArt gameId={game.id} name={game.name} />
      <div className="absolute inset-0 bg-gradient-to-t from-bg-base/80 via-transparent to-transparent" />
      <div className="absolute top-2 left-2">
        <span className="px-1.5 py-0.5 rounded bg-surface-1/90 backdrop-blur-md font-code-sm text-code-sm text-text-secondary">
          {formatRegion(game.regions[0])}
        </span>
      </div>
      <div className="absolute top-2 right-2">
        {inst ? (
          <GameStatePill
            status={inst.status}
            updateAvailable={inst.updateAvailable}
            size="sm"
          />
        ) : (
          <Pill tone="neutral" label="未安装" size="sm" />
        )}
      </div>
      {inst && (
        <div className="absolute bottom-2 right-2">
          <span className="font-code-sm text-code-sm text-text-primary bg-surface-1/80 px-1.5 py-0.5 rounded backdrop-blur">
            {formatDuration(inst.playtime.totalSec)}
          </span>
        </div>
      )}
    </button>

    <button type="button" onClick={onOpen} className="flex flex-col text-left cursor-pointer">
      <span className="font-headline-md text-headline-md text-text-primary truncate group-hover:text-primary transition-colors">
        {game.name}
      </span>
      <span className="font-body-sm text-body-sm text-text-secondary mt-0.5 truncate">
        {inst?.versionUnknown ? '版本未知' : game.enName}
      </span>
    </button>

    {/* 待处理原因（D1：不进二级页就能知道要处理什么） */}
    {inst && inst.attentionReasons.length > 0 && (
      <div className="flex flex-wrap gap-1 mt-2">
        {inst.attentionReasons.map((r) => (
          <span
            key={r}
            className="px-1.5 py-0.5 rounded bg-secondary-container/20 font-code-sm text-code-sm text-secondary"
          >
            {REASON_LABEL[r]}
          </span>
        ))}
      </div>
    )}

    <button
      type="button"
      disabled={launching}
      onClick={onPrimary}
      className={`mt-3 h-9 rounded-lg font-label-sm text-label-sm flex items-center justify-center gap-1.5 transition-all cursor-pointer ${
        !inst
          ? 'bg-surface-2 text-text-secondary hover:text-text-primary'
          : inst.status === 'running'
            ? 'bg-surface-2 text-primary-container'
            : 'bg-primary-container/90 text-bg-base hover:brightness-110'
      }`}
    >
      <span className={`material-symbols-outlined text-[15px] ${launching ? 'animate-spin' : ''}`}>
        {launching ? 'refresh' : !inst ? 'keyboard_arrow_right' : inst.status === 'running' ? 'stop_circle' : 'play_arrow'}
      </span>
      <span>{launching ? '调起中…' : !inst ? '查看' : inst.status === 'running' ? '结束' : '启动'}</span>
    </button>
  </div>
);
