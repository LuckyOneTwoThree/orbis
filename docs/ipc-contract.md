---
title: Orbis 前后端契约 v1
doc_id: IPC-1
type: interface-contract
status: frozen
version: v1.0
stage: 开工前置 → 冻结
parent: 04-技术设计 v0.3 §4.3
upstream:
  - pm/04-技术设计.md v0.3（§4.3 命令面索引 / §5.5 状态机 / §6.4 状态模型 / §8 降级策略）
  - pm/00-产品基石.md v0.7（§7.9 UI 解耦 / §7.10 实体 / §9.1 日志 schema）
  - pm/01-MVP功能清单.md v0.5（P0 范围契约）
  - pm/02-MVP-PRD.md v0.4（Given/When/Then 验收）
  - data/compatibility/seed.json（兼容性权威 · schema 0.1.0）
updated: 2026-09-18
tags:
  - contract
  - ipc
  - frozen
---

# Orbis 前后端契约 v1（冻结）

> **这是前后端接口的唯一字段级来源。** `pm/04-技术设计.md §4.3` 只保留分组索引，不重复字段定义（避免双源漂移，Q9）。
>
> 引入 `tauri-specta` 后，**Rust 侧签名成为最终单源**，本文件的角色降级为「设计说明 + 变更评审依据」；届时生成物为 `src/api/bindings.gen.ts`。
>
> 变更规则：本文为 `status: frozen`。任何字段增删必须先在本文件提案 → 评审 → 同步 `04 §4.3` 分组索引 → 再改实现。**不允许实现侧先行修改。**

---

## 1. 通用约定

| 项 | 约定 | 理由 |
|----|------|------|
| 命名 | 命令 camelCase；事件 `域:动作`（`scan:progress`）；枚举值 snake_case（`update_available`） | 与 04 §4.3 一致；枚举值直接入库（§6.1），避免展示文案入库 |
| 时间 | IPC 层一律 **Unix epoch 毫秒（`number`）**；日志文件用 ISO 8601 带时区 | DB 为 INTEGER（04 §6.1）；日志供人读（00 §9.1） |
| 时长 | 一律**秒（`number`）**，格式化归 UI | 后端不做展示格式化（§1.1 责任划分） |
| 路径 | OS 原生分隔符字符串（Windows 为 `\`）。**UI 只展示、不拼接、不做字符串运算** | 00 §7.9；路径决策全部在 Core |
| ID | 全部字符串。`toolId` = `namespace/name`（如 `orbis-builtin/wuwa-fps-120`）；`installationId` / `backupId` = UUID v4 | P0-C §1.2 |
| 版本 | 跨 IPC 传 `versionNorm`（`major.minor`）；需要展示原始值时用 `localVersion`，**禁止用原始值做比较或种子查询** | 04 §5.1 归一化契约 |
| 未知即 null | 无法识别的值一律 `null`，**不用空字符串、不用 `"Unknown"` 字符串**；UI 侧渲染成 `Unknown` | 02 A3「不猜、不显示过期缓存」 |
| 展示文案 | **命令返回值不含面向用户的成品文案**（除 Provider 声明的 `description` 与 L3 授权文案）。文案与行动建议归 UI，按 `code` 映射 | 00 §12.2 / i18n 预留（04 §1.3） |

---

## 2. 枚举与口径定稿

```ts
// ── 游戏标识（P0-C §1.2 slug，禁用其它写法）────────────────
type GameId =
  | 'genshin-impact'
  | 'honkai-star-rail'
  | 'zenless-zone-zero'
  | 'wuthering-waves'
  | 'arknights-endfield';

// ── 区服（04 §4.2）──────────────────────────────────────
// 库内与 IPC 一律小写；「国服」仅为 UI 展示映射，禁止入库或跨 IPC
type Region = 'cn' | 'global' | 'bili';

// ── 游戏运行态：互斥主状态（04 §6.4.1 ①）──────────────────
// MVP 实际只会出现 installed / running / broken
type GameRuntimeStatus =
  | 'installed' | 'running' | 'updating' | 'repairing' | 'broken';

// ── 工具兼容态：五态（00 §8.8 / P0-C §2.1，权威 = seed.json）──
type ToolCompatStatus =
  | 'verified' | 'compatible' | 'unknown' | 'incompatible' | 'deprecated';

