/**
 * S2a · Game Dashboard · 鸣潮（D2 / B1 / B5 / B6）
 *
 * 03 §5.2 规格：返回 + 英雄区（封面/名称/区服/版本/时长）+ 主 CTA +
 * Enhancement 卡片（L1） + 配置历史入口卡。
 *
 * 已移除（docs/05 §3）：IPC 遥测、差分存储占用、「镜像签名/隔离副本」措辞、
 * 「通过 ACE/TP 反作弊纯净合规校验」——后者字面构成安全担保，违反 00 §8.11。
 * 底层事实（文件名 / 键名 / 哈希）一律收进「高级详情」，默认层只呈现动作与结果（03 U8）。
 */
import React, { useEffect, useState } from 'react';
import { useAppStore } from '../store/useAppStore';
import { useBackupsStore } from '../store/useBackupsStore';
import { useGamesStore } from '../store/useGamesStore';
import { useToolsStore } from '../store/useToolsStore';
import { CoverArt } from '../components/common/CoverArt';
import { PageHeader } from '../components/layout/PageHeader';
import { CompatPill, GameStatePill, Pill } from '../components/common/Pill';
import {
  formatBackupTrigger,
  formatBytes,
  formatDateTime,
  formatDurationLong,
  formatRegion,
  formatRiskLabel,
} from '../utils/format';

const WUWA_TOOL_ID = 'orbis-builtin/wuwa-fps-120';

