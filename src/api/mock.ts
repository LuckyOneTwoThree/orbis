/**
 * Orbis mock 实现 —— 无 Rust 环境下的**行为对齐参照实现**（契约 §7.2）
 *
 * 设计要点：
 *  1. 种子表与 Manifest 直接 import 仓库内的真实数据文件，
 *     从机制上保证 mock 口径不可能与 data/ 漂移（契约 §7.2）
 *  2. 兼容匹配（exact → prefix → none）与「需处理」判定式在 mock 内**真实实现**，
 *     而非返回硬编码结果 —— 这样 UI 可以验证 B6/D1 的真实表现
 *  3. setToolEnabled 逐步发 tool:apply-step（gate → precheck → backup → modify → verify），
 *     且 detail 为两层结构（default / advanced），用于验证 03 §5.6 U8 的分层展示
 *  4. 不引入任何契约外字段（禁用虚构指标）
 */
import seedRaw from '../../data/compatibility/seed.json';
import assetsRaw from '../../data/tools/assets.json';
import wuwaManifestRaw from '../../data/tools/manifests/wuwa-fps-120.json';
import genshinManifestRaw from '../../data/tools/manifests/genshin-fps-unlock.json';
import type { OrbisApi } from './contract';
import {
  OrbisInvokeError,
  type AppSettings,
  type ApplyStep,
  type BackupSummary,
  type CompatibilityDto,
  type ErrorCode,
  type GameCatalogEntry,
  type GameId,
  type GameRuntimeStatus,
  type InstallationDto,
  type OrbisEventMap,
  type OrbisEventName,
  type Region,
  type RemoteVersionResult,
  type SettingKey,
  type ToolAssetDto,
  type ToolCompatStatus,
  type ToolDto,
  type ToolPermission,
  type ToolType,
  type VersionSourceId,
} from './types';

// ─────────────────────────────────────────────────────────
// 真实数据文件的结构类型（JSON 导入的窄化视图）
// ─────────────────────────────────────────────────────────

interface SeedRecord {
  game_version: string;
  version_match?: 'exact' | 'prefix';
  status: string;
  verified_at?: string;
  verified_by?: string;
  evidence?: string[];
  notes?: string;
}
interface SeedEntry {
  game_id: string;
  tool_id: string;
  risk_level: string;
  compatibility: SeedRecord[];
}
interface SeedTable {
  schema_version: string;
  updated_at: string;
  maintainer?: string;
  entries: SeedEntry[];
}
interface ManifestShape {
  id: string;
  game: string;
  name: string;
  description: string;
  version: string;
  type: string;
  risk_level: string;
  permissions: string[];
  requires_admin: boolean;
  backup_required: boolean;
  entry: { executor: string; asset?: string; channel?: string };
  source: { kind: string; repo: string | null; license: string | null; version: string | null };
  pending_verifications: string[];
}
interface AssetsShape {
  schema_version: string;
  assets: {
    asset_key: string;
    tool_id: string;
    channel: string;
    upstream: { repo: string; license: string; version: string };
    artifacts: { platform: string; version: string; url: string; sha256: string; size: number }[];
  }[];
}

const SEED = seedRaw as unknown as SeedTable;
const ASSETS = assetsRaw as unknown as AssetsShape;
const MANIFESTS = [
  wuwaManifestRaw as unknown as ManifestShape,
  genshinManifestRaw as unknown as ManifestShape,
];

/** 状态映射：seed 用首字母大写，IPC 用小写（契约 §2） */
const SEED_STATUS_MAP: Record<string, ToolCompatStatus> = {
  Verified: 'verified',
  Compatible: 'compatible',
  Unknown: 'unknown',
  Incompatible: 'incompatible',
  Deprecated: 'deprecated',
};

// ─────────────────────────────────────────────────────────
// 兼容引擎（P0-C §2.2：exact → prefix → none）
// ─────────────────────────────────────────────────────────

function queryCompat(
  gameId: GameId,
  toolId: string,
  versionNorm: string | null,
): CompatibilityDto {
  const base = {
    gameId,
    toolId,
    seedSchemaVersion: SEED.schema_version,
  };
  const entry = SEED.entries.find(
    (e) => e.game_id === gameId && e.tool_id === toolId,
  );

  // 无条目 / 版本未知 / seed 缺失 → Unknown（默认安全态，02 C4 验收）
  if (!entry || !versionNorm) {
    return {
      ...base,
      status: 'unknown',
      matchKind: 'none',
      matchedVersionKey: null,
      verifiedAt: null,
      verifiedBy: null,
      notes: entry ? '本地版本未知，无法判定（02 A3）' : '种子表无该工具条目',
    };
  }

  const exact = entry.compatibility.find(
    (r) => (!r.version_match || r.version_match === 'exact') && r.game_version === versionNorm,
  );
  const prefixHit = entry.compatibility.find(
    (r) => r.version_match === 'prefix' && versionNorm.startsWith(r.game_version.replace(/\.x$/, '.')),
  );
  const hit = exact ?? prefixHit;

  if (!hit) {
    return {
      ...base,
      status: 'unknown',
      matchKind: 'none',
      matchedVersionKey: null,
      verifiedAt: null,
      verifiedBy: null,
      notes: `种子表无 ${versionNorm} 条目 → 默认安全态 Unknown`,
    };
  }

  return {
    ...base,
    status: SEED_STATUS_MAP[hit.status] ?? 'unknown',
    matchKind: exact ? 'exact' : 'prefix',
    matchedVersionKey: hit.game_version,
    verifiedAt: hit.verified_at ?? null,
    verifiedBy: hit.verified_by ?? null,
    notes: hit.notes ?? null,
  };
}