// ── 风险分级（00 §8.1）──────────────────────────────────
type RiskLevel = 'L0' | 'L1' | 'L2' | 'L3';

// ── 工具类型 = 执行器分派键（04 §5.7）──────────────────────
type ToolType = 'config_modify' | 'external_process';

type ToolPermission =
  | 'read_config' | 'write_config' | 'launch_external' | 'process_attach';

// ── 备份触发方式（04 §5.4 / §6.1）────────────────────────
type BackupTrigger = 'manual' | 'pre_modify' | 'pre_restore';

// ── 版本源标识（04 §5.2）────────────────────────────────
type VersionSourceId =
  | 'hyp:getGamePackages' | 'kuro:index.json' | 'degraded';

// ── L1 执行链路步骤（04 §5.5 状态机，逐步骤发事件）────────────
type ApplyStep = 'gate' | 'precheck' | 'backup' | 'modify' | 'verify' | 'rollback';

// ── 设置项键（04 §6.1 预置键，**不得新增**，见 04 §7.4）────────
type SettingKey =
  | 'version_check.enabled'      // bool, default true   —— 02 E1「用户可关」
  | 'log.retention_days'         // int,  default 14     —— 04 §5.11 (Q4 已拍板)
  | 'playtime.checkpoint_sec';   // int,  default 30     —— 04 §5.10
```

### 2.1 需处理判定（`needsAttention`，04 §6.4.3）

```
attentionReasons(installation) :=
    'update_available'  当 updateAvailable == true
  ∨ 'broken'            当 status == 'broken'
  ∨ 'version_unknown'   当 versionUnknown == true
  ∨ 'tool_unknown'      当该游戏任一工具 compat == 'unknown'
  ∨ 'tool_incompatible' 当该游戏任一工具 compat ∈ {'incompatible','deprecated'}