export const WuwaDashboardView: React.FC = () => {
  const requestTerminate = useAppStore((s) => s.requestTerminate);
  const openExecutionPanel = useAppStore((s) => s.openExecutionPanel);
  const openConfigHistory = useAppStore((s) => s.openConfigHistory);
  const install = useGamesStore((s) => s.byGameId('wuthering-waves'));
  const catalogOf = useGamesStore((s) => s.catalogOf);
  const launchingId = useGamesStore((s) => s.launchingId);
  const launch = useGamesStore((s) => s.launch);

  const tool = useToolsStore((s) => s.tools.find((t) => t.id === WUWA_TOOL_ID));
  const applying = useToolsStore((s) => s.applyingToolId === WUWA_TOOL_ID);
  const enable = useToolsStore((s) => s.enable);
  const disable = useToolsStore((s) => s.disable);
  const loadTools = useToolsStore((s) => s.load);

  const backups = useBackupsStore((s) => s.backups);
  const loadBackups = useBackupsStore((s) => s.load);

  const [overrideOpen, setOverrideOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  useEffect(() => {
    void loadTools();
    if (install) void loadBackups(install.id);
  }, [loadTools, loadBackups, install]);

  const game = catalogOf('wuthering-waves');
  const running = install?.status === 'running';
  const isLaunching = Boolean(install && launchingId === install.id);
  const latest = backups[0] ?? null;

  if (!install || !game) {
    return (
      <main className="w-full pt-14 px-margin pb-margin">
        <PageHeader title="鸣潮" subtitle="本机未检出这款游戏" />
      </main>
    );
  }

  const needsOverride = tool?.compat.status === 'unknown' && tool.riskLevel === 'L1';

  const handleEnable = () => {
    if (!tool) return;
    if (needsOverride) {
      setOverrideOpen(true);
      return;
    }
    void enable(tool.id);
  };

  return (
    <main className="w-full pt-14 px-margin pb-margin select-none">
      <div className="w-full max-w-7xl mx-auto flex flex-col gap-y-space-xl">
        <PageHeader />

        {/* 英雄区 */}
        <section className="w-full bg-surface-1 rounded-xl p-space-lg relative overflow-hidden border border-border-hairline">
          <div className="absolute -right-24 -top-24 w-96 h-96 bg-primary-container/5 rounded-full blur-3xl pointer-events-none" />
          <div className="relative z-10 flex flex-col lg:flex-row items-stretch gap-space-xl">
            <div className="relative w-full lg:w-80 h-44 lg:h-auto shrink-0 rounded-lg overflow-hidden bg-surface-2">
              <CoverArt gameId={install.gameId} name={game.name} />
            </div>

            <div className="flex-1 flex flex-col justify-between py-1">
              <div className="flex flex-col gap-space-sm">
                <div className="flex flex-wrap items-center gap-space-sm">
                  <h1 className="font-headline-xl text-headline-xl text-text-primary tracking-tight">
                    {game.name}
                  </h1>
                  <span className="px-space-sm py-0.5 rounded bg-surface-2 text-primary font-label-sm text-label-sm border border-border-hairline">
                    {formatRegion(install.region)}
                  </span>
                  <span className="px-space-sm py-0.5 rounded bg-surface-3 text-on-surface-variant font-code-sm text-code-sm">
                    {install.versionNorm ?? '版本未知'}
                  </span>
                  <div className="ml-auto">
                    <GameStatePill
                      status={install.status}
                      updateAvailable={install.updateAvailable}
                    />
                  </div>
                </div>

                <p className="font-body-md text-body-md text-text-secondary">
                  {game.enName} · {game.publisher}
                </p>

                {/* 版本未知必须显式可见（02 A3），且不能靠缓存值遮掩 */}
                {install.versionUnknown && (
                  <span className="inline-flex items-center gap-1.5 font-body-sm text-body-sm text-secondary">
                    <span className="material-symbols-outlined text-[15px]">help</span>
                    无法识别本地版本，工具兼容性将被判定为「未验证」
                  </span>
                )}

                <div className="flex items-center gap-x-space-md py-space-sm text-text-secondary font-label-md text-label-md flex-wrap">
                  {(
                    [
                      ['今日', install.playtime.todaySec],
                      ['本周', install.playtime.weekSec],
                      ['总计', install.playtime.totalSec],
                    ] as const
                  ).map(([label, sec], idx) => (
                    <React.Fragment key={label}>
                      {idx > 0 && <span className="text-text-disabled">/</span>}
                      <div className="flex items-baseline gap-1.5">
                        <span className="text-text-disabled font-code-sm text-code-sm">
                          {label}
                        </span>
                        <span className="text-text-primary font-code-md text-code-md">
                          {formatDurationLong(sec)}
                        </span>
                      </div>
                    </React.Fragment>
                  ))}
                </div>
              </div>

              <div className="pt-space-md">
                <button
                  type="button"
                  disabled={isLaunching}
                  onClick={() => (running ? requestTerminate(install.id) : void launch(install.id))}
                  className={`w-full h-12 rounded flex items-center justify-center gap-space-sm font-headline-md text-headline-md transition-all cursor-pointer active:scale-[0.99] ${
                    running
                      ? 'bg-surface-2 text-primary-container border border-primary-container/40'
                      : 'bg-primary-container text-bg-base hover:bg-primary-fixed-dim shadow-[0_0_24px_rgba(0,229,255,0.35)]'
                  }`}
                >
                  <span
                    className={`material-symbols-outlined text-[20px] ${isLaunching ? 'animate-spin' : ''}`}
                  >
                    {isLaunching ? 'hourglass_empty' : running ? 'stop_circle' : 'play_arrow'}
                  </span>
                  <span className="tracking-wide">
                    {isLaunching ? '正在调起…' : running ? '结束进程' : '启动游戏'}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </section>

        {/* Enhancement + 配置历史 */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-gutter">
          {/* L1 Enhancement */}
          <div className="relative bg-surface-1 rounded-xl p-space-lg flex flex-col justify-between overflow-hidden border border-border-hairline">
            <div className="absolute left-0 top-0 bottom-0 w-[3px] bg-secondary" />
            <div className="flex flex-col gap-space-md">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-space-sm min-w-0">
                  <span className="material-symbols-outlined text-secondary text-[22px]">
                    speed
                  </span>
                  <h2 className="font-headline-lg text-headline-lg text-text-primary truncate">
                    {tool?.name ?? '120 FPS 解锁'}
                  </h2>
                </div>
                <Pill
                  tone={tool?.enabled ? 'warn' : 'neutral'}
                  label={tool?.enabled ? '已启用' : '未启用'}
                  size="sm"
                />
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <Pill tone="warn" label={formatRiskLabel(tool?.riskLevel ?? 'L1')} size="sm" />
                {tool && <CompatPill status={tool.compat.status} withAdvice size="sm" />}
              </div>

              <p className="font-body-md text-body-md text-text-secondary">
                {tool?.description ?? '把游戏帧率上限从 60 提升到 120，适配高刷新率显示器。'}
              </p>

              {tool && tool.pendingVerifications.length > 0 && (
                <span className="font-body-sm text-body-sm text-text-disabled">
                  部分参数待实测确认：{tool.pendingVerifications.join('、')}
                </span>
              )}
            </div>

            <div className="mt-space-lg pt-space-md flex items-center justify-between gap-2 border-t border-border-hairline/60">
              <button
                type="button"
                onClick={() => setAdvancedOpen((v) => !v)}
                className="inline-flex items-center gap-1 font-code-sm text-code-sm text-text-disabled hover:text-text-secondary transition-colors cursor-pointer"
              >
                <span className="material-symbols-outlined text-[14px]">
                  {advancedOpen ? 'expand_less' : 'expand_more'}
                </span>
                高级详情
              </button>

              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => openExecutionPanel(WUWA_TOOL_ID)}
                  className="h-10 px-space-md rounded bg-surface-2 hover:bg-surface-3 text-text-secondary hover:text-text-primary font-label-md text-label-md transition-all cursor-pointer flex items-center gap-space-xs"
                >
                  <span className="material-symbols-outlined text-[16px]">terminal</span>
                  <span>透明流</span>
                </button>

                <button
                  type="button"
                  disabled={applying}
                  onClick={() => (tool?.enabled ? void disable(WUWA_TOOL_ID) : handleEnable())}
                  className={`h-10 px-space-lg rounded font-label-md text-label-md transition-all cursor-pointer flex items-center gap-space-xs ${
                    tool?.enabled
                      ? 'bg-surface-2 text-secondary'
                      : 'bg-secondary hover:bg-secondary-fixed active:scale-[0.98] text-bg-base shadow-[0_0_16px_rgba(199,125,255,0.3)]'
                  } ${applying ? 'opacity-60 cursor-wait' : ''}`}
                >
                  <span className="material-symbols-outlined text-[18px]">
                    {applying ? 'sync' : tool?.enabled ? 'check' : 'bolt'}
                  </span>
                  <span>{applying ? '执行中…' : tool?.enabled ? '已启用' : '启用'}</span>
                </button>
              </div>
            </div>

            {/* 高级详情（00 §12.2：普通用户简单，高级用户透明） */}
            {advancedOpen && (
              <div className="mt-space-md rounded-lg bg-surface-2 p-space-sm flex flex-col gap-1">
                <span className="font-code-sm text-code-sm text-text-disabled">
                  修改目标
                </span>
                <span className="font-code-sm text-code-sm text-text-secondary">
                  游戏配置数据库 · CustomFrameRate / GameQualitySetting.KeyCustomFrameRate
                </span>
                <span className="font-code-sm text-code-sm text-text-disabled mt-1">
                  备份方式
                </span>
                <span className="font-code-sm text-code-sm text-text-secondary">
                  整目录快照 + 逐文件 SHA-256 校验
                </span>
                <span className="font-code-sm text-code-sm text-text-disabled mt-1">
                  使用提醒
                </span>
                <span className="font-code-sm text-code-sm text-text-secondary">
                  启用后在游戏内修改画质会把帧率锁回 60，需要重新启用本工具
                </span>
              </div>
            )}

            {/* L1 Unknown 覆盖入口（04 §5.5：仅 L1，逐次、显式、留痕） */}
            {overrideOpen && (
              <div className="mt-space-md rounded-lg border border-secondary/30 bg-secondary-container/10 p-space-sm flex flex-col gap-2">
                <span className="font-label-md text-label-md text-secondary">
                  当前版本尚未验证
                </span>
                <span className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
                  这个工具还没有在你当前的游戏版本上验证过。继续使用会走完全相同的流程——
                  仍然先备份、再修改、再校验，失败会自动回滚。本次操作会被记录在日志中。
                </span>
                <div className="flex items-center gap-2 justify-end">
                  <button
                    type="button"
                    onClick={() => setOverrideOpen(false)}
                    className="h-9 px-3 rounded text-text-secondary font-label-sm text-label-sm hover:bg-surface-2 transition-colors cursor-pointer"
                  >
                    暂不启用
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setOverrideOpen(false);
                      if (tool) void enable(tool.id, { overrideCompat: true });
                    }}
                    className="h-9 px-4 rounded bg-secondary text-bg-base font-label-sm text-label-sm hover:brightness-110 transition-colors cursor-pointer"
                  >
                    本次继续使用
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* 配置历史入口 */}
          <div className="bg-surface-1 rounded-xl p-space-lg flex flex-col justify-between border border-border-hairline">
            <div className="flex flex-col gap-space-md">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-space-sm">
                  <span className="material-symbols-outlined text-primary text-[22px]">
                    history
                  </span>
                  <h2 className="font-headline-lg text-headline-lg text-text-primary">
                    配置历史与备份
                  </h2>
                </div>
                {latest && (
                  <span className="font-code-sm text-code-sm text-primary">
                    {backups.length} 条记录
                  </span>
                )}
              </div>

              <p className="font-body-md text-body-md text-text-secondary">
                每次改动配置前都会自动创建备份。备份是游戏配置目录的完整快照，
                任何一次改动都可以安全撤回。
              </p>

              {latest ? (
                <div className="bg-surface-2 rounded-lg p-space-md flex flex-col gap-2 border border-border-hairline">
                  <div className="flex items-center justify-between">
                    <span className="font-label-sm text-label-sm text-text-disabled uppercase tracking-wider">
                      最近备份
                    </span>
                    <span className="font-code-sm text-code-sm text-secondary">
                      {formatBackupTrigger(latest.trigger)}
                    </span>
                  </div>
                  <div className="flex items-baseline justify-between">
                    <span className="font-code-md text-code-md text-text-primary">
                      {formatDateTime(latest.createdAt)}
                    </span>
                    <span className="font-code-sm text-code-sm text-text-secondary">
                      {latest.fileCount} 个文件 · {formatBytes(latest.totalBytes)}
                    </span>
                  </div>
                </div>
              ) : (
                <div className="bg-surface-2 rounded-lg p-space-md border border-border-hairline">
                  <span className="font-body-sm text-body-sm text-text-secondary">
                    还没有备份记录。启用工具时会自动创建第一条。
                  </span>
                </div>
              )}
            </div>

            <div className="mt-space-lg pt-space-md flex items-center justify-between border-t border-border-hairline/60">
              <span className="font-code-sm text-code-sm text-text-disabled">
                备份全部保留，可在历史页手动清理
              </span>
              <button
                type="button"
                onClick={() => openConfigHistory('wuthering-waves')}
                className="inline-flex items-center gap-space-xs text-primary-container hover:text-primary font-label-md text-label-md transition-colors group cursor-pointer"
              >
                <span>查看全部历史</span>
                <span className="material-symbols-outlined text-[18px] group-hover:translate-x-1 transition-transform">
                  arrow_forward
                </span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
};
