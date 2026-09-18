/**
 * 错误码 → 用户可见文案（契约 §5 / §7.3）
 *
 * 铁律：
 *  - `OrbisError.message` 是开发者信息，**禁止直接展示**（契约 §1）
 *  - 文案不得出现路径、键名、进程名、哈希等底层细节（00 §12.2）
 *  - 每个 code 必须给出「行动建议」，否则用户无法处置（02 各功能边界要求）
 */
import type { ErrorCode, OrbisInvokeError } from '../api/types';

export interface ErrorDisplay {
  /** 简短标题，用于 toast / 卡片标题 */
  title: string;
  /** 一句话解释 + 行动建议 */
  hint: string;
  /** 是否提供「重试」按钮（以 Core 返回的 retryable 为准，此处仅作缺省建议） */
  suggestRetry: boolean;
  /** 是否为「非错误」的降级路径，UI 应以中性样式呈现而非报错样式 */
  informational: boolean;
}

const TABLE: Record<ErrorCode, ErrorDisplay> = {
  GAME_NOT_FOUND: {
    title: '找不到这个游戏',
    hint: '它可能已被移除，请刷新列表后重试。',
    suggestRetry: true,
    informational: false,
  },
  GAME_ALREADY_RUNNING: {
    title: '游戏已在运行',
    hint: '无需重复启动，可以直接切换到游戏窗口。',
    suggestRetry: false,
    informational: true,
  },
  GAME_NOT_RUNNING: {
    title: '游戏已经退出了',
    hint: '状态已更新。',
    suggestRetry: false,
    informational: true,
  },
  EXECUTABLE_MISSING: {
    title: '找不到游戏程序',
    hint: '游戏可能被移动或卸载了，请重新指定程序位置。',
    suggestRetry: true,
    informational: false,
  },
  EXECUTABLE_INVALID: {
    title: '无法识别这个文件',
    hint: '请选择游戏本体的可执行文件，而不是启动器或快捷方式。',
    suggestRetry: false,
    informational: false,
  },
  LOOKS_LIKE_LAUNCHER: {
    title: '这是官方启动器，不是游戏本体',
    hint: 'Orbis 需要指向游戏本体程序，否则无法启动与记录时长。',
    suggestRetry: false,
    informational: false,
  },
  GAME_MISMATCH: {
    title: '选的程序属于另一款游戏',
    hint: '请确认选择的程序与当前游戏一致。',
    suggestRetry: false,
    informational: false,
  },
  INSTALLATION_DUPLICATE: {
    title: '这个游戏已经添加过了',
    hint: '无需重复添加，可以在列表中直接管理它。',
    suggestRetry: false,
    informational: true,
  },
  PATH_NOT_FOUND: {
    title: '路径不存在',
    hint: '请确认文件仍然存在，或重新指定位置。',
    suggestRetry: true,
    informational: false,
  },
  PATH_PERMISSION_DENIED: {
    title: '没有写入权限',
    hint: '游戏安装在受保护目录下。请以管理员身份运行 Orbis，或把游戏安装到普通目录。',
    suggestRetry: false,
    informational: false,
  },
  GAME_PROCESS_ACTIVE: {
    title: '请先完全退出游戏',
    hint: '游戏或启动器仍在运行，此时修改会造成配置被覆盖。退出后再试。',
    suggestRetry: true,
    informational: false,
  },
  CONFIG_SOURCE_UNSUPPORTED: {
    title: '该游戏暂不支持配置备份',
    hint: '我们还没有为这款游戏确认安全的配置范围，确认后会开放。',
    suggestRetry: false,
    informational: true,
  },
  COMPAT_BLOCKED: {
    title: '当前版本不兼容',
    hint: '这个工具已被标记为不适用于你当前的游戏版本，为避免损坏配置已阻止操作。建议先恢复此前的配置。',
    suggestRetry: false,
    informational: false,
  },
  COMPAT_UNKNOWN_L3: {
    title: '尚未验证，已阻止启用',
    hint: '进程级工具在未验证的版本上默认不允许启用。可以等待验证结果，或先用普通模式启动游戏。',
    suggestRetry: false,
    informational: false,
  },
  COMPAT_OVERRIDE_REQUIRED: {
    title: '当前版本尚未验证',
    hint: '这个工具还没有在你当前的游戏版本上验证过。了解风险后可以选择「本次继续使用」。',
    suggestRetry: false,
    informational: false,
  },
  CONSENT_REQUIRED: {
    title: '需要先确认风险说明',
    hint: '启用前请完整阅读并确认风险说明，之后再试一次。',
    suggestRetry: false,
    informational: false,
  },
  BACKUP_NOT_FOUND: {
    title: '找不到这条备份',
    hint: '它可能已被清理，请刷新历史列表。',
    suggestRetry: true,
    informational: false,
  },
  BACKUP_FAILED: {
    title: '备份失败，已取消修改',
    hint: '没有备份就不会改动配置。请检查磁盘空间与文件占用后重试。',
    suggestRetry: true,
    informational: false,
  },
  BACKUP_SPACE_INSUFFICIENT: {
    title: '磁盘空间不足',
    hint: '备份需要更多可用空间。清理一些空间后再试，配置未被修改。',
    suggestRetry: true,
    informational: false,
  },
  RESTORE_HASH_MISMATCH: {
    title: '恢复未完全成功',
    hint: '校验发现文件与备份不一致。可用「恢复前」自动生成的备份再回退一次。',
    suggestRetry: false,
    informational: false,
  },
  TOOL_NOT_FOUND: {
    title: '找不到这个工具',
    hint: '它可能已在本次更新中变化，请刷新后重试。',
    suggestRetry: true,
    informational: false,
  },
  TOOL_ASSET_NOT_CONFIGURED: {
    title: '组件尚未发布',
    hint: '这个工具所需的组件还在准备中。你仍可以正常启动游戏，本次不会应用解锁。',
    suggestRetry: false,
    informational: true,
  },
  TOOL_ASSET_DOWNLOAD_FAILED: {
    title: '组件下载失败',
    hint: '请检查网络后重试。下载失败不影响正常启动游戏。',
    suggestRetry: true,
    informational: false,
  },
  TOOL_ASSET_HASH_MISMATCH: {
    title: '组件校验未通过',
    hint: '下载内容与预期不一致，已拒绝加载以保护你的设备。请重新下载。',
    suggestRetry: true,
    informational: false,
  },
  NETWORK_UNREACHABLE: {
    title: '暂时无法获取更新信息',
    hint: '网络不可达，版本状态显示为「未知」——我们不会用旧数据猜测。稍后可手动刷新。',
    suggestRetry: true,
    informational: true,
  },
  VERSION_SOURCE_UNSUPPORTED: {
    title: '该游戏暂不支持自动检测更新',
    hint: '厂商没有公开的版本接口。请通过官方启动器确认更新情况。',
    suggestRetry: false,
    informational: true,
  },
  SETTING_UNKNOWN_KEY: {
    title: '设置项不可用',
    hint: '这看起来是程序内部问题，请反馈给我们。',
    suggestRetry: false,
    informational: false,
  },
  SETTING_INVALID_VALUE: {
    title: '设置值超出范围',
    hint: '请调整到允许的范围内。',
    suggestRetry: false,
    informational: false,
  },
  NOT_IMPLEMENTED: {
    title: '功能尚未完成',
    hint: '这个能力还在开发中。',
    suggestRetry: false,
    informational: true,
  },
  INTERNAL: {
    title: '出了点问题',
    hint: '操作没有完成。可以打开日志目录把日志发给我们，帮助定位。',
    suggestRetry: true,
    informational: false,
  },
};

export function errorDisplay(err: OrbisInvokeError): ErrorDisplay {
  return TABLE[err.code] ?? TABLE.INTERNAL;
}

/** 兼容非 OrbisInvokeError 的异常（如未捕获的 TypeError） */
export function toErrorDisplay(err: unknown): ErrorDisplay {
  if (err && typeof err === 'object' && 'code' in err) {
    const code = (err as { code: ErrorCode }).code;
    if (code in TABLE) return TABLE[code];
  }
  return TABLE.INTERNAL;
}