// ─────────────────────────────────────────────────────────
// 游戏目录（编译期静态；Rust 侧对应 orbis-providers 的 catalog 条目）
//
// 官方入口 URL 为待核实值（04 §10 新增实测项 T8）：仅用于空状态引导，
// 定稿前若发现偏差，只需改此处一处（UI 不自带目录）。
// ─────────────────────────────────────────────────────────

const GAME_CATALOG: GameCatalogEntry[] = [
  {
    id: 'wuthering-waves',
    name: '鸣潮',
    enName: 'Wuthering Waves',
    publisher: '库洛游戏',
    regions: ['cn', 'global'],
    officialUrl: 'https://mc.kurogames.com/',
    hasBundledComponent: false,
  },
  {
    id: 'genshin-impact',
    name: '原神',
    enName: 'Genshin Impact',
    publisher: '米哈游',
    regions: ['cn', 'global', 'bili'],
    officialUrl: 'https://ys.mihoyo.com/',
    // B7：主安装包不含注入器，首次启用才下载 → 空状态需提前告知
    hasBundledComponent: true,
  },
  {
    id: 'honkai-star-rail',
    name: '崩坏：星穹铁道',
    enName: 'Honkai: Star Rail',
    publisher: '米哈游',
    regions: ['cn', 'global'],
    officialUrl: 'https://sr.mihoyo.com/',
    hasBundledComponent: false,
  },
  {
    id: 'zenless-zone-zero',
    name: '绝区零',
    enName: 'Zenless Zone Zero',
    publisher: '米哈游',
    regions: ['cn', 'global'],
    officialUrl: 'https://zzz.mihoyo.com/',
    hasBundledComponent: false,
  },
  {
    id: 'arknights-endfield',
    name: '明日方舟：终末地',
    enName: 'Arknights: Endfield',
    publisher: '鹰角网络',
    regions: ['cn'],
    officialUrl: 'https://endfield.hypergryph.com/',
    hasBundledComponent: false,
  },
];

// ─────────────────────────────────────────────────────────
// 可变状态
// ─────────────────────────────────────────────────────────

const HOUR = 3600_000;
const DAY = 24 * HOUR;
const now = () => Date.now();

interface InstallationSeed {
  id: string;
  gameId: GameId;
  region: Region;
  installPath: string;
  executablePath: string;
  /** 原始识别值（模拟 exe VERSIONINFO 的多段构建号，04 §5.1 契约） */
  localVersion: string | null;
  versionSource: InstallationDto['versionSource'];
  /** 远端最新版本（归一化）；null = 降级/不可达 */
  remoteVersion: string | null;
  versionSourceId: VersionSourceId;
  hasConfigSource: boolean;
  configUnsupportedReason: InstallationDto['configUnsupportedReason'];
  playtime: { todaySec: number; weekSec: number; totalSec: number };
  pid: number | null;
}

const SEED_INSTALLATIONS: InstallationSeed[] = [
  {
    id: 'inst-wuwa-cn',
    gameId: 'wuthering-waves',
    region: 'cn',
    installPath: 'C:/WutheringWaves',
    executablePath: 'C:/WutheringWaves/Wuthering Waves Game/Client-Win64-Shipping.exe',
    localVersion: '3.5.0.128940',
    versionSource: 'exe_versioninfo',
    remoteVersion: '3.5',
    versionSourceId: 'kuro:index.json',
    hasConfigSource: true,
    configUnsupportedReason: null,
    playtime: { todaySec: 4320, weekSec: 23400, totalSec: 460800 },
    pid: null,
  },
  {
    id: 'inst-genshin-cn',
    gameId: 'genshin-impact',
    region: 'cn',
    installPath: 'D:/GenshinImpact',
    executablePath: 'D:/GenshinImpact/Genshin Impact Game/YuanShen.exe',
    localVersion: '7.1.0.4821',
    versionSource: 'exe_versioninfo',
    remoteVersion: '7.1',
    versionSourceId: 'hyp:getGamePackages',
    hasConfigSource: false,
    configUnsupportedReason: 'not_applicable',
    playtime: { todaySec: 0, weekSec: 15120, totalSec: 1224000 },
    pid: null,
  },
  {
    id: 'inst-starrail-cn',
    gameId: 'honkai-star-rail',
    region: 'cn',
    installPath: 'D:/StarRail',
    executablePath: 'D:/StarRail/Game/StarRail.exe',
    localVersion: '4.4.0.3105',
    versionSource: 'exe_versioninfo',
    remoteVersion: '4.5',
    versionSourceId: 'hyp:getGamePackages',
    hasConfigSource: false,
    configUnsupportedReason: 'provider_not_declared',
    playtime: { todaySec: 0, weekSec: 7560, totalSec: 342000 },
    pid: null,
  },
  {
    id: 'inst-zzz-cn',
    gameId: 'zenless-zone-zero',
    region: 'cn',
    installPath: 'E:/ZenlessZoneZero',
    executablePath: 'E:/ZenlessZoneZero/Game/ZenlessZoneZero.exe',
    localVersion: '2.1.0.7702',
    versionSource: 'exe_versioninfo',
    remoteVersion: '2.1',
    versionSourceId: 'hyp:getGamePackages',
    hasConfigSource: false,
    configUnsupportedReason: 'provider_not_declared',
    playtime: { todaySec: 0, weekSec: 0, totalSec: 151200 },
    pid: null,
  },
  {
    id: 'inst-endfield-cn',
    gameId: 'arknights-endfield',
    region: 'cn',
    installPath: 'F:/Endfield',
    executablePath: 'F:/Endfield/Client/Endfield.exe',
    // 终末地可识别本地版本，但无公开 manifest → E1 降级（P0-B §3.2）
    localVersion: '0.1.4.2210',
    versionSource: 'exe_versioninfo',
    remoteVersion: null,
    versionSourceId: 'degraded',
    hasConfigSource: false,
    configUnsupportedReason: 'provider_not_declared',
    playtime: { todaySec: 0, weekSec: 0, totalSec: 43200 },
    pid: null,
  },
];