```

- `needsAttention = attentionReasons.length > 0`
- **该判定在 Core 内计算一次**，随 `listInstallations()` 返回；UI 只消费，不重算（04 §3.2）
- 优先级（pill 只显示一个，04 §6.4.2）：`running > broken > updating/repairing > update_available > installed`

---

## 3. 命令面 v1

共 27 条。每条标注：验收映射（01/02）→ 错误码（§5）。

### 3.1 发现与实例管理（A1–A3）

```ts
listGames(): Promise<GameCatalogEntry[]>
scanGames(input?: { rescan?: boolean }): Promise<ScanResult>
listInstallations(): Promise<InstallationListResult>
getInstallationDetail(installationId: string): Promise<InstallationDetail>
validateExecutable(gameId: GameId, executablePath: string): Promise<ExecutableValidation>
addInstallation(input: {
  gameId: GameId; executablePath: string; region?: Region
}): Promise<InstallationDto>
removeInstallation(installationId: string): Promise<void>
```

- `listGames`：返回**编译期静态游戏目录**（00 §7.10：`Game` 实体不入库、不因扫描结果变化）。UI 用它渲染「未安装」行、Dashboard 标题、以及 A1 空状态里的官方入口引导。**必须有**——否则前端拿不到游戏显示名，只能自己硬编码一份目录，等于把 catalog 复制到 UI 层（违反 00 §7.9 与规则 1）
- `scanGames`：严格只读（L0）。并发 `scan:progress` 事件；超时/权限不足返回 `partial: true` + `warnings`，**不抛出**（02 A1 边界：不阻塞启动）
- `validateExecutable`：A2 边界要求 —— 必须能识别「用户选的是官方启动器而非游戏本体」；UI 在「确认添加」前调用，失败则提示而非静默接受
- `removeInstallation`：只移除条目与管理数据，**不删游戏文件与存档**（02 A2 验收）；备份目录是否级联删除由 `deleteBackups` 参数决定，默认保留
- 错误码：`GAME_NOT_FOUND` / `INSTALLATION_DUPLICATE` / `EXECUTABLE_INVALID` / `PATH_NOT_FOUND` / `PATH_PERMISSION_DENIED`

### 3.2 启动与运行状态（A4 / A5 / B8）

```ts
launchGame(
  installationId: string,
  opts?: { skipUnlock?: boolean }
): Promise<LaunchResult>
terminateGame(installationId: string): Promise<void>
getRuntimeStates(): Promise<RuntimeStateDto[]>
```

- `launchGame`：编排见 04 §5.9。已在运行 → `GAME_ALREADY_RUNNING`（A4 验收：显示「已在运行」不重复启动）
- `opts.skipUnlock` = B8「本次不解锁」开关：**不改变工具启用状态**（B8 验收）
- 原神解锁失败 → **降级普通启动**，`unlock.degradedReason` 非空，UI 明示「本次未解锁」，**绝不阻塞游戏本体**（02 B7 边界）
- `terminateGame`：UI 必须先展示数据损坏风险提示并取确认（A4 边界）；确认属 UI 责任，Core 只执行
- `getRuntimeStates`：首次拉取的快照；此后由 `game:state-changed` 增量驱动（A5：5 秒内反映）
- 错误码：`GAME_NOT_FOUND` / `GAME_ALREADY_RUNNING` / `GAME_NOT_RUNNING` / `COMPAT_BLOCKED` / `CONSENT_REQUIRED` / `EXECUTABLE_MISSING`

### 3.3 启动参数（A7）

```ts
getLaunchProfile(installationId: string): Promise<LaunchProfileDto>
setLaunchProfile(installationId: string, args: string): Promise<LaunchProfileDto>
resetLaunchProfile(installationId: string): Promise<void>
```

- `args` 为**单个字符串**（与 `launch_profile.args` 列一致）；切分由 Core 负责（引号/空格规则在 Core 单点实现）
- 参数冲突检测**不做**（P2，02 A7 边界）——本组命令不返回 warnings
- 错误码：`GAME_NOT_FOUND`

### 3.4 时长（A6）

```ts
getPlaytime(
  scope: 'today' | 'week' | 'total',
  installationId?: string      // 省略 = 全部安装实例
): Promise<PlaytimeResult>
```

- 口径 = **游戏进程存活时长**（02 A6 边界），UI 必须明示该口径
- 「今日 / 本周」按**本地时区自然日界**聚合（04 §5.10）
- 运行中由 `playtime:updated` 事件刷新；落库仍走 `playtime.checkpoint_sec`（默认 30s，A6 崩溃不丢）

### 3.5 版本与更新（E1）

```ts
refreshRemoteVersions(input?: { force?: boolean }): Promise<RemoteVersionResult[]>
getUpdateStatus(gameId?: GameId): Promise<UpdateStatusDto[]>
```

- 低频：应用启动时延迟触发（不阻塞冷启动 ≤3s）+ 手动刷新；受 `version_check.enabled` 控制（02 E1「用户可关」）
- 失败/不可达 → `version: null` + `error`，**状态为 `unknown`，不误报**（02 E1 验收）
- 终末地恒走 `degraded`：`version: null`，UI 标「调研中」+ 官方渠道引导（P0-B §3.2）
- 比较：双方先经 04 §5.1 归一化，在 `(major, minor)` 粒度比较
- 错误码：`NETWORK_UNREACHABLE` / `VERSION_SOURCE_UNSUPPORTED`（不视为错误路径，降级返回）

### 3.6 工具（C1–C3 / B7）

```ts
listTools(gameId?: GameId): Promise<ToolDto[]>
getTool(toolId: string): Promise<ToolDetail>
setToolEnabled(
  toolId: string, enabled: boolean,
  opts?: { overrideCompat?: boolean }
): Promise<ToolEnableResult>
grantToolAuthorization(toolId: string, consentTextHash: string): Promise<void>
revokeToolAuthorization(toolId: string): Promise<void>
getToolAssetStatus(toolId: string): Promise<ToolAssetDto>
downloadToolAsset(toolId: string): Promise<ToolAssetDto>
```

- `setToolEnabled`：走 04 §5.5 完整链路（Gate → Precheck → Backup → Modify → Verify），并发 `tool:apply-step` 事件驱动 D4 透明卡片
- `opts.overrideCompat` **仅对 L1 工具在 `unknown` 状态下有效**，且必须配合 UI 的显式确认；覆盖动作写日志（04 §5.5）。L3 `unknown` 一律硬阻止（02 B7「拦截并阻止」），传 `true` 也返回 `COMPAT_UNKNOWN_L3`
- `grantToolAuthorization`：`consentTextHash` = 用户实际看到的**版本化文案哈希**（防文案更新后沿用旧同意，04 §7.2）
- `downloadToolAsset`：仅 `external_process` 类需要；并发 `tool:asset-progress`；`assets.json` 未配置 URL 时返回 `TOOL_ASSET_NOT_CONFIGURED`（**不是崩溃路径**，UI 显示「组件构建管道待定稿」，对应 04 §11 Q1）
- 错误码：`TOOL_NOT_FOUND` / `COMPAT_BLOCKED` / `COMPAT_UNKNOWN_L3` / `COMPAT_OVERRIDE_REQUIRED` / `CONSENT_REQUIRED` / `TOOL_ASSET_NOT_CONFIGURED` / `TOOL_ASSET_HASH_MISMATCH` / `TOOL_ASSET_DOWNLOAD_FAILED` / `GAME_PROCESS_ACTIVE` / `PATH_PERMISSION_DENIED` / `BACKUP_FAILED` / `BACKUP_SPACE_INSUFFICIENT`

### 3.7 兼容性（C4 / B6）

```ts
getCompatibility(gameId: GameId, toolId: string): Promise<CompatibilityDto>
```

- 权威 = `data/compatibility/seed.json`（schema 0.1.0）；匹配 `exact → prefix → unknown`（P0-C §2.2）
- 入参不传版本：Core 内部取最新 `installation` 的 `version_norm`；无安装实例或版本未知 → `unknown` + `matchKind: 'none'`
- seed 缺失/损坏 → 一律 `unknown`，**不崩溃**（02 C4 验收）

### 3.8 备份与恢复（A8 / B5）

```ts
createBackup(installationId: string, trigger?: BackupTrigger): Promise<BackupSummary>
listBackups(installationId: string): Promise<BackupSummary[]>
restoreBackup(backupId: string): Promise<RestoreResult>
deleteBackup(backupId: string): Promise<void>
getBackupStorageInfo(installationId: string): Promise<BackupStorageInfo>
```

- 备份单元 = **整目录快照**（含 journal / db2 伴生文件），逐文件 SHA256（04 §5.4）
- `restoreBackup` 内部必须先建 `pre_restore` 备份（任何恢复本身可撤销，02 A8）；返回其 ID
- 前置检查：游戏/启动器运行中 → `GAME_PROCESS_ACTIVE`（02 A8 边界：阻止并提示先退出）；空间不足 → `BACKUP_SPACE_INSUFFICIENT`（04 §5.12，**禁止进入 Modify**）
- 若 `installation` 的 `configUnsupportedReason != null` → `CONFIG_SOURCE_UNSUPPORTED`（UI 本就应为禁用态，此处为兜底）
- `deleteBackup` 为破坏性操作，UI 需二次确认；操作本身写日志
- 错误码：`CONFIG_SOURCE_UNSUPPORTED` / `BACKUP_NOT_FOUND` / `GAME_PROCESS_ACTIVE` / `PATH_PERMISSION_DENIED` / `BACKUP_SPACE_INSUFFICIENT` / `BACKUP_FAILED` / `RESTORE_HASH_MISMATCH`

### 3.9 设置（D5 前置 / E1）

```ts
getSettings(): Promise<AppSettings>
setSetting(key: SettingKey, value: boolean | number): Promise<AppSettings>
```

- 仅允许 `SettingKey` 三键（04 §7.4）；未知键 → `SETTING_UNKNOWN_KEY`
- 值越界 → `SETTING_INVALID_VALUE`（如 `log.retention_days` 限 1–365，`playtime.checkpoint_sec` 限 10–300）

### 3.10 壳层

```ts
windowControl(action: 'minimize' | 'maximize' | 'close'): Promise<void>
```

- 仅 Tauri 环境有意义；mock 实现为 no-op

---

## 4. 事件（Core → UI）

```ts
'scan:progress': {
  scanId: string;
  phase: 'registry' | 'paths' | 'validate' | 'done';
  scanned: number;
  total: number;
  currentGameId: GameId | null;
}

