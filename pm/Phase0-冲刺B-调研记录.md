---
title: Phase 0 冲刺 B · E1 版本源探测路径核实记录
doc_id: P0-B
type: phase0-research
status: accepted
version: v0.2
date: 2026-09-18
upstream: 00-产品基石.md v0.5（§10.1 入场条件 #3 / #4，§13 Q3）；01-MVP功能清单 E1 / A8 / B1；02-MVP-PRD E1
tags:
  - phase0
  - research
  - adr
  - version-source
---

# Phase 0 冲刺 B · E1 版本源核实 + 鸣潮 Provider 声明

> 对应 00 基石 §10.1 入场条件 **#3（E1 版本源拍板 / Q3 最小路径）** 与 **#4（5 款游戏 Provider 配置声明清单）**。
> 性质：ADR 风格决策记录；**04 技术设计 VersionProvider / ConfigProvider 接口的直接输入**。

---

## 0. 判定标准

一条探测路径「过关」须同时满足：

1. **只读**：单次 HTTP GET，无鉴权或仅公共参数（launcher_id / appKey 等）
2. **结构化**：返回 JSON，含可解析的当前版本号字段
3. **多源交叉**：≥ 2 个独立社区项目 / 文档验证过该端点（非孤证）
4. **低频可容忍**：为官方启动器自身使用的端点，客户端低频轮询不构成异常流量

---

## 1. 米哈游系（原神 / 星铁 / 绝区零）—— HoYoPlay 统一 API

### 1.1 核心端点（国际服，已验证）

| 项 | 值 |
|----|-----|
| Base | `https://sg-hyp-api.hoyoverse.com/hyp/hyp-connect/api/` |
| 公共参数 | `launcher_id=VYTpXlbWo8&language=en-us` |
| 版本端点 | `getGamePackages`（`game_id` 参数实际被忽略，一次返回全部游戏） |
| 版本字段 | `data.game_packages[].main.major.version` |
| 附加字段 | `game_pkgs[]`（安装包 + md5）、`patches[]`（增量补丁）、`pre_download`（预下载） |
| 辅助端点 | `getGameConfigs`：`exe_file_name` / `installation_dir` / `game_screenshot_dir`（**对 A1 自动发现同样有用**） |

已知 game_id（biz 缩写印证）：

| 游戏 | game_id | biz |
|------|---------|-----|
| 原神 | `gopR6Cufr3` | hk4e_global |
| 星铁 | `4ziysqXOQ8` | hkrpg_global |
| 绝区零 | `U5hbdsT9W7` | nap_global |

### 1.2 国服路径（待实测确认）

- 国服 HYP base 推测为 `hyp-api.mihoyo.com`（**未直接确认，04 阶段实测**）
- 老版 mdk API 有 UIGF 社区文档背书（国服）：
  - 原神：`https://sdk-static.mihoyo.com/hk4e_cn/mdk/launcher/api/content`（版本资源在 `/resource` 端点）
  - 星铁：`https://api-launcher.mihoyo.com/hkrpg_cn/mdk/launcher/api/content`
  - 参数：`key` / `launcher_id` / `language`
- 连通性检查端点（启动器自检用）：`hk4e-sdk.mihoyo.com/ping` / `hkrpg-sdk.mihoyo.com/ping` / `nap-sdk.mihoyo.com/ping`
- 星铁启动器自更新 API：`api-static.mihoyo.com/takumi/ptolemaios_api/api/getLatestRelease`

### 1.3 判定

**过关**。一个 adapter 覆盖 3 款游戏：一次 `getGamePackages` 调用 + 客户端按 game_id 过滤。md5 / patches / pre_download 字段为后续版本（下载引擎、预下载提示）预留，MVP E1 只读 `version` 字段即可。

---

## 2. 鸣潮（库洛）—— 官方 CDN index.json

### 2.1 核心端点（国际服 osLive，已验证）

| 项 | 值 |
|----|-----|
| 游戏索引 | `https://prod-alicdn-gamestarter.kurogame.com/launcher/game/G153/50004_obOHXFrFanqsaIEOmuKroCcbZkQRBC7c/index.json` |
| 版本字段 | `default.config.version`（含配套 hash） |
| 启动器配置 | `.../launcher/launcher/50004_obOHXFrFanqsaIEOmuKroCcbZkQRBC7c/index.json`（稳定端点） |
| URL 模式 | `<CDN_BASE>/<appId>_<appKey>/<gameId>/...` |

