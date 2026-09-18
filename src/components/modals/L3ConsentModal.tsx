/**
 * S3 · 工具详情 + L3 风险授权（C2 / C3 / B7）
 *
 * 03 §5.5 规格：工具名/版本/来源/License → 组件下载步骤（发行包分离）→
 * 权限清单（图标 + 说明）→ 兼容状态行 → 风险披露块（**不承诺绝对安全**）→
 * 显式勾选 → 「启用解锁」由禁用变可用 → 次按钮「暂不启用」。无任何黑暗模式。
 *
 * 授权记录文案哈希（04 §7.2）：用户实际看到的文案参与哈希，文案更新后旧同意自动失效。
 */
import React, { useEffect, useState } from 'react';
import { useAppStore } from '../../store/useAppStore';
import { useToolsStore } from '../../store/useToolsStore';
import { CompatPill } from '../common/Pill';
import { formatRiskLabel } from '../../utils/format';
import type { ToolDetail } from '../../api/types';

export const L3ConsentModal: React.FC = () => {
  const toolId = useAppStore((s) => s.l3ModalToolId);
  const close = useAppStore((s) => s.closeL3Modal);
  const getDetail = useToolsStore((s) => s.getDetail);
  const grantAuthorization = useToolsStore((s) => s.grantAuthorization);
  const enable = useToolsStore((s) => s.enable);
  const downloadAsset = useToolsStore((s) => s.downloadAsset);

  const [detail, setDetail] = useState<ToolDetail | null>(null);
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!toolId) {
      setDetail(null);
      setChecked(false);
      return;
    }
    void getDetail(toolId).then(setDetail);
  }, [toolId, getDetail]);

  if (!toolId) return null;

  const asset = detail?.asset ?? null;
  const assetReady = !asset || asset.state === 'ready' || asset.downloadUrlConfigured;
  const canSubmit = checked && assetReady && !busy;

  const handleConfirm = async () => {
    if (!detail || !canSubmit) return;
    setBusy(true);
    const granted = detail.consentTextHash
      ? await grantAuthorization(detail.id, detail.consentTextHash)
      : true;
    if (granted) {
      await enable(detail.id);
    }
    setBusy(false);
    close();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-margin">
      <button
        type="button"
        aria-label="关闭"
        onClick={close}
        className="fixed inset-0 bg-bg-base/80 backdrop-blur-md z-40 cursor-default"
      />

      <div className="relative z-50 w-full max-w-[560px] bg-surface-1 rounded-xl shadow-2xl overflow-hidden flex flex-col border border-border-hairline animate-in fade-in zoom-in-95 duration-200">
        <div className="h-1 w-full bg-surface-2 flex">
          <div className="h-full bg-secondary w-1/3" />
          <div className="h-full bg-primary-container w-2/3" />
        </div>

        {/* 头部 */}
        <div className="p-space-lg pb-space-md flex flex-col gap-space-xs">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-space-sm">
              <div className="w-2 h-2 rounded-full bg-secondary animate-pulse" />
              <span className="font-code-sm text-code-sm text-secondary uppercase tracking-wider">
                {formatRiskLabel(detail?.riskLevel ?? 'L3')} · 运行时权限
              </span>
            </div>
            <span className="font-code-sm text-code-sm text-text-disabled">
              {detail?.version ? `v${detail.version}` : ''}
            </span>
          </div>

          <h2 className="font-headline-xl text-headline-xl text-text-primary tracking-tight mt-1">
            {detail?.name ?? '正在载入…'}
          </h2>

          {detail && (
            <div className="flex items-center gap-2 text-text-secondary font-body-sm text-body-sm flex-wrap">
              <span>
                {detail.source.kind === 'upstream' ? '社区开源' : 'Orbis 内置'}
              </span>
              {detail.source.license && (
                <>
                  <span className="text-text-disabled">·</span>
                  <span>{detail.source.license} License</span>
                </>
              )}
              {detail.source.repo && (
                <>
                  <span className="text-text-disabled">·</span>
                  <a
                    href={`https://github.com/${detail.source.repo}`}
                    target="_blank"
                    rel="noreferrer noopener"
                    className="font-code-sm text-code-sm text-text-disabled hover:text-primary-container transition-colors"
                  >
                    {detail.source.repo}
                  </a>
                </>
              )}
            </div>
          )}
        </div>

        {/* 主体 */}
        <div className="px-space-lg flex flex-col gap-3 max-h-[64vh] overflow-y-auto">
          {/* 组件下载步骤（B7 发行包分离必须显式） */}
          {detail?.asset && (
            <div className="bg-surface-2 rounded-lg p-space-md flex flex-col gap-2">
              <span className="font-label-sm text-label-sm text-text-secondary uppercase tracking-wider">
                解锁组件
              </span>
              {detail.asset.downloadUrlConfigured ? (
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="material-symbols-outlined text-[16px] text-primary-container">
                      download
                    </span>
                    <span className="font-body-sm text-body-sm text-text-secondary">
                      来自 Orbis Releases · 附 SHA-256 校验 · 版本{' '}
                      {detail.asset.version ?? '—'}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={() => void downloadAsset(detail.id)}
                    className="h-8 px-3 rounded bg-surface-3 text-text-primary font-label-sm text-label-sm hover:bg-surface-container-high transition-colors cursor-pointer shrink-0"
                  >
                    下载
                  </button>
                </div>
              ) : (
                <div className="flex items-start gap-2">
                  <span className="material-symbols-outlined text-[16px] text-text-disabled mt-0.5">
                    pending
                  </span>
                  <span className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
                    组件尚未发布，暂时无法启用这个工具。你仍可以正常启动游戏，
                    只是本次不会应用解锁。组件发布后这里会出现下载入口。
                  </span>
                </div>
              )}
            </div>
          )}

          {/* 权限清单 */}
          {detail && detail.permissionsDetail.length > 0 && (
            <div className="bg-surface-2 rounded-lg p-space-md flex flex-col gap-space-sm">
              <span className="font-label-sm text-label-sm text-text-secondary uppercase tracking-wider">
                执行机制与权限范围
              </span>
              {detail.permissionsDetail.map((p) => (
                <div key={p.permission} className="flex items-start gap-3">
                  <div className="mt-0.5 w-6 h-6 rounded bg-surface-3 flex items-center justify-center shrink-0">
                    <span className="material-symbols-outlined text-[15px] text-primary-container">
                      {p.permission === 'process_attach'
                        ? 'memory'
                        : p.permission === 'write_config'
                          ? 'edit'
                          : p.permission === 'read_config'
                            ? 'search'
                            : 'launch'}
                    </span>
                  </div>
                  <div className="flex flex-col">
                    <span className="font-label-md text-label-md text-text-primary">
                      {p.label}
                    </span>
                    <span className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
                      {p.detail}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* 兼容状态 */}
          {detail && (
            <div className="bg-surface-2/60 rounded-lg px-space-md py-3 flex items-center justify-between gap-3">
              <CompatPill status={detail.compat.status} size="sm" />
              {detail.compat.matchedVersionKey && (
                <span className="font-code-sm text-code-sm text-text-disabled shrink-0">
                  命中记录 {detail.compat.matchedVersionKey}
                  {detail.compat.matchKind === 'prefix' ? '（区间兜底）' : ''}
                </span>
              )}
            </div>
          )}

          {/* 风险披露 */}
          <div className="bg-surface-2 rounded-lg p-space-md flex gap-3 items-start border border-error/20">
            <div className="mt-0.5 w-6 h-6 rounded bg-error-container/30 flex items-center justify-center shrink-0">
              <span className="material-symbols-outlined text-error text-[18px]">warning</span>
            </div>
            <div className="flex flex-col gap-1 min-w-0">
              <span className="font-headline-md text-headline-md text-error">
                风险说明
              </span>
              <p className="font-body-sm text-body-sm text-on-surface-variant leading-relaxed whitespace-pre-line">
                {detail?.consentText ??
                  '这个工具会在游戏运行时调整内存中的帧率上限。启用前请确认你已了解可能的影响。'}
              </p>
            </div>
          </div>
        </div>

        {/* 页脚 */}
        <div className="p-space-lg flex flex-col gap-space-md border-t border-border-hairline">
          <label className="flex items-start gap-3 cursor-pointer select-none group">
            <input
              type="checkbox"
              checked={checked}
              disabled={!assetReady}
              onChange={(e) => setChecked(e.target.checked)}
              className="peer sr-only"
            />
            <div
              className={`mt-0.5 w-5 h-5 rounded border flex items-center justify-center transition-colors shrink-0 ${
                !assetReady
                  ? 'bg-surface-2 border-border-hairline opacity-40'
                  : checked
                    ? 'bg-secondary border-secondary'
                    : 'bg-surface-2 border-border-hairline group-hover:border-outline'
              }`}
            >
              {checked && (
                <span className="material-symbols-outlined text-bg-base text-[16px] font-bold">
                  check
                </span>
              )}
            </div>
            <span className="font-body-sm text-body-sm text-text-secondary group-hover:text-text-primary transition-colors">
              我已完整阅读并理解上述说明，自愿启用此功能
            </span>
          </label>

          <div className="flex items-center gap-space-sm justify-end">
            <button
              type="button"
              onClick={close}
              className="h-11 px-space-md rounded text-text-secondary font-label-md text-label-md hover:text-text-primary hover:bg-surface-2 transition-colors cursor-pointer"
            >
              暂不启用
            </button>
            <button
              type="button"
              disabled={!canSubmit}
              onClick={() => void handleConfirm()}
              className={`h-11 px-space-lg rounded font-label-md text-label-md bg-secondary text-on-secondary-fixed transition-all flex items-center gap-2 ${
                canSubmit
                  ? 'cursor-pointer shadow-[0_0_16px_rgba(199,125,255,0.4)] hover:brightness-110 active:scale-[0.98]'
                  : 'opacity-30 cursor-not-allowed'
              }`}
            >
              <span className={`material-symbols-outlined text-[18px] ${busy ? 'animate-spin' : ''}`}>
                {busy ? 'sync' : 'bolt'}
              </span>
              <span>{busy ? '正在启用…' : '启用解锁'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
