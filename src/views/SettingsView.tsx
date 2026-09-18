/**
 * D5 · 设置（P1）
 *
 * 铁律（04 §7.4）：设置页**只允许**出现 §6.1 的三个预置键。
 *
 * 已移除（docs/05 §3 C16 / 新发现）：
 *  - 「游戏启动后最小化至托盘」：不在 04 §6.1 预置键白名单内，属 01 F3 待定范围
 *  - 「严格执行自动容灾快照」**开关**：安全模型不可被关闭 —— 若允许用户关掉它，
 *    就等于允许「无备份修改」，直接违反 00 §12.3 规则 5。故改为只读说明。
 */
import React, { useEffect } from 'react';
import { useGamesStore } from '../store/useGamesStore';
import { useSettingsStore } from '../store/useSettingsStore';
import { PageHeader } from '../components/layout/PageHeader';

export const SettingsView: React.FC = () => {
  const settings = useSettingsStore((s) => s.settings);
  const load = useSettingsStore((s) => s.load);
  const update = useSettingsStore((s) => s.update);
  const runScan = useGamesStore((s) => s.runScan);

  useEffect(() => {
    if (!settings) void load();
  }, [settings, load]);

  return (
    <main className="w-full pt-14 px-margin pb-margin select-none">
      <div className="flex flex-col w-full max-w-4xl mx-auto py-space-md gap-space-lg">
        <PageHeader
          title="设置"
          subtitle="这里只放会影响运行行为的通用选项"
        />

        {/* 版本检测（E1：探测只读、用户可关） */}
        <section className="bg-surface-1 rounded-xl p-space-lg border border-border-hairline flex flex-col gap-space-md">
          <div className="flex items-center gap-space-sm">
            <span className="material-symbols-outlined text-primary-container">cloud_sync</span>
            <h2 className="font-headline-md text-headline-md text-text-primary">
              更新检测
            </h2>
          </div>

          <SettingRow
            title="检查游戏更新"
            description="启动时向官方接口查询一次最新版本号（只读，不下载任何文件）。关闭后仍可手动检查。"
          >
            <Toggle
              checked={settings?.['version_check.enabled'] ?? false}
              disabled={!settings}
              onChange={(v) => void update('version_check.enabled', v)}
            />
          </SettingRow>
        </section>

        {/* 数据与日志 */}
        <section className="bg-surface-1 rounded-xl p-space-lg border border-border-hairline flex flex-col gap-space-md">
          <div className="flex items-center gap-space-sm">
            <span className="material-symbols-outlined text-primary-container">folder_managed</span>
            <h2 className="font-headline-md text-headline-md text-text-primary">
              本地数据
            </h2>
          </div>

          <SettingRow
            title="日志保留天数"
            description="超过天数的日志会在启动时清理。日志只保存在本机，不会上传。"
          >
            <NumberField
              value={settings?.['log.retention_days'] ?? 14}
              min={1}
              max={365}
              suffix="天"
              disabled={!settings}
              onCommit={(v) => void update('log.retention_days', v)}
            />
          </SettingRow>

          <SettingRow
            title="时长记录频率"
            description="游戏运行期间每隔多久写入一次时长。间隔越短越不容易因异常退出丢失记录。"
          >
            <NumberField
              value={settings?.['playtime.checkpoint_sec'] ?? 30}
              min={10}
              max={300}
              step={10}
              suffix="秒"
              disabled={!settings}
              onCommit={(v) => void update('playtime.checkpoint_sec', v)}
            />
          </SettingRow>

          <SettingRow
            title="游戏库扫描"
            description="重新读取官方启动器记录与常见安装路径。整个扫描过程只读，不会改动任何文件。"
          >
            <button
              type="button"
              onClick={() => void runScan()}
              className="h-9 px-4 rounded-lg bg-surface-2 text-text-primary font-label-sm text-label-sm hover:bg-surface-3 transition-colors cursor-pointer"
            >
              立即扫描
            </button>
          </SettingRow>
        </section>

        {/* 安全模型：只读说明，不提供开关 */}
        <section className="bg-surface-1 rounded-xl p-space-lg border border-border-hairline flex flex-col gap-space-md">
          <div className="flex items-center gap-space-sm">
            <span className="material-symbols-outlined text-secondary">shield</span>
            <h2 className="font-headline-md text-headline-md text-text-primary">
              安全机制
            </h2>
            <span className="font-code-sm text-code-sm text-text-disabled uppercase">
              始终开启
            </span>
          </div>

          <p className="font-body-sm text-body-sm text-text-secondary leading-relaxed">
            下面这些机制是工具的信任基础，
            <strong className="text-text-primary font-medium">不提供关闭选项</strong>
            。关闭其中任何一项，都意味着可能出现「配置被改了但没有备份」的情况。
          </p>

          <div className="flex flex-col gap-2">
            {[
              ['修改前强制备份', '没有备份就不会执行任何修改'],
              ['修改后验证', '写入后读回比对，不一致即视为失败'],
              ['失败自动回滚', '验证失败时还原到修改前的状态'],
              ['高风险工具显式授权', 'L3 工具默认关闭，启用前必须阅读并确认风险说明'],
              ['版本兼容门控', '游戏更新后未验证的工具会被阻止启用，而不是静默生效'],
            ].map(([title, detail]) => (
              <div
                key={title}
                className="flex items-start gap-3 px-3 py-2.5 rounded-lg bg-surface-2 border border-border-hairline"
              >
                <span className="material-symbols-outlined text-[16px] text-primary-container mt-0.5">
                  check
                </span>
                <div className="flex flex-col">
                  <span className="font-label-sm text-label-sm text-text-primary">
                    {title}
                  </span>
                  <span className="font-body-sm text-body-sm text-text-disabled">
                    {detail}
                  </span>
                </div>
              </div>
            ))}
          </div>

          <p className="font-body-sm text-body-sm text-text-disabled leading-relaxed">
            唯一的例外是 L1 工具在「当前版本未验证」时，你可以
            <strong className="text-text-secondary font-medium">逐次</strong>
            选择继续使用；
            这不是关掉机制——它仍然会完整走一遍备份、修改与验证。
          </p>
        </section>
      </div>
    </main>
  );
};