常量：`appId=50004`、`appKey=obOHXFrFanqsaIEOmuKroCcbZkQRBC7c`、`gameId=G153`。

### 2.2 证据与佐证

- DynamiByte gist（2026-03，4 次修订）端点记录
- Wuwa-Web-Request 项目独立使用同类端点
- appKey 可从本地客户端自校验：`KRApp.conf`（`C:\Program Files\Wuthering Waves\<version>\Assets\`）经 `base64(XOR(data, 0x63))` 解密——**端点漂移时有兜底恢复手段**
- 本地发现参考：wuwatracker `import.ps1` 的注册表 / MuiCache 扫描逻辑（A1 复用）

### 2.3 判定

**过关**。单一稳定 index.json，含版本 + hash；appKey 有本地解密兜底，端点失效可恢复。国服 / 国际服索引差异待 04 阶段实测（常量结构一致，预期仅 appKey / 路径不同）。

---

## 3. 终末地（鹰角）—— GRYPHLINK 现状

### 3.1 已核实事实

| 项 | 值 |
|----|-----|
| 启动器分发 | `https://launcher.gryphline.com/launcher/get_latest_launcher?appcode=TiaytKBUIEdoEwRT&ta=endfield&channel=6&sub_channel=6`（Lutris 安装脚本佐证） |
| 安装路径 | `C:\Program Files\GRYPHLINK\Launcher.exe` |
| 游戏日志 | `%USERPROFILE%\AppData\LocalLow\Gryphline\Endfield\sdklogs\HGWebview.log`（含 webview 域名：`ef-webview.gryphline.com` / `ef.webview.hypergryph.com`） |
| 反作弊 | ACE（腾讯系） |
| 更新模型 | 大版本更新需**全量重下客户端**（无增量补丁体系） |
| 社区逆向 | **无公开的 CDN manifest API 文档**（2026-09 检索时点；游戏 2026-01 全球公测，生态尚新） |

### 3.2 判定

**不过关 → 走降级方案**。MVP 中终末地 E1 降级为：本地版本展示 + 官方渠道引导（GRYPHLINK 内更新），状态标注「调研中」。

未来线索（不进 MVP）：`HGWebview.log` 的 webview 域名指向启动器数据接口，可从 Launcher 本地缓存 / 日志逆向出 manifest 端点——等社区出现公开逆向或 04 阶段自行抓包后再升级为直连。

---

## 4. E1 拍板建议（汇总）

| 游戏 | 厂商 | 探测路径 | 判定 | MVP 行为 |
|------|------|----------|------|----------|
| 原神 | 米哈游 | HYP `getGamePackages`（国服 base 待实测，mdk 有文档） | ✓ 过关 | 只读版本检测 |
| 星铁 | 米哈游 | 同上（`hkrpg_global` / `hkrpg_cn`） | ✓ 过关 | 只读版本检测 |
| 绝区零 | 米哈游 | 同上（`nap_global` / 国服待实测） | ✓ 过关 | 只读版本检测 |
| 鸣潮 | 库洛 | CDN `index.json`（`default.config.version`） | ✓ 过关 | 只读版本检测 |
| 终末地 | 鹰角 | 无公开 manifest | △ 降级 | 本地版本展示 + 官方渠道引导，标「调研中」 |

> **结论**：4/5 直连过关 + 1 降级，**入场条件 #3 满足**（降级本身是 00 §10.1 预设的合规路径，非阻塞项）。

---

## 5. 对 04 技术设计的直接输入

1. **VersionProvider 接口**：`getRemoteVersion(gameId, region) -> Version`，region 维度（Q7）从第一天进入接口签名（米哈游系国服 / 国际服 base 不同）
2. **三个 adapter**：`HoyoPlayAdapter`（1 对 3，一次调用 + 过滤）、`KuroAdapter`（鸣潮）、`DegradedAdapter`（终末地，终态 Unknown + 引导）
3. **鸣潮 appKey 兜底**：KRApp.conf 解密逻辑作为端点漂移的恢复手段写入技术设计
4. **频率与合规**：探测为官方启动器同源端点，须低频（如启动时 + 手动刷新）、只读、用户可关；不做预下载 / 下载之外的数据消费
5. **待实测清单**：米哈游国服 HYP base、鸣潮国服索引、`getGamePackages` 国服 game_id——04 阶段首批工程验证项