const state = {
  installations: SEED_INSTALLATIONS.map((s) => ({ ...s })),
  runtime: new Map<string, GameRuntimeStatus>(
    SEED_INSTALLATIONS.map((s) => [s.id, s.pid ? 'running' : 'installed']),
  ),
  toolEnabled: new Map<string, boolean>([
    ['orbis-builtin/wuwa-fps-120', false],
    ['orbis-bundled/genshin-fps-unlock', false],
  ]),
  toolAuthorized: new Map<string, string | null>([
    ['orbis-builtin/wuwa-fps-120', null],
    ['orbis-bundled/genshin-fps-unlock', null],
  ]),
  launchProfiles: new Map<string, { args: string; updatedAt: number }>(),
  backups: [
    {
      id: 'bk-42',
      installationId: 'inst-wuwa-cn',
      gameId: 'wuthering-waves' as GameId,
      toolId: 'orbis-builtin/wuwa-fps-120',
      trigger: 'pre_modify' as const,
      fileCount: 3,
      totalBytes: 430080,
      primaryFile: 'LocalStorage.db',
      createdAt: now() - 5 * HOUR - 29 * 60_000,
    },
    {
      id: 'bk-41',
      installationId: 'inst-wuwa-cn',
      gameId: 'wuthering-waves' as GameId,
      toolId: null,
      trigger: 'manual' as const,
      fileCount: 3,
      totalBytes: 428032,
      primaryFile: 'LocalStorage.db',
      createdAt: now() - DAY - HOUR,
    },
  ] as BackupSummary[],
  settings: {
    'version_check.enabled': true,
    'log.retention_days': 14,
    'playtime.checkpoint_sec': 30,
  } as AppSettings,
  scanCount: 0,
};

/** dev/test 专用：强制下一次 apply 在 verify 阶段失败，用于验证 B4 自动回滚链路 */
let forceApplyFailure = false;
export function __devForceApplyFailure(enabled: boolean): void {
  forceApplyFailure = enabled;
}

// ─────────────────────────────────────────────────────────
// 事件总线
// ─────────────────────────────────────────────────────────

type Handler = (payload: unknown) => void;
const listeners = new Map<string, Set<Handler>>();

function emit<K extends OrbisEventName>(event: K, payload: OrbisEventMap[K]): void {
  listeners.get(event)?.forEach((h) => h(payload));
}

function subscribe<K extends OrbisEventName>(
  event: K,
  handler: (payload: OrbisEventMap[K]) => void,
): () => void {
  const set = listeners.get(event) ?? new Set<Handler>();
  set.add(handler as Handler);
  listeners.set(event, set);
  return () => set.delete(handler as Handler);
}

// ─────────────────────────────────────────────────────────
// 派生计算（04 §6.4 / §5.1）
// ─────────────────────────────────────────────────────────

/** 04 §5.1 归一化契约：取前两段数值；任一段缺失/非数值 → null（不猜） */
function normalize(raw: string | null): string | null {
  if (!raw) return null;
  const parts = raw.split('.');
  if (parts.length < 2) return null;
  const [a, b] = parts;
  if (!/^\d+$/.test(a) || !/^\d+$/.test(b)) return null;
  return `${a}.${b}`;
}

/** 数值点分比较器（游戏版本非 semver，04 §5.2） */
function compareNorm(a: string, b: string): number {
  const [a1, a2] = a.split('.').map(Number);
  const [b1, b2] = b.split('.').map(Number);
  return a1 !== b1 ? a1 - b1 : a2 - b2;
}

function toolsOf(gameId: GameId): ToolDto[] {
  return MANIFESTS.filter((m) => m.game === gameId).map((m) => toToolDto(m));
}

function toToolDto(m: ManifestShape): ToolDto {
  const inst = state.installations.find((i) => i.gameId === m.game);
  const versionNorm = normalize(inst?.localVersion ?? null);
  const gameId = m.game as GameId;
  return {
    id: m.id,
    gameId,
    name: m.name,
    description: m.description,
    version: m.version,
    type: m.type as ToolType,
    riskLevel: m.risk_level as ToolDto['riskLevel'],
    permissions: m.permissions as ToolPermission[],
    requiresAdmin: m.requires_admin,
    backupRequired: m.backup_required,
    enabled: state.toolEnabled.get(m.id) ?? false,
    compat: queryCompat(gameId, m.id, versionNorm),
    asset: m.entry.asset ? toAssetDto(m) : null,
    source: {
      kind: m.source.kind as ToolDto['source']['kind'],
      repo: m.source.repo,
      license: m.source.license,
      version: m.source.version,
    },
    pendingVerifications: m.pending_verifications,
  };
}

