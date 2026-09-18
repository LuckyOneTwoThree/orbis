/**
 * S6 · 首次运行引导（J1 / A1 / A2）
 *
 * 03 §5.8 规格：居中构图 → 正在扫描 → 5 个游戏槽位（已检出 / 未安装）→ 进入 Orbis。
 * 另按 02 A1 边界实现空状态（无任何已安装游戏时给官方入口引导，而非空白页）。
 *
 * 数据全部来自 Core：游戏目录 `listGames()`、扫描 `scanGames()`、校验 `validateExecutable()`、
 * 添加 `addInstallation()`。本视图不含任何游戏名单硬编码。
 */
import React, { useEffect, useRef, useState } from 'react';
import { useAppStore } from '../store/useAppStore';
import { useGamesStore } from '../store/useGamesStore';
import { OrbisLogo } from '../components/common/OrbisLogo';
import { CoverArt } from '../components/common/CoverArt';
import { formatRegion } from '../utils/format';
import type { GameId } from '../api/types';

export const OnboardingView: React.FC = () => {
  const setActiveView = useAppStore((s) => s.setActiveView);
  const catalog = useGamesStore((s) => s.catalog);
  const installations = useGamesStore((s) => s.installations);
  const scanProgress = useGamesStore((s) => s.scanProgress);
  const lastScan = useGamesStore((s) => s.lastScan);
  const runScan = useGamesStore((s) => s.runScan);

  const [showManual, setShowManual] = useState(false);
  const scannedRef = useRef(false);

  useEffect(() => {
    if (scannedRef.current) return;
    scannedRef.current = true;
    void runScan();
  }, [runScan]);

  const isScanning = scanProgress !== null;
  const foundIds = new Set(installations.map((i) => i.gameId));
  const foundCount = catalog.filter((g) => foundIds.has(g.id)).length;
  const percent =
    scanProgress && scanProgress.total > 0
      ? Math.min(100, Math.round((scanProgress.scanned / scanProgress.total) * 100))
      : isScanning
        ? 8
        : 100;

  return (
    <div className="min-h-screen bg-bg-base flex flex-col items-center justify-center p-gutter md:p-margin relative overflow-hidden select-none">
      <div className="absolute -top-12 w-64 h-64 bg-primary-container/10 rounded-full blur-3xl pointer-events-none -z-10" />

      <div className="relative w-full max-w-[640px] flex flex-col items-center">
        <div className="flex flex-col items-center text-center mb-space-lg w-full">
          <div className="mb-space-md">
            <OrbisLogo size="lg" showText={false} />
          </div>

          <h1 className="font-headline-xl text-headline-xl text-text-primary tracking-tight mb-1">
            {isScanning ? '正在扫描本机游戏' : foundCount > 0 ? '扫描完成' : '未找到已安装的游戏'}
          </h1>
          <p className="font-body-md text-body-md text-text-secondary">
            {isScanning
              ? '正在读取官方启动器记录与常见安装路径…'
              : foundCount > 0
                ? `已检出 ${foundCount} 款游戏`
                : '确认游戏已通过官方渠道安装，或手动添加程序位置'}
          </p>
        </div>

        {/* 扫描进度（由 scan:progress 事件驱动） */}
        {isScanning && (
          <div className="w-full mb-space-md">
            <div className="flex items-center justify-between font-code-sm text-code-sm mb-1.5 px-0.5">
              <span className="text-text-secondary flex items-center gap-1.5">
                <span className="material-symbols-outlined text-[14px] text-primary-container animate-spin">
                  sync
                </span>
                {scanProgress.phase === 'registry'
                  ? '读取启动器记录'
                  : scanProgress.phase === 'paths'
                    ? '检查安装路径'
                    : '校验游戏程序'}
              </span>
              <span className="text-primary-container font-medium tracking-wide">
                {percent}%
              </span>
            </div>
            <div className="w-full h-1.5 bg-surface-3 rounded-full overflow-hidden">
              <div
                className="h-full bg-primary-container rounded-full transition-all duration-500"
                style={{ width: `${percent}%` }}
              />
            </div>
          </div>
        )}

        {/* 5 个游戏槽位 */}
        <div className="w-full bg-surface-1 rounded-xl p-space-sm mb-space-lg shadow-xl shadow-black/60 flex flex-col gap-1.5 border border-border-hairline">
          {catalog.map((g) => {
            const found = foundIds.has(g.id);
            const inst = installations.find((i) => i.gameId === g.id);
            return (
              <div
                key={g.id}
                className={`flex items-center justify-between p-3 rounded-lg transition-colors ${
                  found ? 'bg-surface-2' : 'bg-surface-container-lowest opacity-50'
                }`}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <div className="w-10 h-10 rounded-md overflow-hidden shrink-0 bg-surface-3">
                    <CoverArt gameId={g.id} name={g.name} showGlyph />
                  </div>
                  <div className="flex flex-col min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className={`font-headline-md text-headline-md truncate ${
                          found ? 'text-text-primary' : 'text-text-disabled'
                        }`}
                      >
                        {g.name}
                      </span>
                      <span className="font-code-sm text-code-sm px-1.5 py-0.5 rounded bg-surface-container text-text-secondary">
                        {formatRegion(g.regions[0])}
                      </span>
                    </div>
                    <span className="font-code-sm text-code-sm text-text-secondary truncate mt-0.5">
                      {found && inst ? inst.executablePath : '未检出可执行程序'}
                    </span>
                  </div>
                </div>

                {found ? (
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-surface-container-high shrink-0 ml-3">
                    <span className="material-symbols-outlined text-primary-container text-[16px]">
                      check_circle
                    </span>
                    <span className="font-label-sm text-label-sm text-primary-container">
                      已检出
                    </span>
                  </div>
                ) : (
                  <span className="font-label-sm text-label-sm text-text-disabled shrink-0 ml-3">
                    未安装
                  </span>
                )}
              </div>
            );
          })}
        </div>

        {/* 空状态引导（02 A1 边界：给官方入口，而不是空白页） */}
        {!isScanning && foundCount === 0 && (
          <div className="w-full bg-surface-1 rounded-xl p-space-md mb-space-lg border border-border-hairline">
            <div className="flex items-center gap-2 mb-3">
              <span className="material-symbols-outlined text-[18px] text-text-secondary">
                download
              </span>
              <span className="font-label-md text-label-md text-text-primary">
                从官方渠道安装
              </span>
            </div>
            <div className="flex flex-col gap-1.5">
              {catalog
                .filter((g) => g.officialUrl)
                .map((g) => (
                  <a
                    key={g.id}
                    href={g.officialUrl ?? '#'}
                    target="_blank"
                    rel="noreferrer noopener"
                    className="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-surface-2 transition-colors group"
                  >
                    <span className="font-body-sm text-body-sm text-text-secondary group-hover:text-text-primary">
                      {g.name} · {g.publisher}
                    </span>
                    <span className="material-symbols-outlined text-[16px] text-text-disabled group-hover:text-primary-container">
                      open_in_new
                    </span>
                  </a>
                ))}
            </div>
            <p className="font-body-sm text-body-sm text-text-disabled mt-3 leading-relaxed">
              安装完成后回到这里重新扫描即可。Orbis 不会自行下载游戏。
            </p>
          </div>
        )}

        {/* 手动添加（A2） */}
        {showManual ? (
          <ManualAddForm onDone={() => setShowManual(false)} />
        ) : (
          <div className="w-full flex flex-col items-center gap-space-md">
            <button
              type="button"
              disabled={isScanning}
              onClick={() => setActiveView('home')}
              className={`w-full h-12 rounded-lg font-label-md text-label-md font-semibold tracking-wide flex items-center justify-center gap-2 transition-all cursor-pointer ${
                isScanning
                  ? 'bg-surface-2 text-text-disabled cursor-not-allowed'
                  : 'bg-primary-container text-bg-base shadow-[0_0_24px_rgba(0,229,255,0.35)] hover:brightness-110 active:scale-[0.99]'
              }`}
            >
              <span>进入 Orbis{foundCount > 0 ? ` (已选 ${foundCount} 款)` : ''}</span>
              <span className="material-symbols-outlined text-[18px]">arrow_forward</span>
            </button>

            <div className="flex items-center gap-4">
              <button
                type="button"
                onClick={() => setShowManual(true)}
                className="inline-flex items-center gap-1.5 font-label-sm text-label-sm text-text-secondary hover:text-text-primary transition-colors cursor-pointer py-1"
              >
                <span className="material-symbols-outlined text-[16px]">add_circle</span>
                <span>手动添加游戏路径</span>
              </button>
              <button
                type="button"
                onClick={() => void runScan()}
                className="inline-flex items-center gap-1.5 font-label-sm text-label-sm text-text-secondary hover:text-text-primary transition-colors cursor-pointer py-1"
              >
                <span className="material-symbols-outlined text-[16px]">refresh</span>
                <span>重新扫描</span>
              </button>
            </div>
          </div>
        )}

        {/* 扫描部分失败时的显式提示（02 A1：不阻塞启动，但必须可见） */}
        {lastScan?.partial && lastScan.warnings.length > 0 && (
          <div className="mt-space-md w-full rounded-lg border border-border-hairline bg-surface-1 px-4 py-3">
            <span className="font-label-sm text-label-sm text-text-secondary">
              扫描未完全：{lastScan.warnings.join('；')}
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

/**
 * 手动添加（A2）
 * 先 validateExecutable 再 addInstallation —— 满足 02 A2 边界「不静默接受官方启动器」。
 * TODO(实现期)：接入 Tauri dialog 插件后改为系统文件选择器，当前为路径输入框。
 */
const ManualAddForm: React.FC<{ onDone: () => void }> = ({ onDone }) => {
  const catalog = useGamesStore((s) => s.catalog);
  const validateExecutable = useGamesStore((s) => s.validateExecutable);
  const addInstallation = useGamesStore((s) => s.addInstallation);
  const [gameId, setGameId] = useState<GameId | ''>('');
  const [path, setPath] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!gameId || !path.trim()) {
      setMessage('请选择游戏并填写程序路径');
      return;
    }
    setBusy(true);
    setMessage(null);
    // A2 边界：先校验再添加，不静默接受官方启动器
    const v = await validateExecutable(gameId, path.trim());
    if (!v || !v.ok) {
      setBusy(false);
      setMessage(
        v?.reason === 'looks_like_launcher'
          ? '这是官方启动器，不是游戏本体。请选择游戏自身的可执行文件。'
          : v?.reason === 'game_mismatch'
            ? `这个程序属于另一款游戏（检测到：${v.detectedGameId}）`
            : '无法识别这个文件，请确认选择的是游戏程序。',
      );
      return;
    }
    const ok = await addInstallation({ gameId, executablePath: path.trim() });
    setBusy(false);
    if (ok) onDone();
  };

  return (
    <div className="w-full bg-surface-1 rounded-xl p-space-md border border-border-hairline flex flex-col gap-3">
      <span className="font-label-md text-label-md text-text-primary">手动添加游戏</span>

      <div className="flex flex-col gap-2">
        {catalog.map((g) => (
          <button
            key={g.id}
            type="button"
            onClick={() => setGameId(g.id)}
            className={`text-left px-3 py-2 rounded-lg font-body-sm text-body-sm transition-colors cursor-pointer ${
              gameId === g.id
                ? 'bg-primary-container/10 text-primary-container border border-primary-container/30'
                : 'bg-surface-2 text-text-secondary hover:text-text-primary border border-transparent'
            }`}
          >
            {g.name}
          </button>
        ))}
      </div>

      <input
        value={path}
        onChange={(e) => setPath(e.target.value)}
        placeholder="游戏程序完整路径，例如 D:\GenshinImpact\Genshin Impact Game\YuanShen.exe"
        className="w-full h-11 rounded-lg bg-surface-2 border border-border-hairline px-3 font-code-sm text-code-sm text-text-primary placeholder:text-text-disabled focus:outline-none focus:border-primary-container/50"
      />

      {message && <span className="font-body-sm text-body-sm text-error">{message}</span>}

      <div className="flex items-center gap-2 justify-end">
        <button
          type="button"
          onClick={onDone}
          className="h-10 px-4 rounded text-text-secondary font-label-md text-label-md hover:text-text-primary hover:bg-surface-2 transition-colors cursor-pointer"
        >
          取消
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void submit()}
          className={`h-10 px-5 rounded font-label-md text-label-md transition-all cursor-pointer ${
            busy
              ? 'bg-surface-2 text-text-disabled cursor-not-allowed'
              : 'bg-primary-container text-bg-base hover:brightness-110'
          }`}
        >
          {busy ? '校验中…' : '校验并添加'}
        </button>
      </div>
    </div>
  );
};
