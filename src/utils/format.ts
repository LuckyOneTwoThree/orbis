/**
 * 展示格式化工具
 *
 * 约定（契约 §1 / §7.3）：后端只给数字与枚举，**格式化全部在 UI 侧**。
 * 因此这里不存在任何业务判断，只有纯粹的表现层转换。
 */
import type { BackupTrigger, GameRuntimeStatus, Region, RiskLevel, ToolCompatStatus } from '../api/types';

const MINUTE = 60;
const HOUR = 3600;

/** 时长：秒 → 人类可读。口径 = 游戏进程存活时长（02 A6，UI 必须明示该口径） */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0m';
  if (seconds < MINUTE) return '<1m';
  if (seconds < HOUR) return `${Math.round(seconds / MINUTE)}m`;
  const hours = seconds / HOUR;
  if (hours < 10) return `${hours.toFixed(1)}h`;
  return `${Math.round(hours)}h`;
}

/** 时长：秒 → 「2h 32m」明细形（用于管理面板，A6 统计更精确的呈现） */
export function formatDurationLong(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0 分钟';
  const h = Math.floor(seconds / HOUR);
  const m = Math.round((seconds % HOUR) / MINUTE);
  if (h === 0) return `${m} 分钟`;
  if (m === 0) return `${h} 小时`;
  return `${h} 小时 ${m} 分`;
}

function startOfLocalDay(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** 时间：epoch 毫秒 → 「今天 14:32」/「昨天 21:05」/「9月16日 14:32」 */
export function formatDateTime(epochMs: number): string {
  const now = new Date();
  const today0 = startOfLocalDay(now.getTime());
  const target0 = startOfLocalDay(epochMs);
  const dayDiff = Math.round((today0 - target0) / 86_400_000);
  const d = new Date(epochMs);
  const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  if (dayDiff === 0) return `今天 ${hm}`;
  if (dayDiff === 1) return `昨天 ${hm}`;
  return `${d.getMonth() + 1}月${d.getDate()}日 ${hm}`;
}

/** 相对时间：「刚刚」/「12 分钟前」/「3 小时前」 */
export function formatRelative(epochMs: number): string {
  const diff = Date.now() - epochMs;
  if (diff < 60_000) return '刚刚';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return `${Math.floor(diff / 86_400_000)} 天前`;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

// ── 枚举 → 展示文案（契约 §7.3：文案责任在 UI）──────────────

/** 区服：库内为 cn/global/bili，展示层才转中文 */
export function formatRegion(region: Region): string {
  return { cn: '国服', global: '国际服', bili: 'B服' }[region] ?? region;
}

export function formatRiskLabel(level: RiskLevel): string {
  switch (level) {
    case 'L0':
      return 'L0 只读';
    case 'L1':
      return 'L1 配置修改';
    case 'L2':
      return 'L2 启动增强';
    case 'L3':
      return 'L3 进程级';
  }
}

/** 兼容状态：主文案（03 §2.2 的语义映射） */
export function formatCompatLabel(status: ToolCompatStatus): string {
  switch (status) {
    case 'verified':
      return '已验证 · 适用当前版本';
    case 'compatible':
      return '推断兼容 · 尚未实机验证';
    case 'unknown':
      return '未验证 · 当前版本无适配记录';
    case 'incompatible':
      return '不兼容 · 已知失效';
    case 'deprecated':
      return '已停止维护';
  }
}

/** 兼容状态：行动建议（02 B6 要求 Unknown 必须给建议） */
export function compatAdvice(status: ToolCompatStatus): string | null {
  switch (status) {
    case 'unknown':
      return '可等待验证结果，或恢复到修改前的配置';
    case 'incompatible':
      return '请勿启用；建议恢复到修改前的配置';
    case 'deprecated':
      return '请改用替代方案；可恢复到修改前的配置';
    default:
      return null;
  }
}

/** 备份触发方式：pre_modify 对用户呈现为「自动备份」（03 §5.7 S5） */
export function formatBackupTrigger(trigger: BackupTrigger): string {
  return { manual: '手动', pre_modify: '自动', pre_restore: '恢复前' }[trigger];
}

/** 游戏运行态 → 主标签（04 §6.4.2 优先级由 Core 的 status 单值保证） */
export function formatRuntimeStatus(status: GameRuntimeStatus): string {
  return {
    installed: '已就绪',
    running: '运行中',
    updating: '更新中',
    repairing: '修复中',
    broken: '异常',
  }[status];
}

/** 状态颜色语义（03 §2.2）：danger 仅留给危险态 */
export type Tone = 'accent' | 'warn' | 'danger' | 'neutral' | 'success';

export function runtimeTone(status: GameRuntimeStatus): Tone {
  switch (status) {
    case 'running':
      return 'success';
    case 'broken':
      return 'danger';
    case 'updating':
    case 'repairing':
      return 'accent';
    default:
      return 'neutral';
  }
}

export function compatTone(status: ToolCompatStatus): Tone {
  switch (status) {
    case 'verified':
    case 'compatible':
      return 'success';
    case 'unknown':
      return 'warn';
    case 'incompatible':
    case 'deprecated':
      return 'danger';
  }
}