function toAssetDto(m: ManifestShape): ToolAssetDto {
  const asset = ASSETS.assets.find((a) => a.asset_key === m.entry.asset);
  const artifact = asset?.artifacts[0] ?? null;
  return {
    toolId: m.id,
    assetKey: m.entry.asset ?? '',
    // MVP 内未实现资产落盘（Q1 未定稿），恒为 missing
    state: 'missing',
    version: artifact?.version ?? null,
    sha256: null,
    expectedSha256: artifact?.sha256 ?? null,
    installedPath: null,
    // Q1 未定稿：artifacts 为空 → UI 走「组件构建管道待定稿」降级（契约 §3.6）
    downloadUrlConfigured: Boolean(artifact?.url),
  };
}

function toInstallationDto(s: InstallationSeed): InstallationDto {
  const versionNorm = normalize(s.localVersion);
  const versionUnknown = versionNorm === null;
  const updateAvailable =
    versionNorm !== null &&
    s.remoteVersion !== null &&
    compareNorm(versionNorm, s.remoteVersion) < 0;

  const reasons: InstallationDto['attentionReasons'] = [];
  const status = state.runtime.get(s.id) ?? 'installed';
  if (status === 'broken') reasons.push('broken');
  if (versionUnknown) reasons.push('version_unknown');
  if (updateAvailable) reasons.push('update_available');
  for (const t of toolsOf(s.gameId)) {
    if (t.compat.status === 'unknown') reasons.push('tool_unknown');
    if (t.compat.status === 'incompatible' || t.compat.status === 'deprecated') {
      reasons.push('tool_incompatible');
    }
  }

  return {
    id: s.id,
    gameId: s.gameId,
    region: s.region,
    installPath: s.installPath,
    executablePath: s.executablePath,
    localVersion: s.localVersion,
    versionNorm,
    versionSource: s.versionSource,
    status,
    updateAvailable,
    versionUnknown,
    needsAttention: reasons.length > 0,
    attentionReasons: [...new Set(reasons)],
    addedVia: 'scan',
    pid: status === 'running' ? s.pid : null,
    playtime: { ...s.playtime },
    hasConfigSource: s.hasConfigSource,
    configUnsupportedReason: s.configUnsupportedReason,
    createdAt: now() - 30 * DAY,
    updatedAt: now() - HOUR,
  };
}

function listInstallationsInternal() {
  const installations = state.installations.map(toInstallationDto);
  return {
    installations,
    summary: {
      total: installations.length,
      needsAttention: installations.filter((i) => i.needsAttention).length,
      updateAvailable: installations.filter((i) => i.updateAvailable).length,
      broken: installations.filter((i) => i.status === 'broken').length,
      toolAttention: installations.filter((i) =>
        i.attentionReasons.some(
          (r) => r === 'tool_unknown' || r === 'tool_incompatible',
        ),
      ).length,
    },
  };
}

function findInstallation(id: string): InstallationSeed {
  const inst = state.installations.find((i) => i.id === id);
  if (!inst) throw err('GAME_NOT_FOUND', `installation ${id} not found`);
  return inst;
}