'game:state-changed': {
  installationId: string;
  status: GameRuntimeStatus;
  updateAvailable: boolean;
  versionUnknown: boolean;
  versionNorm: string | null;
  pid: number | null;
}

'playtime:updated': {
  installationId: string;
  sessionSec: number;     // 当前会话累计
  todaySec: number;       // 该实例今日累计（权威值，UI 直接覆盖，不做加法）
}

'tool:apply-step': {
  toolId: string;
  installationId: string;
  step: ApplyStep;
  state: 'pending' | 'running' | 'completed' | 'failed' | 'skipped';
  detail: {
    // 默认层：语义描述，禁止出现文件名/键名/哈希（03 §5.6 U8）
    default: string;              // e.g. "备份原配置"
    // 高级层：底层事实，仅在「高级详情」展开时展示（00 §12.2）
    advanced: string[] | null;    // e.g. ["LocalStorage.db · KeyCustomFrameRate", "a3f9c2"]
  };
}

'tool:asset-progress': {
  toolId: string;
  phase: 'download' | 'verify' | 'extract';
  receivedBytes: number;
  totalBytes: number | null;   // 无 Content-Length 时为 null（UI 显示不确定进度）
}

'version:refreshed': {
  fetchedAt: number;
  results: RemoteVersionResult[];
}
```

> **`tool:apply-step.detail` 两层结构是 D4/U8 的落地机制**：Core 同时给出「语义层」与「底层事实层」，UI 默认只渲染 `default`，把 `advanced` 收进「高级详情」次行。这样既满足 00 §12.2（普通用户简单、高级用户透明），又避免像 mock 那样把 `LocalStorage.db` / 哈希直接铺在默认层。

---

## 5. 错误码表

错误模型统一为：

```ts
type OrbisError = {
  code: ErrorCode;
  message: string;                    // 开发者可读，仅日志/调试，**不直接展示**
  detail: Record<string, string | number | boolean> | null;  // 结构化上下文
  retryable: boolean;                 // UI 决定是否给「重试」按钮
};
```

**Tauri 侧约定**：命令返回 `Result<T, OrbisError>`，拒绝时前端 `catch` 到 `OrbisError`（形状与上面一致，不要依赖 Tauri 默认的字符串错误）。

| code | 触发场景 | retryable | UI 行为（要点） |
|------|----------|:---------:|----------------|
| `GAME_NOT_FOUND` | installationId / gameId 不存在 | ✗ | 提示并刷新列表 |
| `GAME_ALREADY_RUNNING` | 重复启动 | ✗ | 按钮显示「已在运行」（A4） |
| `GAME_NOT_RUNNING` | 结束时进程已退出 | ✗ | 静默刷新状态（不弹错） |
| `EXECUTABLE_MISSING` | exe 被移动/删除 | ✓ | 引导重新添加；状态置 `broken` |
| `EXECUTABLE_INVALID` | 非 exe / 无法读取 | ✗ | A2 边界提示 |
| `LOOKS_LIKE_LAUNCHER` | 选到官方启动器而非本体 | ✗ | A2 边界：显式提示，不静默接受 |
| `GAME_MISMATCH` | 选的 exe 属另一款游戏 | ✗ | 提示正确游戏名 |
| `INSTALLATION_DUPLICATE` | 同路径重复添加 | ✗ | 去重提示（A2 边界） |
| `PATH_NOT_FOUND` | 路径不存在 | ✓ | 提示 |
| `PATH_PERMISSION_DENIED` | 写权限不足（如装于 Program Files） | ✗ | precheck 阻止 + 引导（04 §7.1 / Q2） |
| `GAME_PROCESS_ACTIVE` | 游戏/启动器运行中 | ✓ | 提示先退出游戏（红线 1） |
| `CONFIG_SOURCE_UNSUPPORTED` | Provider 未声明配置路径 | ✗ | 显示「该游戏暂不支持配置备份」/「不适用」（A8 / 03 §5.4） |
| `COMPAT_BLOCKED` | `incompatible` / `deprecated` 硬阻止 | ✗ | 红色 + 原因 + 行动建议（B6） |
| `COMPAT_UNKNOWN_L3` | L3 工具 `unknown` | ✗ | **硬阻止**（B7 验收），解释原因 |
| `COMPAT_OVERRIDE_REQUIRED` | L1 工具 `unknown` 未带覆盖 | ✗ | 弹「继续使用」确认入口（04 §5.5） |
| `CONSENT_REQUIRED` | L3 未授权 / 文案哈希失效 | ✗ | 拉起 S3 授权流（B7） |
| `BACKUP_NOT_FOUND` | backupId 不存在 | ✗ | 刷新历史列表 |
| `BACKUP_FAILED` | 备份写入失败 | ✓ | **不得进入 Modify**（B2 硬性顺序） |
| `BACKUP_SPACE_INSUFFICIENT` | 剩余空间 < 需求×1.5 | ✓ | 提示清理空间（04 §5.12） |
| `RESTORE_HASH_MISMATCH` | 恢复后哈希校验不一致 | ✗ | 明示未还原成功 + 引导用 pre_restore 备份 |
| `TOOL_NOT_FOUND` | toolId 不存在 | ✗ | 刷新工具列表 |
| `TOOL_ASSET_NOT_CONFIGURED` | `assets.json` 未配置 URL（Q1 未定稿） | ✗ | 显示「组件构建管道待定稿」，**不显示为错误** |
| `TOOL_ASSET_DOWNLOAD_FAILED` | 下载失败/中断 | ✓ | 可重试；失败不阻塞游戏本体 |
| `TOOL_ASSET_HASH_MISMATCH` | 哈希不符 | ✗ | 拒绝加载（防篡改，04 §5.8） |
| `NETWORK_UNREACHABLE` | 版本源不可达 | ✓ | 状态置 `unknown`，**不误报**（E1） |
| `VERSION_SOURCE_UNSUPPORTED` | 厂商无公开 manifest（终末地） | ✗ | 降级路径而非错误：`version: null` + 官方渠道引导，标「调研中」（P0-B §3.2） |
| `SETTING_UNKNOWN_KEY` | 非白名单键 | ✗ | 开发期错误 |
| `SETTING_INVALID_VALUE` | 值越界 | ✗ | 开发期错误 |
| `NOT_IMPLEMENTED` | 后端尚未实现（过渡期） | ✗ | 开发期占位，**发布前必须为 0 处** |
| `INTERNAL` | 未归类异常 | ✗ | 提示 + 引导导出日志（D6 P1 前为「打开日志目录」） |

---

## 6. DTO 定义

```ts
// ── 游戏目录（编译期静态，00 §7.10 Game 实体不入库）────────
type GameCatalogEntry = {
  id: GameId;
  name: string;                 // 中文名
  enName: string;
  publisher: string;
  /** 该游戏已知的区服（A1：同款多区服视为多个 GameInstallation） */
  regions: Region[];
  /** 空状态引导用的官方入口（A1 边界）；URL 正确性由 T8 核实 */
  officialUrl: string | null;
  /** L3 工具存在时，空状态需提示「需额外组件」（B7 发行包分离对用户可见） */
  hasBundledComponent: boolean;
};

