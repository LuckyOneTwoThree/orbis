/**
 * A4 边界 · 结束进程的风险确认
 *
 * PRD：「结束进程」附数据损坏风险提示，非紧急场景引导游戏内退出。
 * 该确认属 UI 责任（契约 §3.2）——Core 只执行 terminateGame，不做二次确认。
 */
import React, { useState } from 'react';
import { useAppStore } from '../../store/useAppStore';
import { useGamesStore } from '../../store/useGamesStore';

export const TerminateConfirmModal: React.FC = () => {
  const installationId = useAppStore((s) => s.terminateConfirmId);
  const cancel = useAppStore((s) => s.cancelTerminate);
  const install = useGamesStore((s) =>
    installationId ? s.byId(installationId) : undefined,
  );
  const game = useGamesStore((s) => (install ? s.catalogOf(install.gameId) : undefined));
  const terminate = useGamesStore((s) => s.terminate);

  const [busy, setBusy] = useState(false);

  if (!installationId) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-margin">
      <button
        type="button"
        aria-label="关闭"
        onClick={cancel}
        className="fixed inset-0 bg-bg-base/80 backdrop-blur-md z-40 cursor-default"
      />

      <div className="relative z-50 w-full max-w-[460px] bg-surface-1 rounded-xl shadow-2xl border border-border-hairline p-space-lg flex flex-col gap-space-md animate-in fade-in zoom-in-95 duration-200">
        <div className="flex items-center gap-space-sm">
          <div className="w-8 h-8 rounded-lg bg-error-container/30 flex items-center justify-center">
            <span className="material-symbols-outlined text-error text-[18px]">warning</span>
          </div>
          <div className="flex flex-col">
            <span className="font-headline-md text-headline-md text-text-primary">
              强制结束 {game?.name ?? '游戏'}？
            </span>
            <span className="font-code-sm text-code-sm text-text-disabled">
              {install?.pid ? `PID ${install.pid}` : ''}
            </span>
          </div>
        </div>

        <p className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
          强制结束进程可能造成尚未保存的游戏进度丢失，个别情况下还可能损坏本地存档或配置。
          如果不是游戏卡死，建议先切回游戏内正常退出。
        </p>

        <div className="flex items-center justify-end gap-space-sm pt-1">
          <button
            type="button"
            onClick={cancel}
            className="h-10 px-4 rounded text-text-secondary font-label-md text-label-md hover:text-text-primary hover:bg-surface-2 transition-colors cursor-pointer"
          >
            取消
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              await terminate(installationId);
              setBusy(false);
              cancel();
            }}
            className={`h-10 px-4 rounded font-label-md text-label-md transition-colors cursor-pointer ${
              busy
                ? 'bg-surface-2 text-text-disabled cursor-wait'
                : 'bg-error text-white hover:brightness-110'
            }`}
          >
            {busy ? '正在结束…' : '确认结束进程'}
          </button>
        </div>
      </div>
    </div>
  );
};
