/**
 * S5 · 配置历史（B5 / A8）
 *
 * 03 §5.7 规格：页头 + 「立即备份」+ 备份列表（时间 / 触发方式 / 高级详情 / 恢复）
 * + 顶部条目带「当前配置」标记 + 页脚说明「恢复前会自动创建新备份，任何恢复操作都可撤销」。
 *
 * 底层信息（文件名 / 哈希）只在「高级详情」一行以弱化等宽字展示（00 §12.2 / 03 U8）。
 */
import React, { useEffect, useState } from 'react';
import { useAppStore } from '../store/useAppStore';
import { useBackupsStore } from '../store/useBackupsStore';
import { useGamesStore } from '../store/useGamesStore';
import { PageHeader } from '../components/layout/PageHeader';
import { Pill } from '../components/common/Pill';
import { formatBackupTrigger, formatBytes, formatDateTime, formatRelative } from '../utils/format';

export const ConfigHistoryView: React.FC = () => {
  const activeGameId = useAppStore((s) => s.activeGameId);
  const install = useGamesStore((s) => (activeGameId ? s.byGameId(activeGameId) : undefined));
  const game = useGamesStore((s) => (activeGameId ? s.catalogOf(activeGameId) : undefined));

  const backups = useBackupsStore((s) => s.backups);
  const storage = useBackupsStore((s) => s.storage);
  const loading = useBackupsStore((s) => s.loading);
  const busy = useBackupsStore((s) => s.busy);
  const load = useBackupsStore((s) => s.load);
  const create = useBackupsStore((s) => s.create);
  const restore = useBackupsStore((s) => s.restore);
  const remove = useBackupsStore((s) => s.remove);

  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  useEffect(() => {
    if (install) void load(install.id);
  }, [install, load]);

  if (!install || !game) {
    return (
      <main className="w-full pt-14 px-margin pb-margin">
        <PageHeader title="配置历史" subtitle="请先选择一个游戏" />
      </main>
    );
  }

  const supported = install.configUnsupportedReason === null;

  return (
    <main className="w-full pt-14 px-margin pb-margin select-none">
      <div className="flex flex-col w-full max-w-4xl mx-auto py-space-md gap-space-lg">
        <PageHeader
          title="配置历史"
          subtitle={`${game.name} · 备份全部保留在本地，可随时恢复到任意一条`}
          right={
            <button
              type="button"
              disabled={!supported || busy}
              onClick={() => void create(install.id, 'manual')}
              className={`h-10 px-5 rounded-lg font-label-md text-label-md flex items-center gap-2 transition-all cursor-pointer ${
                supported && !busy
                  ? 'bg-primary-container text-bg-base hover:brightness-110 shadow-[0_0_20px_rgba(0,229,255,0.3)]'
                  : 'bg-surface-2 text-text-disabled cursor-not-allowed'
              }`}
            >
              <span className={`material-symbols-outlined text-[18px] ${busy ? 'animate-spin' : ''}`}>
                {busy ? 'sync' : 'add_photo_alternate'}
              </span>
              <span>{busy ? '处理中…' : '立即备份'}</span>
            </button>
          }
        />

        {/* 不支持备份时的显式说明（A8 范围规则） */}
        {!supported && (
          <div className="rounded-xl bg-surface-1 border border-border-hairline p-space-md flex items-start gap-3">
            <span className="material-symbols-outlined text-[18px] text-text-disabled mt-0.5">
              info
            </span>
            <span className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
              {install.configUnsupportedReason === 'not_applicable'
                ? '该游戏的解锁方式不修改任何配置文件，因此没有可备份的内容。'
                : '我们还没有为这款游戏确认安全的配置范围，暂不支持备份与恢复。'}
            </span>
          </div>
        )}

        {/* 列表 */}
        {supported && (
          <div className="flex flex-col gap-3">
            {loading && backups.length === 0 && (
              <div className="py-10 text-center font-body-sm text-body-sm text-text-disabled">
                正在读取备份记录…
              </div>
            )}

            {!loading && backups.length === 0 && (
              <div className="py-14 rounded-xl bg-surface-1 border border-border-hairline flex flex-col items-center gap-2">
                <span className="material-symbols-outlined text-[28px] text-text-disabled">
                  history
                </span>
                <span className="font-headline-md text-headline-md text-text-primary">
                  还没有备份记录
                </span>
                <span className="font-body-sm text-body-sm text-text-secondary">
                  可以随时点右上角「立即备份」，或启用工具时自动创建。
                </span>
              </div>
            )}

            {backups.map((b, idx) => (
              <div
                key={b.id}
                className="group rounded-xl bg-surface-1 border border-border-hairline px-5 py-4 flex flex-col gap-2 hover:border-outline-variant transition-colors"
              >
                <div className="flex items-center justify-between gap-3 flex-wrap">
                  <div className="flex items-center gap-3 min-w-0 flex-wrap">
                    <span className="font-code-md text-code-md text-text-primary">
                      {formatDateTime(b.createdAt)}
                    </span>
                    <Pill
                      tone={b.trigger === 'pre_restore' ? 'accent' : 'neutral'}
                      label={formatBackupTrigger(b.trigger)}
                      size="sm"
                    />
                    <span className="font-body-sm text-body-sm text-text-secondary">
                      {b.fileCount} 个文件 · {formatBytes(b.totalBytes)}
                    </span>
                    {idx === 0 && (
                      <Pill tone="accent" label="当前配置" size="sm" />
                    )}
                  </div>

                  <div className="flex items-center gap-2 shrink-0">
                    <button
                      type="button"
                      disabled={busy || idx === 0}
                      onClick={() => void restore(b.id, install.id)}
                      title={idx === 0 ? '这已经是最新的一份，无需恢复' : undefined}
                      className={`h-9 px-4 rounded-lg font-label-sm text-label-sm transition-colors cursor-pointer ${
                        idx === 0 || busy
                          ? 'bg-surface-2 text-text-disabled cursor-not-allowed'
                          : 'bg-surface-2 text-text-primary hover:bg-surface-3'
                      }`}
                    >
                      恢复
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmDeleteId(b.id)}
                      aria-label="删除这条备份"
                      className="w-9 h-9 rounded-lg flex items-center justify-center text-text-disabled hover:text-error hover:bg-error/10 transition-colors cursor-pointer"
                    >
                      <span className="material-symbols-outlined text-[16px]">delete</span>
                    </button>
                  </div>
                </div>

                {/* 高级详情（弱化次行；底层事实不占默认层的注意力） */}
                {b.primaryFile && (
                  <span className="font-code-sm text-code-sm text-text-disabled">
                    {b.primaryFile} · {b.id} · {formatRelative(b.createdAt)}
                  </span>
                )}

                {/* 删除确认（破坏性操作需二次确认） */}
                {confirmDeleteId === b.id && (
                  <div className="mt-1 rounded-lg border border-error/30 bg-error/5 p-3 flex flex-col gap-2">
                    <span className="font-body-sm text-body-sm text-text-secondary">
                      删除后这条备份无法恢复。确定要删除吗？
                    </span>
                    <div className="flex items-center gap-2 justify-end">
                      <button
                        type="button"
                        onClick={() => setConfirmDeleteId(null)}
                        className="h-8 px-3 rounded text-text-secondary font-label-sm text-label-sm hover:bg-surface-2 transition-colors cursor-pointer"
                      >
                        取消
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setConfirmDeleteId(null);
                          void remove(b.id, install.id);
                        }}
                        className="h-8 px-3 rounded bg-error text-white font-label-sm text-label-sm hover:brightness-110 transition-colors cursor-pointer"
                      >
                        确认删除
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}

            {/* 页脚说明（03 §5.7） */}
            <div className="flex items-center justify-between gap-3 px-1 pt-2">
              <span className="font-body-sm text-body-sm text-text-disabled">
                恢复前会自动创建新备份，任何恢复操作都可撤销。
              </span>
              {storage && (
                <span className="font-code-sm text-code-sm text-text-disabled shrink-0">
                  占用 {formatBytes(storage.totalBytes)} · 剩余 {formatBytes(storage.freeDiskBytes)}
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </main>
  );
};