function err(
  code: ErrorCode,
  message: string,
  detail: Record<string, string | number | boolean> | null = null,
  retryable = false,
): OrbisInvokeError {
  return new OrbisInvokeError({
    code,
    message,
    detail,
    retryable,
  });
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ─────────────────────────────────────────────────────────
// apply 步骤（两层 detail，03 §5.6 U8）
// ─────────────────────────────────────────────────────────

type StepSpec = {
  step: ApplyStep;
  default: string;
  advanced: string[] | null;
  state: 'completed' | 'failed' | 'skipped';
};

function buildApplySteps(toolId: string, versionNorm: string | null): StepSpec[] {
  const isWuwa = toolId === 'orbis-builtin/wuwa-fps-120';
  const compat = queryCompat(
    isWuwa ? 'wuthering-waves' : 'genshin-impact',
    toolId,
    versionNorm,
  );
  return [
    {
      step: 'gate',
      default: '检查兼容性',
      advanced: [
        `兼容状态 ${compat.status} · 匹配方式 ${compat.matchKind}${
          compat.matchedVersionKey ? ` · 命中条目 ${compat.matchedVersionKey}` : ''
        }`,
        compat.notes ?? '',
      ].filter(Boolean),
      state: 'completed',
    },
    {
      step: 'precheck',
      default: '确认游戏已退出',
      advanced: ['游戏进程未运行', '目标目录写入权限正常'],
      state: 'completed',
    },
    {
      step: 'backup',
      default: '备份原配置',
      advanced: ['LocalStorage.db（整目录快照）', '备份 bk-42 · 420 KB'],
      state: 'completed',
    },
    {
      step: 'modify',
      default: '把帧率上限改为 120',
      advanced: ['CustomFrameRate ← 120', 'GameQualitySetting.KeyCustomFrameRate ← 120'],
      state: 'completed',
    },
    {
      step: 'verify',
      default: '确认修改已生效',
      advanced: ['读回 CustomFrameRate = 120 ✓', '读回 KeyCustomFrameRate = 120 ✓'],
      state: forceApplyFailure ? 'failed' : 'completed',
    },
  ];
}

// ─────────────────────────────────────────────────────────
// mock API
// ─────────────────────────────────────────────────────────

export const mockApi: OrbisApi = {
  subscribe,

  async listGames() {
    await wait(50);
    return GAME_CATALOG.map((g) => ({ ...g, regions: [...g.regions] }));
  },

  async scanGames() {
    const scanId = `scan-${++state.scanCount}`;
    const total = state.installations.length;
    const phases: {
      phase: OrbisEventMap['scan:progress']['phase'];
      upto: number;
    }[] = [
      { phase: 'registry', upto: 2 },
      { phase: 'paths', upto: 4 },
      { phase: 'validate', upto: total },
    ];
    for (const { phase, upto } of phases) {
      for (let i = 1; i <= upto; i++) {
        emit('scan:progress', {
          scanId,
          phase,
          scanned: i,
          total,
          currentGameId: state.installations[i - 1]?.gameId ?? null,
        });
        await wait(90);
      }
    }
    emit('scan:progress', {
      scanId,
      phase: 'done',
      scanned: total,
      total,
      currentGameId: null,
    });
    const { installations } = listInstallationsInternal();
    return { scanId, found: total, installations, partial: false, warnings: [] };
  },

  async listInstallations() {
    await wait(120);
    return listInstallationsInternal();
  },

  async getInstallationDetail(installationId) {
    const inst = findInstallation(installationId);
    const dto = toInstallationDto(inst);
    const backups = state.backups
      .filter((b) => b.installationId === installationId)
      .sort((a, b) => b.createdAt - a.createdAt);
    const profile = state.launchProfiles.get(installationId);
    return {
      ...dto,
      tools: toolsOf(inst.gameId),
      latestBackup: backups[0] ?? null,
      launchProfile: {
        installationId,
        args: profile?.args ?? '',
        updatedAt: profile?.updatedAt ?? dto.updatedAt,
      },
    };
  },

  async validateExecutable(gameId, executablePath) {
    await wait(160);
    const lower = executablePath.toLowerCase();
    if (!lower.endsWith('.exe')) {
      return { ok: false, detectedGameId: null, reason: 'not_an_executable', versionRaw: null };
    }
    // A2 边界：识别「选到官方启动器而非游戏本体」
    if (/(launcher|starter|hyp|krlauncher|gryphlink)/.test(lower)) {
      return { ok: false, detectedGameId: null, reason: 'looks_like_launcher', versionRaw: null };
    }
    const owner = state.installations.find((i) =>
      lower.includes(i.executablePath.split('/').pop()!.toLowerCase()),
    );
    if (owner && owner.gameId !== gameId) {
      return {
        ok: false,
        detectedGameId: owner.gameId,
        reason: 'game_mismatch',
        versionRaw: owner.localVersion,
      };
    }
    return {
      ok: true,
      detectedGameId: gameId,
      reason: 'ok',
      versionRaw: owner?.localVersion ?? null,
    };
  },

  async addInstallation(input) {
    await wait(300);
    if (
      state.installations.some(
        (i) => i.executablePath.toLowerCase() === input.executablePath.toLowerCase(),
      )
    ) {
      throw err('INSTALLATION_DUPLICATE', 'executable already managed');
    }
    const seedLike = SEED_INSTALLATIONS.find((i) => i.gameId === input.gameId);
    const created: InstallationSeed = {
      id: `inst-${input.gameId}-${Date.now()}`,
      gameId: input.gameId,
      region: input.region ?? 'cn',
      installPath: input.executablePath.replace(/[\\/][^\\/]+$/, ''),
      executablePath: input.executablePath,
      localVersion: null,
      versionSource: 'directory',
      remoteVersion: null,
      versionSourceId: 'degraded',
      hasConfigSource: seedLike?.hasConfigSource ?? false,
      configUnsupportedReason: seedLike?.configUnsupportedReason ?? 'provider_not_declared',
      playtime: { todaySec: 0, weekSec: 0, totalSec: 0 },
      pid: null,
    };
    state.installations.push(created);
    state.runtime.set(created.id, 'installed');
    return toInstallationDto(created);
  },

  async removeInstallation(installationId) {
    await wait(180);
    findInstallation(installationId);
    state.installations = state.installations.filter((i) => i.id !== installationId);
    state.runtime.delete(installationId);
  },

  async launchGame(installationId, opts) {
    const inst = findInstallation(installationId);
    if (state.runtime.get(installationId) === 'running') {
      throw err('GAME_ALREADY_RUNNING', 'game is already running');
    }
    await wait(900);
    const pid = Math.floor(10000 + Math.random() * 60000);
    inst.pid = pid;
    state.runtime.set(installationId, 'running');

    // B7/B8：原神解锁编排（失败 → 降级普通启动，不阻塞游戏本体）
    const unlockTool = 'orbis-bundled/genshin-fps-unlock';
    const wantsUnlock =
      inst.gameId === 'genshin-impact' && (state.toolEnabled.get(unlockTool) ?? false);
    const skipped = Boolean(opts?.skipUnlock);
    const asset = toAssetDto(MANIFESTS.find((m) => m.id === unlockTool)!);
    const attachFailed = wantsUnlock && !skipped && !asset.downloadUrlConfigured;

    emit('game:state-changed', {
      installationId,
      status: 'running',
      updateAvailable: toInstallationDto(inst).updateAvailable,
      versionUnknown: normalize(inst.localVersion) === null,
      versionNorm: normalize(inst.localVersion),
      pid,
    });

    return {
      installationId,
      pid,
      unlock: {
        requested: wantsUnlock && !skipped,
        attached: wantsUnlock && !skipped && !attachFailed,
        skipped,
        degradedReason: attachFailed ? 'TOOL_ASSET_NOT_CONFIGURED' : null,
      },
    };
  },

  async terminateGame(installationId) {
    const inst = findInstallation(installationId);
    if (state.runtime.get(installationId) !== 'running') {
      throw err('GAME_NOT_RUNNING', 'game is not running');
    }
    await wait(240);
    inst.pid = null;
    state.runtime.set(installationId, 'installed');
    emit('game:state-changed', {
      installationId,
      status: 'installed',
      updateAvailable: toInstallationDto(inst).updateAvailable,
      versionUnknown: normalize(inst.localVersion) === null,
      versionNorm: normalize(inst.localVersion),
      pid: null,
    });
  },

  async getRuntimeStates() {
    await wait(80);
    return state.installations.map((i) => ({
      installationId: i.id,
      status: state.runtime.get(i.id) ?? 'installed',
      pid: state.runtime.get(i.id) === 'running' ? i.pid : null,
      updateAvailable: toInstallationDto(i).updateAvailable,
      versionUnknown: normalize(i.localVersion) === null,
    }));
  },

  async getLaunchProfile(installationId) {
    await wait(80);
    findInstallation(installationId);
    const p = state.launchProfiles.get(installationId);
    return {
      installationId,
      args: p?.args ?? '',
      updatedAt: p?.updatedAt ?? now(),
    };
  },

  async setLaunchProfile(installationId, args) {
    findInstallation(installationId);
    state.launchProfiles.set(installationId, { args, updatedAt: now() });
    return { installationId, args, updatedAt: now() };
  },

  async resetLaunchProfile(installationId) {
    findInstallation(installationId);
    state.launchProfiles.delete(installationId);
  },

  async getPlaytime(scope, installationId) {
    await wait(90);
    const pool = state.installations.filter(
      (i) => !installationId || i.id === installationId,
    );
    const pick = (i: InstallationSeed) =>
      scope === 'today' ? i.playtime.todaySec : scope === 'week' ? i.playtime.weekSec : i.playtime.totalSec;
    return {
      scope,
      totalSec: pool.reduce((sum, i) => sum + pick(i), 0),
      perInstallation: pool.map((i) => ({
        installationId: i.id,
        gameId: i.gameId,
        seconds: pick(i),
      })),
    };
  },

  async refreshRemoteVersions() {
    await wait(700);
    const results: RemoteVersionResult[] = state.installations.map((i) => ({
      gameId: i.gameId,
      region: i.region,
      version: i.remoteVersion,
      source: i.versionSourceId,
      fetchedAt: now(),
      // 终末地降级：返回 null + 降级信息（非异常路径，02 E1 不误报）
      error:
        i.remoteVersion === null
          ? {
              code: 'VERSION_SOURCE_UNSUPPORTED',
              message: 'no public manifest for this publisher',
              detail: { gameId: i.gameId },
              retryable: false,
              }
          : null,
    }));
    emit('version:refreshed', { fetchedAt: now(), results });
    return results;
  },

  async getUpdateStatus(gameId) {
    await wait(90);
    return state.installations
      .filter((i) => !gameId || i.gameId === gameId)
      .map((i) => {
        const norm = normalize(i.localVersion);
        const dto = toInstallationDto(i);
        return {
          installationId: i.id,
          gameId: i.gameId,
          localVersionNorm: norm,
          remoteVersion: i.remoteVersion,
          status: i.remoteVersion === null
            ? ('unknown' as const)
            : dto.updateAvailable
              ? ('update_available' as const)
              : ('up_to_date' as const),
          checkedAt: now() - 120_000,
          source: i.versionSourceId,
        };
      });
  },

  async listTools(gameId) {
    await wait(90);
    return MANIFESTS.filter((m) => !gameId || m.game === gameId).map(toToolDto);
  },

  async getTool(toolId) {
    await wait(90);
    const m = MANIFESTS.find((x) => x.id === toolId);
    if (!m) throw err('TOOL_NOT_FOUND', `tool ${toolId} not found`);
    const dto = toToolDto(m);
    const consentText =
      dto.riskLevel === 'L3'
        ? [
            '这个工具会在游戏运行时修改内存中的帧率上限。',
            '',
            '· 它不修改游戏文件，关闭游戏即恢复原状。',
            '· 进程级调整存在触发游戏反作弊审查的可能。',
            '· 这是社区多年的实践方式，并非官方授权。',
            '',
            '本项目不承诺绝对安全，相关风险由你知情并自行承担。',
          ].join('\n')
        : null;
    return {
      ...dto,
      consentText,
      consentTextHash: consentText ? `sha256:${hashLite(consentText)}` : null,
      permissionsDetail: dto.permissions.map((p) => ({
        permission: p,
        label: PERMISSION_LABEL[p],
        detail: PERMISSION_DETAIL[p],
      })),
    };
  },

  async setToolEnabled(toolId, enabled, opts) {
    const m = MANIFESTS.find((x) => x.id === toolId);
    if (!m) throw err('TOOL_NOT_FOUND', `tool ${toolId} not found`);
    const inst = state.installations.find((i) => i.gameId === m.game)!;
    const dto = toToolDto(m);

    if (!enabled) {
      state.toolEnabled.set(toolId, false);
      return {
        toolId,
        enabled: false,
        outcome: 'unchanged',
        compat: dto.compat,
        backupId: null,
        rollbackBackupId: null,
        blockedReason: null,
      };
    }

    // ── [Gate] 兼容门控三层（04 §5.5）───────────────────
    const isL3 = dto.riskLevel === 'L3';
    if (dto.compat.status === 'incompatible' || dto.compat.status === 'deprecated') {
      throw err('COMPAT_BLOCKED', `compat=${dto.compat.status}`, { toolId });
    }
    if (dto.compat.status === 'unknown') {
      if (isL3) throw err('COMPAT_UNKNOWN_L3', 'L3 blocked on unknown compat', { toolId });
      if (!opts?.overrideCompat) {
        throw err('COMPAT_OVERRIDE_REQUIRED', 'L1 unknown requires explicit override', { toolId });
      }
    }
    if (isL3 && !state.toolAuthorized.get(toolId)) {
      throw err('CONSENT_REQUIRED', 'L3 authorization not granted', { toolId });
    }

    // ── 逐步执行 + 事件（D4 透明卡片）──────────────────
    const steps = buildApplySteps(toolId, dto.compat.matchKind === 'none' ? null : toInstallationDto(inst).versionNorm);
    let failed = false;
    for (const s of steps) {
      emit('tool:apply-step', {
        toolId,
        installationId: inst.id,
        step: s.step,
        state: 'running',
        detail: { default: s.default, advanced: s.advanced },
      });
      await wait(420);
      if (s.state === 'failed') {
        failed = true;
        emit('tool:apply-step', {
          toolId,
          installationId: inst.id,
          step: s.step,
          state: 'failed',
          detail: { default: s.default, advanced: ['读回值与期望不符'] },
        });
        break;
      }
      emit('tool:apply-step', {
        toolId,
        installationId: inst.id,
        step: s.step,
        state: 'completed',
        detail: { default: s.default, advanced: s.advanced },
      });
    }

    if (failed) {
      // ── [RollingBack] B4：自动回滚 ───────────────────
      emit('tool:apply-step', {
        toolId,
        installationId: inst.id,
        step: 'rollback',
        state: 'running',
        detail: { default: '正在恢复原配置', advanced: null },
      });
      await wait(520);
      emit('tool:apply-step', {
        toolId,
        installationId: inst.id,
        step: 'rollback',
        state: 'completed',
        detail: {
          default: '修改失败，已自动恢复原配置',
          advanced: ['已还原至 bk-42 · 哈希校验通过'],
        },
      });
      state.toolEnabled.set(toolId, false);
      return {
        toolId,
        enabled: false,
        outcome: 'rolled_back',
        compat: dto.compat,
        backupId: 'bk-42',
        rollbackBackupId: 'bk-42',
        blockedReason: null,
      };
    }

    state.toolEnabled.set(toolId, true);
    return {
      toolId,
      enabled: true,
      outcome: 'applied',
      compat: dto.compat,
      backupId: dto.backupRequired ? 'bk-42' : null,
      rollbackBackupId: null,
      blockedReason: null,
    };
  },

  async grantToolAuthorization(toolId, consentTextHash) {
    if (!MANIFESTS.some((m) => m.id === toolId)) {
      throw err('TOOL_NOT_FOUND', `tool ${toolId} not found`);
    }
    state.toolAuthorized.set(toolId, consentTextHash);
  },

  async revokeToolAuthorization(toolId) {
    state.toolAuthorized.set(toolId, null);
    state.toolEnabled.set(toolId, false);
  },

  async getToolAssetStatus(toolId) {
    const m = MANIFESTS.find((x) => x.id === toolId);
    if (!m || !m.entry.asset) throw err('TOOL_NOT_FOUND', `tool ${toolId} has no asset`);
    return toAssetDto(m);
  },

  async downloadToolAsset(toolId) {
    const m = MANIFESTS.find((x) => x.id === toolId);
    if (!m || !m.entry.asset) throw err('TOOL_NOT_FOUND', `tool ${toolId} has no asset`);
    const dto = toAssetDto(m);
    if (!dto.downloadUrlConfigured) {
      // Q1 未定稿：不是崩溃路径（契约 §5）
      throw err('TOOL_ASSET_NOT_CONFIGURED', 'assets.json has no artifact url yet', {
        toolId,
        assetKey: dto.assetKey,
      });
    }
    for (let i = 1; i <= 5; i++) {
      emit('tool:asset-progress', {
        toolId,
        phase: 'download',
        receivedBytes: i * 2000,
        totalBytes: 10000,
      });
      await wait(160);
    }
    return { ...dto, state: 'ready', installedPath: `%APPDATA%/orbis/tools/${dto.assetKey}` };
  },

  async getCompatibility(gameId, toolId) {
    await wait(70);
    const inst = state.installations.find((i) => i.gameId === gameId);
    return queryCompat(gameId, toolId, normalize(inst?.localVersion ?? null));
  },

  async createBackup(installationId, trigger) {
    const inst = findInstallation(installationId);
    if (state.runtime.get(installationId) === 'running') {
      throw err('GAME_PROCESS_ACTIVE', 'game is running');
    }
    if (inst.configUnsupportedReason) {
      throw err('CONFIG_SOURCE_UNSUPPORTED', 'provider declares no config path');
    }
    await wait(600);
    const created: BackupSummary = {
      id: `bk-${43 + state.backups.length}`,
      installationId,
      gameId: inst.gameId,
      toolId: null,
      trigger: trigger ?? 'manual',
      fileCount: 3,
      totalBytes: 430080,
      primaryFile: 'LocalStorage.db',
      createdAt: now(),
    };
    state.backups.unshift(created);
    return created;
  },

  async listBackups(installationId) {
    await wait(110);
    findInstallation(installationId);
    return state.backups
      .filter((b) => b.installationId === installationId)
      .sort((a, b) => b.createdAt - a.createdAt);
  },

  async restoreBackup(backupId) {
    const target = state.backups.find((b) => b.id === backupId);
    if (!target) throw err('BACKUP_NOT_FOUND', `backup ${backupId} not found`);
    if (state.runtime.get(target.installationId) === 'running') {
      throw err('GAME_PROCESS_ACTIVE', 'game is running');
    }
    await wait(800);
    // A8：恢复前必须先做 pre_restore 备份（任何恢复本身可撤销）
    const preRestore: BackupSummary = {
      id: `bk-${100 + state.backups.length}`,
      installationId: target.installationId,
      gameId: target.gameId,
      toolId: null,
      trigger: 'pre_restore',
      fileCount: 3,
      totalBytes: 430080,
      primaryFile: 'LocalStorage.db',
      createdAt: now(),
    };
    state.backups.unshift(preRestore);

    emit('tool:apply-step', {
      toolId: target.toolId ?? 'orbis-builtin/wuwa-fps-120',
      installationId: target.installationId,
      step: 'rollback',
      state: 'completed',
      detail: {
        default: `已恢复至${target.trigger === 'manual' ? '手动备份' : '自动备份'}的配置`,
        advanced: [`还原 ${target.id} · 逐文件哈希校验通过`],
      },
    });

    return {
      backupId,
      restoredFileCount: 3,
      verified: true,
      preRestoreBackupId: preRestore.id,
    };
  },

  async deleteBackup(backupId) {
    if (!state.backups.some((b) => b.id === backupId)) {
      throw err('BACKUP_NOT_FOUND', `backup ${backupId} not found`);
    }
    state.backups = state.backups.filter((b) => b.id !== backupId);
  },

  async getBackupStorageInfo(installationId) {
    findInstallation(installationId);
    const mine = state.backups.filter((b) => b.installationId === installationId);
    return {
      installationId,
      backupCount: mine.length,
      totalBytes: mine.reduce((s, b) => s + b.totalBytes, 0),
      freeDiskBytes: 128 * 1024 ** 3,
      estimatedNextSizeBytes: 430080,
    };
  },

  async getSettings() {
    await wait(60);
    return { ...state.settings };
  },

  async setSetting(key: SettingKey, value: boolean | number) {
    if (!(key in state.settings)) {
      throw err('SETTING_UNKNOWN_KEY', `unknown setting ${key}`, { key });
    }
    if (key === 'log.retention_days' && (typeof value !== 'number' || value < 1 || value > 365)) {
      throw err('SETTING_INVALID_VALUE', 'log.retention_days out of range 1-365', { key });
    }
    if (
      key === 'playtime.checkpoint_sec' &&
      (typeof value !== 'number' || value < 10 || value > 300)
    ) {
      throw err('SETTING_INVALID_VALUE', 'playtime.checkpoint_sec out of range 10-300', { key });
    }
    state.settings = { ...state.settings, [key]: value } as AppSettings;
    return { ...state.settings };
  },

  async windowControl() {
    // 仅 Tauri 环境有意义；mock 下 no-op
  },
};

// ─────────────────────────────────────────────────────────
// 辅助
// ─────────────────────────────────────────────────────────

const PERMISSION_LABEL: Record<ToolPermission, string> = {
  read_config: '读取游戏配置',
  write_config: '修改游戏配置',
  launch_external: '启动外部程序',
  process_attach: '访问游戏进程',
};

const PERMISSION_DETAIL: Record<ToolPermission, string> = {
  read_config: '读取当前帧率与画质相关设置，用于展示与还原对比。',
  write_config: '写入帧率上限设置。修改前会自动备份，可随时恢复。',
  launch_external: '在启动游戏时一并拉起解锁组件，无需你手动操作。',
  process_attach: '在游戏运行时调整内存中的帧率上限，不修改游戏文件。',
};

/** 仅用于生成 consentTextHash 的演示值；真实哈希在 Core 侧（sha2） */
function hashLite(input: string): string {
  let h = 0;
  for (let i = 0; i < input.length; i++) {
    h = (h * 31 + input.charCodeAt(i)) | 0;
  }
  return Math.abs(h).toString(16).padStart(16, '0').slice(0, 32);
}