---

## 6. 鸣潮 LocalStorage.db · Provider 声明调研（入场条件 #4）

### 6.1 文件与格式事实（多源一致）

| 项 | 值 |
|----|-----|
| 路径 | `<游戏根目录>\Wuthering Waves Game\Client\Saved\LocalStorage\LocalStorage.db` |
| 格式 | SQLite，单表 `LocalStorage(key, value)`，key-value 存储 |
| 游戏根目录发现 | `%APPDATA%\KRLauncher\*\*\kr_starter_game.json` 的 `path` 字段（MikuAuahDark 脚本）——A1 发现通道之一 |
| 同目录伴生文件 | SQLite journal / 游戏生成的 db2 副本——**备份范围必须覆盖整目录，不能只拷 db** |

### 6.2 FPS 相关键（双键，社区共识两处同改）

| 键 | 结构 | 说明 |
|----|------|------|
| `CustomFrameRate` | 独立键，整数 | 1.0 时代存枚举索引（0=30 / 1=45 / 2=60 / 3=120）；2.2 起社区改为**直接写 120**——值语义随版本漂移，F2 种子表的活案例 |
| `GameQualitySetting` | JSON 值 | 内含 `KeyCustomFrameRate` 字段（与 00 §12.2 Advanced 展示示例一致）；游戏内改画质以此回写 |

写入语义（幂等 upsert，MikuAuahDark）：

```sql
INSERT INTO LocalStorage VALUES ('CustomFrameRate', 120)
ON CONFLICT(key) DO UPDATE SET value = 120;
```

`GameQualitySetting` 走「读 → JSON 反序列化 → 改 `KeyCustomFrameRate` → 写回」（FastChen 工具 `Microsoft.Data.Sqlite` 实现印证；Orbis 用 Rust `rusqlite` 等价）。

### 6.3 游戏行为约束（工程红线，04 直接输入）

1. **运行时不可写**：必须关闭游戏 + 启动器后修改（文件锁 + 游戏退出时回写覆盖）——写入前置进程检查
2. **游戏内改画质会锁回 60**：修改帧率后在游戏内调整任何画质项即被重置——须在 B6 提示文案中明示
3. **上限 120**：写更高值触发游戏修复机制（回 60 或不生效）——UI 锁定 120 封顶
4. **2.2+ 回写对抗**：游戏启动时生成 db2 副本 / 重写 db，老式直改会失效（MikuAuahDark 标注 BROKEN IN 2.2）；社区现行方案 = SQL UPDATE + 重置 `MenuData` / `PlayMenuInfo` + SQLite TRIGGER 防回写（yuspring gist，2025-12 活跃）
5. **验证时效**：网页编辑器 wuwa.kyou.dev 标注 verified 2.7；3.x 当前版本待实测——**印证 B6「游戏更新后未验证状态显式提示」的必要性**，鸣潮将是 F2 种子表首条数据

### 6.4 风险证据（L1 口径）

- WakuWakuPadoru（活跃维护至 2.3+）：「不修改游戏文件与资产，仅改用户设置，100% 安全不会封号」
- 教程生态自 2024-05 持续演进（中 / 英 / 法 / 葡多语种），无「仅因改帧率」封号案例
- 技术上属 ToS 灰色（igamesnews 提示）——披露文案与 L3 统一口径：「社区多年实践、非官方授权」

### 6.5 MVP Provider 声明清单（入场条件 #4 交付物）

