/**
 * S2c · Game Dashboard · 通用型（D2 / A4–A7）
 *
 * 03 §5.4 规格：星铁 / 绝区零 / 终末地等无 Enhancement 的游戏使用。
 * 英雄区与主 CTA 同 S2a；「启动参数」卡片（A7，可编辑）；
 * 「配置备份」卡片为禁用态「该游戏暂不支持配置备份」（A8 范围规则）；无 Enhancement 卡片。
 *
 * A8 的禁用态**必须可见**（01 A8 验收要点）——不能像原 frontend 那样直接把卡片藏起来。
 */
import React, { useEffect, useState } from 'react';
import { useAppStore } from '../store/useAppStore';
import { useGamesStore } from '../store/useGamesStore';
import { CoverArt } from '../components/common/CoverArt';
import { PageHeader } from '../components/layout/PageHeader';
import { GameStatePill, Pill } from '../components/common/Pill';
import { formatDurationLong, formatRegion } from '../utils/format';

export const GenericDashboardView: React.FC = () => {
  const activeGameId = useAppStore((s) => s.activeGameId);
  const requestTerminate = useAppStore((s) => s.requestTerminate);
  const install = useGamesStore((s) => (activeGameId ? s.byGameId(activeGameId) : undefined));
  const game = useGamesStore((s) => (activeGameId ? s.catalogOf(activeGameId) : undefined));
  const launchingId = useGamesStore((s) => s.launchingId);
  const launch = useGamesStore((s) => s.launch);
  const launchArgs = useGamesStore((s) =>
    install ? (s.launchArgs[install.id] ?? '') : '',
  );
  const loadLaunchProfile = useGamesStore((s) => s.loadLaunchProfile);
  const saveLaunchArgs = useGamesStore((s) => s.saveLaunchArgs);
  const resetLaunchArgs = useGamesStore((s) => s.resetLaunchArgs);

  const [argsDraft, setArgsDraft] = useState('');
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (install) void loadLaunchProfile(install.id);
  }, [install, loadLaunchProfile]);

  useEffect(() => {
    setArgsDraft(launchArgs);
  }, [launchArgs]);

  if (!install || !game) {
    return (
      <main className="w-full pt-14 px-margin pb-margin">
        <PageHeader title="未找到游戏" subtitle="请从首页重新选择" />
      </main>
    );
  }

  const running = install.status === 'running';
  const isLaunching = launchingId === install.id;
  const backupUnsupported = install.configUnsupportedReason === 'provider_not_declared';

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

            <div className="flex flex-col gap-1.5 min-w-0">
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

              <span className="font-code-sm text-code-sm text-text-disabled truncate">
                {install.executablePath}
              </span>
            </div>
          </div>

          <div className="flex items-center w-full md:w-auto justify-end z-10 shrink-0">
            <button
              type="button"
              disabled={isLaunching}
              onClick={() =>
                running ? requestTerminate(install.id) : void launch(install.id)
              }
              className={`w-full md:w-44 h-12 rounded-lg font-headline-md text-headline-md flex items-center justify-center gap-space-sm transition-all active:scale-[0.98] cursor-pointer ${
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

        <div className="grid grid-cols-1 md:grid-cols-2 gap-gutter">
          {/* 启动参数（A7） */}
          <div className="bg-surface-1 rounded-xl p-space-lg flex flex-col justify-between border border-border-hairline">
            <div className="flex flex-col gap-space-md">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-space-sm">
                  <span className="material-symbols-outlined text-primary text-[22px]">
                    terminal
                  </span>
                  <h2 className="font-headline-lg text-headline-lg text-text-primary">
                    启动参数
                  </h2>
                </div>
                <span className="font-code-sm text-code-sm text-text-disabled">
                  {launchArgs ? '已配置' : '未配置'}
                </span>
              </div>

              <p className="font-body-md text-body-md text-text-secondary">
                这些参数会在每次从 Orbis 启动该游戏时自动附加。
              </p>

              {editing ? (
                <input
                  autoFocus
                  value={argsDraft}
                  onChange={(e) => setArgsDraft(e.target.value)}
                  placeholder="例如 -dx12 -windowed"
                  className="w-full h-11 rounded-lg bg-surface-2 border border-border-hairline px-3 font-code-sm text-code-sm text-text-primary placeholder:text-text-disabled focus:outline-none focus:border-primary-container/50"
                />
              ) : (
                <div className="bg-surface-2 rounded-lg p-space-md border border-border-hairline">
                  <span className="font-code-md text-code-md text-text-primary break-all">
                    {launchArgs || '（空）'}
                  </span>
                </div>
              )}
            </div>

            <div className="mt-space-lg pt-space-md flex items-center justify-between border-t border-border-hairline/60">
              <span className="font-code-sm text-code-sm text-text-disabled">
                参数冲突检测将在后续版本提供
              </span>
              <div className="flex items-center gap-2">
                {editing ? (
                  <>
                    <button
                      type="button"
                      onClick={() => {
                        setArgsDraft(launchArgs);
                        setEditing(false);
                      }}
                      className="h-9 px-3 rounded text-text-secondary font-label-sm text-label-sm hover:bg-surface-2 transition-colors cursor-pointer"
                    >
                      取消
                    </button>
                    <button
                      type="button"
                      disabled={saving}
                      onClick={async () => {
                        setSaving(true);
                        const ok = await saveLaunchArgs(install.id, argsDraft);
                        setSaving(false);
                        if (ok) setEditing(false);
                      }}
                      className="h-9 px-4 rounded bg-primary-container text-bg-base font-label-sm text-label-sm hover:brightness-110 transition-colors cursor-pointer disabled:opacity-60"
                    >
                      {saving ? '保存中…' : '保存'}
                    </button>
                  </>
                ) : (
                  <>
                    {launchArgs && (
                      <button
                        type="button"
                        onClick={() => void resetLaunchArgs(install.id)}
                        className="h-9 px-3 rounded text-text-secondary font-label-sm text-label-sm hover:bg-surface-2 transition-colors cursor-pointer"
                      >
                        清除
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => setEditing(true)}
                      className="h-9 px-4 rounded bg-surface-2 text-text-primary font-label-sm text-label-sm hover:bg-surface-3 transition-colors cursor-pointer"
                    >
                      编辑
                    </button>
                  </>
                )}
              </div>
            </div>
          </div>

          {/* 配置备份（A8 范围规则） */}
          <div
            className={`rounded-xl p-space-lg flex flex-col justify-between border ${
              backupUnsupported
                ? 'bg-surface-1/60 border-border-hairline'
                : 'bg-surface-1 border-border-hairline'
            }`}
          >
            <div className="flex flex-col gap-space-md">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-space-sm">
                  <span className="material-symbols-outlined text-[22px] text-text-disabled">
                    settings_backup_restore
                  </span>
                  <h2 className="font-headline-lg text-headline-lg text-text-disabled">
                    配置备份
                  </h2>
                </div>
                <Pill tone="neutral" label="暂不支持" size="sm" />
              </div>

              <p className="font-body-md text-body-md text-text-disabled leading-relaxed">
                {backupUnsupported
                  ? '我们还没有为这款游戏确认安全的配置范围。确认之后会在这里开放备份与恢复。'
                  : '该游戏无需配置备份。'}
              </p>

              <div className="rounded-lg bg-surface-2/60 p-space-md border border-border-hairline flex items-start gap-2">
                <span className="material-symbols-outlined text-[16px] text-text-disabled mt-0.5">
                  info
                </span>
                <span className="font-body-sm text-body-sm text-text-disabled leading-relaxed">
                  这是刻意为之：不会对未确认安全的文件做读写。游戏文件与存档在任何情况下都不会被改动。
                </span>
              </div>
            </div>

            <div className="mt-space-lg pt-space-md border-t border-border-hairline/60">
              <button
                type="button"
                disabled
                className="w-full h-9 rounded-lg bg-surface-2 text-text-disabled font-label-sm text-label-sm cursor-not-allowed"
              >
                备份配置
              </button>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
};