// ── 安装实例 ────────────────────────────────────────────
type InstallationDto = {
  id: string;
  gameId: GameId;
  region: Region;
  installPath: string;
  executablePath: string;
  localVersion: string | null;        // 原始识别值；null = Unknown（02 A3）
  versionNorm: string | null;         // major.minor；查询/比较只用它
  versionSource: 'exe_versioninfo' | 'feature_file' | 'directory' | null;
  status: GameRuntimeStatus;
  updateAvailable: boolean;           // 会话内派生（远程版本不落库）
  versionUnknown: boolean;            // = versionNorm === null
  needsAttention: boolean;            // §2.1 判定式
  attentionReasons: AttentionReason[];
  addedVia: 'scan' | 'manual';
  pid: number | null;                 // 仅 running 时非空
  playtime: { todaySec: number; weekSec: number; totalSec: number };
  hasConfigSource: boolean;           // Provider 是否声明配置路径
  configUnsupportedReason: 'provider_not_declared' | 'not_applicable' | null;
  createdAt: number;
  updatedAt: number;
};

type AttentionReason =
  | 'update_available' | 'broken' | 'version_unknown'
  | 'tool_unknown' | 'tool_incompatible';

type InstallationListResult = {
  installations: InstallationDto[];
  summary: {
    total: number;
    needsAttention: number;
    updateAvailable: number;
    broken: number;
    toolAttention: number;
  };
};