| 游戏 | 配置目标 | 声明状态 |
|------|----------|----------|
| 鸣潮 | `LocalStorage.db`（`CustomFrameRate` + `GameQualitySetting.KeyCustomFrameRate`） | **已声明**（MVP 交付 B1/B2/B5） |
| 鸣潮 | `Client\Saved\Config\WindowsNoEditor\` 下 `GameUserSettings.ini` / `Engine.ini` | 调研中（P2+；UE 标准 ini，社区用于全屏 / 纹理流送优化，暂不进 MVP） |
| 原神 | —（L3 运行时解锁不落盘） | 不适用（无配置文件修改，A8 显示「暂不支持」） |
| 星铁 / 绝区零 / 终末地 | — | 暂不支持（Provider 声明前 UI 显示「暂不支持」，随 04 逐个解锁） |

> **判定**：#4 满足——MVP 门槛「至少鸣潮交付」已具备完整键级声明 + 工程红线；其余游戏状态齐备（调研中 / 暂不支持 / 不适用）。

---

## 7. 文档回写清单（拍板后执行）

| 目标 | 动作 |
|------|------|
| 00 基石 | §13 Q3 标注「已收敛：4/5 直连 + 终末地降级」；§10.1 #3 / #4 状态更新 |
| 01 清单 | E1 行补「版本源已核实（P0-B 记录）」；终末地 E1 标注降级行为；A8 / B1 补「鸣潮 Provider 已声明（P0-B §6）」 |
| 02 PRD | E1 验收标准补引用本记录 §4 判定表；B1/B2 补备份范围 = LocalStorage 整目录（P0-B §6.3） |
| 04 技术设计（未来） | §5 全部输入 + 待实测清单；ConfigProvider 接口（键级声明 / upsert 写入 / 进程前置检查 / 整目录快照备份）；鸣潮 3.x 键语义实测 + F2 种子表首条 |

---

## 8. Phase 0 入场条件进度

| # | 入场条件 | 状态（终态，2026-09-18） |
|---|----------|------|
| 1 | F4 原神解锁上游核实 | 完成（冲刺 A） |
| 2 | F2 兼容性种子表 v0 | **已交付**（P0-C） |
| 3 | E1 版本源拍板 | **已拍板**（本记录 §4：4/5 直连 + 终末地降级） |
| 4 | 5 款游戏 Provider 配置声明清单 | **已拍板**（本记录 §6.5：鸣潮键级声明） |
| 5 | 项目 License 选择 | **已拍板**（P0-A §2：GPL-3.0-or-later） |

---

## 附录：调研来源

- DynamiByte《HoYoPlay (HYP) API》gist（2026-03，4 revisions）：sg-hyp-api 端点族、getGamePackages / getGameConfigs 结构
- DynamiByte gist（鸣潮部分）：kurogame CDN index.json、appId/appKey/gameId 常量、KRApp.conf 解密
- UIGF《米哈游游戏启动器 API》社区文档：国服 mdk content / resource 端点、SDK ping、ptolemaios getLatestRelease
- Wuwa-Web-Request（GitHub）：鸣潮端点独立使用佐证
- wuwatracker（import.ps1）：鸣潮注册表 / MuiCache 本地发现逻辑
- Lutris 安装脚本（终末地）：GRYPHLINK get_latest_launcher 分发 URL
- 米哈游启动器预下载公告（2026-08 星铁 4.5）：版本节奏旁证
- MikuAuahDark《wuthering_120fps.py》gist（public domain，2 revisions）：kr_starter_game.json 发现路径、upsert 写入语义、2.2 失效标注
- WakuWakuPadoru/WuWa_Simple_FPSUnlocker（GitHub，活跃维护）：L1 风险口径（仅改设置不碰游戏文件）、2.2/2.3 持续适配
- FastChen《鸣潮自定义帧率解锁工具》博文 + Wuthering-Waves-Tool（开源）：Microsoft.Data.Sqlite 实现、GameQualitySetting JSON 修改路径、120 上限修复机制
- yuspring《wuwa_unlock120.md》gist（2025-12，5 revisions）：SQL UPDATE + MenuData/PlayMenuInfo 重置 + SQLite TRIGGER 防回写
- wuwa.kyou.dev（wuwaconf 网页编辑器）：verified 2.7、原文件备份交互范式
- NGA 鸣潮版（2024-05 / 2025-03 双帖）：中文社区双键同改共识、2.2 后 db2 只读流程、RTSS 限帧配平实践
- Bear_lele / igamesnews / fastchen 博客教程（2024-05，中/法/葡）：DB Browser 手动路径、游戏内改画质锁回 60、ToS 灰色提示
- windowsnoticias（2026-08）：GameUserSettings.ini / Engine.ini 隐藏配置项（Provider P2+ 线索）

## 变更记录

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1 | 2026-09-18 | 初版：米哈游系 / 鸣潮直连路径核实过关、终末地确认无公开 manifest 走降级、E1 拍板建议 4/5 + 1 降级 |
| v0.2 | 2026-09-18 | 追加鸣潮 LocalStorage.db Provider 声明调研（键级声明 / 工程红线 / 声明清单），入场条件 #4 建议已备 |