// ── 基础控件 ────────────────────────────────────────────────

const SettingRow: React.FC<{
  title: string;
  description: string;
  children: React.ReactNode;
}> = ({ title, description, children }) => (
  <div className="flex items-center justify-between gap-4 p-3 rounded-lg bg-surface-2 border border-border-hairline">
    <div className="flex flex-col min-w-0">
      <span className="font-label-md text-label-md text-text-primary">{title}</span>
      <span className="font-body-sm text-body-sm text-text-secondary mt-0.5 leading-relaxed">
        {description}
      </span>
    </div>
    <div className="shrink-0">{children}</div>
  </div>
);

const Toggle: React.FC<{
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}> = ({ checked, disabled, onChange }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    disabled={disabled}
    onClick={() => onChange(!checked)}
    className={`w-10 h-6 rounded-full p-0.5 flex items-center transition-colors cursor-pointer ${
      checked ? 'bg-primary-container/30 justify-end' : 'bg-surface-3 justify-start'
    } ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
  >
    <span
      className={`w-5 h-5 rounded-full shadow-sm transition-colors ${
        checked ? 'bg-primary-container' : 'bg-outline'
      }`}
    />
  </button>
);

const NumberField: React.FC<{
  value: number;
  min: number;
  max: number;
  step?: number;
  suffix?: string;
  disabled?: boolean;
  onCommit: (v: number) => void;
}> = ({ value, min, max, step = 1, suffix, disabled, onCommit }) => {
  const [draft, setDraft] = React.useState(String(value));

  useEffect(() => setDraft(String(value)), [value]);

  const commit = () => {
    const n = Number(draft);
    if (!Number.isFinite(n) || n < min || n > max) {
      setDraft(String(value));
      return;
    }
    if (n !== value) onCommit(n);
  };

  return (
    <div className="flex items-center gap-2">
      <input
        type="number"
        value={draft}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit();
          if (e.key === 'Escape') setDraft(String(value));
        }}
        className="w-20 h-9 rounded-lg bg-surface-3 border border-border-hairline px-2 text-right font-code-md text-code-md text-text-primary focus:outline-none focus:border-primary-container/50 disabled:opacity-50"
      />
      {suffix && <span className="font-label-sm text-label-sm text-text-secondary">{suffix}</span>}
    </div>
  );
};