type InstallationDetail = InstallationDto & {
  tools: ToolDto[];
  latestBackup: BackupSummary | null;
  launchProfile: LaunchProfileDto;
};

type ScanResult = {
  scanId: string;
  found: number;
  installations: InstallationDto[];
  partial: boolean;                   // 超时/权限不足 → true（不阻塞，02 A1）
  warnings: string[];
};

type ExecutableValidation = {
  ok: boolean;
  detectedGameId: GameId | null;
  reason: 'ok' | 'not_an_executable' | 'looks_like_launcher'
        | 'game_mismatch' | 'unreadable';
  versionRaw: string | null;
};

type RuntimeStateDto = {
  installationId: string;
  status: GameRuntimeStatus;
  pid: number | null;
  updateAvailable: boolean;
  versionUnknown: boolean;
};

// ── 启动 ───────────────────────────────────────────────
type LaunchResult = {
  installationId: string;
  pid: number;
  unlock: {
    requested: boolean;               // 是否走了解锁编排
    attached: boolean;                // 是否成功挂载
    skipped: boolean;                 // B8「本次不解锁」
    degradedReason: string | null;    // 非空 → UI 明示「本次未解锁」（不阻塞）
  };
};

type LaunchProfileDto = {
  installationId: string;
  args: string;
  updatedAt: number;
};

// ── 时长 ───────────────────────────────────────────────
type PlaytimeResult = {
  scope: 'today' | 'week' | 'total';
  totalSec: number;
  perInstallation: { installationId: string; gameId: GameId; seconds: number }[];
};

// ── 版本与更新 ─────────────────────────────────────────
type RemoteVersionResult = {
  gameId: GameId;
  region: Region;
  version: string | null;             // 归一化后 major.minor；null = 不可达/降级
  source: VersionSourceId;
  fetchedAt: number;
  error: OrbisError | null;           // 降级信息，非异常路径
};

type UpdateStatusDto = {
  installationId: string;
  gameId: GameId;
  localVersionNorm: string | null;
  remoteVersion: string | null;
  status: 'up_to_date' | 'update_available' | 'unknown';
  checkedAt: number | null;
  source: VersionSourceId | null;
};

// ── 工具 ───────────────────────────────────────────────
type ToolDto = {
  id: string;                         // namespace/name
  gameId: GameId;
  name: string;
  description: string;                // 面向用户的一句话（来自 Manifest）
  version: string;
  type: ToolType;
  riskLevel: RiskLevel;
  permissions: ToolPermission[];
  requiresAdmin: boolean;
  backupRequired: boolean;
  enabled: boolean;
  compat: CompatibilityDto;           // 实时查询，非 Manifest 静态值
  asset: ToolAssetDto | null;         // type = external_process 时非空
  source: ToolSource;
  pendingVerifications: string[];     // 如 ["T4a","T4b"] → UI 显示「部分参数待实测」
};

type ToolDetail = ToolDto & {
  consentText: string | null;         // L3 版本化风险文案（原文，UI 全屏呈现）
  consentTextHash: string | null;     // 与 grantToolAuthorization 入参一致
  permissionsDetail: { permission: ToolPermission; label: string; detail: string }[];
};

type ToolEnableResult = {
  toolId: string;
  enabled: boolean;
  outcome: 'applied' | 'rolled_back' | 'blocked' | 'unchanged';
  compat: CompatibilityDto;
  backupId: string | null;            // 成功时为 pre_modify 备份 ID
  rollbackBackupId: string | null;    // 回滚时使用/生成的备份 ID
  blockedReason: OrbisError | null;
};

type ToolAssetDto = {
  toolId: string;
  assetKey: string;
  state: 'not_required' | 'missing' | 'ready' | 'hash_mismatch';
  version: string | null;
  sha256: string | null;              // 已安装资产的实测哈希
  expectedSha256: string | null;      // 来自 assets.json
  installedPath: string | null;
  downloadUrlConfigured: boolean;     // false → UI 显示「构建管道待定稿」（Q1）
};

type ToolSource = {
  kind: 'builtin' | 'bundled' | 'upstream';
  repo: string | null;
  license: string | null;
  version: string | null;
};

// ── 兼容性 ─────────────────────────────────────────────
type CompatibilityDto = {
  gameId: GameId;
  toolId: string;
  status: ToolCompatStatus;
  matchKind: 'exact' | 'prefix' | 'none';   // none → status 必为 unknown
  matchedVersionKey: string | null;        // 如 "3.x"（prefix）
  verifiedAt: string | null;               // YYYY-MM-DD（seed 原文，非 epoch）
  verifiedBy: string | null;
  notes: string | null;                    // seed 的 notes，供「高级详情」
  seedSchemaVersion: string | null;        // null → seed 缺失/损坏
};

// ── 备份 ───────────────────────────────────────────────
type BackupSummary = {
  id: string;
  installationId: string;
  gameId: GameId;
  toolId: string | null;              // null = 手动备份
  trigger: BackupTrigger;
  fileCount: number;
  totalBytes: number;
  primaryFile: string | null;         // 主文件相对路径，供「高级详情」单行展示
  createdAt: number;
};

type RestoreResult = {
  backupId: string;
  restoredFileCount: number;
  verified: boolean;                  // 逐文件哈希校验通过
  preRestoreBackupId: string;         // 自动生成的 pre_restore 备份（可撤销恢复本身）
};

type BackupStorageInfo = {
  installationId: string;
  backupCount: number;
  totalBytes: number;
  freeDiskBytes: number;
  estimatedNextSizeBytes: number | null;   // 源目录当前大小；null = 无法估算
};

// ── 设置 ───────────────────────────────────────────────
type AppSettings = {
  'version_check.enabled': boolean;
  'log.retention_days': number;
  'playtime.checkpoint_sec': number;
};
```

---

## 7. 前端接入约定

### 7.1 分层（04 §4.1）

```
src/views/**            ← 只消费 store
src/store/**            ← 只调用 api，不直接 invoke
src/api/contract.ts     ← OrbisApi 接口（UI 唯一依赖点）
src/api/mock.ts         ← 开发期实现（现有 mock 收敛到这里）
src/api/tauri.ts        ← invoke 实现（未实现命令抛 NOT_IMPLEMENTED）
src/api/index.ts        ← export const api: OrbisApi = isTauri() ? tauri : mock
```

- **禁止**在 `views/` 或 `store/` 里出现 `invoke(...)`、路径拼接、SQL、哈希运算（00 §7.9）
- 过渡期 `src/api/types.ts` 手写 DTO；接入 `tauri-specta` 后由 `bindings.gen.ts` 取代（04 §2.1 / Q3），`contract.ts` 的方法签名保持不变

### 7.2 mock 的行为要求

mock 不是「随便返回假数据」，而是**行为对齐的参照实现**，用于在无 Rust 时验证 UI：

- `setToolEnabled` 的 mock 必须**逐步发** `tool:apply-step`（gate → precheck → backup → modify → verify），且提供失败 → `rollback` 的注入开关（对齐 04 §5.5 状态机）
- mock 数据中的版本号、`toolId`、`compat.status` **必须与 `data/compatibility/seed.json` 口径一致**（如鸣潮 `3.x` → `unknown`，原神 `7.x` → `compatible`）
- mock 不得引入任何契约外的字段（如虚构的「节点数」「差分占用」）

### 7.3 文案责任划分

| 内容 | 来源 |
|------|------|
| 工具名称 / 描述 | Manifest（`name` / `description`） |
| 兼容状态文案与行动建议 | UI 按 `CompatibilityDto.status` 映射（03 §2.2） |
| 风险等级标签「L1 配置修改」「L3 进程级」 | UI 按 `riskLevel` 映射 |
| 错误提示与引导 | UI 按 `OrbisError.code` 映射（§5） |
| L3 授权全文 | Manifest/资源内的版本化文案，经 `consentText` 下发 |

---

## 附录：变更记录

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.0 | 2026-09-18 | 首版冻结：通用约定（时间/时长/路径/ID/版本/null 语义/文案责任）、枚举口径（含 Region 小写约束与「需处理」判定式）、27 条命令 v1 定稿（补齐 A7 / 设置 / 资产 / override / scope / validateExecutable / deleteBackup / storageInfo / revokeAuthorization）、6 个事件（含 `tool:apply-step` 的两层 detail 结构与 U8 落地机制）、30 项错误码表（含 retryable 与 UI 行为）、完整 DTO 定义、前端接入分层约定与 mock 行为要求 |
| v1.1 | 2026-09-18 | 增补 `listGames()` 与 `GameCatalogEntry`（共 28 条命令）。理由：写视图时发现 UI 无法取得游戏显示名与官方入口——缺此命令则前端只能自行硬编码一份游戏目录，等于把 catalog 复制到 UI 层，违反 00 §7.9 与 00 §12.3 规则 1（Game Core 不硬编码游戏逻辑，反之 UI 也不应自带目录）。同时为 A1 空状态（02 要求「展示各游戏官方下载入口」）提供数据来源。新增 T8 待实测项核实 5 条官方入口 URL |
