---
title: Phase 0 冲刺 C · F2 兼容性种子表 v0 定稿
doc_id: P0-C
type: phase0-research
status: accepted
version: v0.2
date: 2026-09-18
upstream: 00-产品基石.md v0.5（§10.1 入场条件 #2，§7.10 ToolCompatibility，§11 开放数据集，§13 Q6）；01-MVP功能清单 F2 / C4 / B6
tags:
  - phase0
  - research
  - adr
  - compatibility
---

# Phase 0 冲刺 C · F2 兼容性种子表 v0

> 对应 00 基石 §10.1 入场条件 **#2（Compatibility 种子表 v0）**：鸣潮 + 原神「版本 × 工具状态」第一份数据，明确 owner 与交付日期。
> 性质：Schema 定稿 + 首批数据；**C4 兼容性数据与 B6 提示逻辑的直接输入**，也是 00 §11 定位的开放数据资产首块砖。

---

## 0. 交付声明（01 F2 要求项）

| 项 | 值 |
|----|-----|
| Owner | 项目维护者（MVP 阶段单人维护；社区共建流程 Phase 1+ 设计，见 §4.3） |
| 交付日期 | 2026-09-18（本记录即 v0 交付物，数据内嵌 §3） |
| 覆盖范围 | 鸣潮（B1 内置 120 FPS，L1）+ 原神（B7 解锁组件，L3） |
| Schema 版本 | 0.1.0 |

---

## 1. Schema 定稿

### 1.1 数据文件格式（JSON）

```json
{
  "schema_version": "0.1.0",
  "updated_at": "2026-09-18",
  "maintainer": "orbis-maintainer",
  "entries": [
    {
      "game_id": "wuthering-waves",
      "tool_id": "orbis-builtin/wuwa-fps-120",
      "risk_level": "L1",
      "compatibility": [
        {
          "game_version": "2.7",
          "status": "Verified",
          "verified_at": "2026-09-18",
          "verified_by": "P0-B §6.3",
          "evidence": ["wuwa.kyou.dev（verified 2.7）", "yuspring gist（2025-12，SQL 方法）"],
          "notes": "CustomFrameRate 直接写数值（2.2+ 语义）；与 GameQualitySetting.KeyCustomFrameRate 双键同改"
        },
        {
          "game_version": "3.x",
          "version_match": "prefix",
          "status": "Unknown",
          "notes": "3.x 未实测；方法论自 2.2 起稳定，待首测后升级状态"
        }
      ]
    }
  ]
}
```

### 1.2 字段语义（对齐 00 §7.10 ToolCompatibility 实体）

| 字段 | 类型 | 语义 |
|------|------|------|
| `game_id` | slug | 游戏标识：`genshin-impact` / `honkai-star-rail` / `zenless-zone-zero` / `wuthering-waves` / `arknights-endfield` |
| `tool_id` | namespace/name | `orbis-builtin/*`（内置工具）/ 未来 `community/*`（第三方） |
| `risk_level` | L1 / L2 / L3 | 沿用 00 风险分级（展示用，权威值在 Tool Manifest） |
| `game_version` | string | 目标游戏版本 |
| `version_match` | `exact`（默认）/ `prefix` | `prefix` + `"3.x"` 匹配 3.0–3.9；缺省为精确匹配 |
| `status` | 五态 | 见 §2 |
| `verified_at` / `verified_by` | 日期 / 来源 | 证据时点与出处（P0 记录锚点 / 社区链接 / `community/*` 贡献者） |
| `evidence` | string[] | 证据链接或描述（可校验性，呼应 00 §11「可导出、可校验」） |
| `notes` | string | 工程备注（键名、写入语义、失效原因等） |

### 1.3 三条设计决策

1. **`prefix` 匹配而非版本区间**：`"3.x"` 一条覆盖整个小版本代，避免每个小版本手工补条目；Unknown 兜底规则（§2.2）保证无条目版本安全降级
2. **Verified ≠ Compatible**：Verified = 该版本有直接验证证据；Compatible = 基于原理 / 上游声明推断可用——证据强度分级，不虚报置信度
3. **历史版本不铺条目**：MVP 只需「当前版本判定 + Unknown 兜底」；历史知识（如鸣潮 1.0 枚举索引语义）沉淀在 `notes`，不占条目

---

## 2. 状态机与查询语义（B6 提示逻辑依据）

### 2.1 五态定义（沿用 00 §6.x，补 Verified/Compatible 边界）

| 状态 | 定义 | B6 行为 |
|------|------|---------|
| Verified | 指定版本有直接验证证据 | ✓ 青色对勾，正常启用 |
| Compatible | 原理/上游声明推断可用，未实测 | ✓ 对勾 + 「推断兼容」小字 |
| Unknown | 无条目或未验证 | ⚠ 粉紫问号 + 显式提示，允许「继续使用」（00 §4.4） |
| Incompatible | 已知失效 | ✗ 红色，**阻止启用** + 原因 |
| Deprecated | 工具被替代 | 横线 + 迁移指引 |

### 2.2 查询规则（优先级自上而下）

1. `exact` 匹配当前游戏版本 → 命中即返回该状态
2. `prefix` 匹配 → 命中即返回该状态
3. **无命中 → Unknown（默认安全态）**——游戏版本更新即自动落入，正是 00 §4.4 场景的触发机制

### 2.3 状态升级路径

`Unknown →（实测通过）→ Verified`；`Verified →（游戏更新后失效报告）→ Incompatible`。MVP 阶段由维护者手工更新种子表文件，随发行包分发。

---

## 3. 首批种子数据（v0 全文）

```json
{
  "schema_version": "0.1.0",
  "updated_at": "2026-09-18",
  "maintainer": "orbis-maintainer",
  "entries": [
    {
      "game_id": "wuthering-waves",
      "tool_id": "orbis-builtin/wuwa-fps-120",
      "risk_level": "L1",
      "compatibility": [
        {
          "game_version": "2.7",
          "status": "Verified",
          "verified_at": "2026-09-18",
          "verified_by": "P0-B §6.3",
          "evidence": ["wuwa.kyou.dev（verified 2.7）", "yuspring gist（2025-12，SQL 方法）"],
          "notes": "CustomFrameRate 直接写数值（2.2+ 语义）；双键同改；上限 120（更高触发游戏修复机制）"
        },
        {
          "game_version": "3.x",
          "version_match": "prefix",
          "status": "Unknown",
          "notes": "3.x 未实测；方法论自 2.2 起稳定，待首测后升级"
        }
      ]
    },
    {
      "game_id": "genshin-impact",
      "tool_id": "orbis-bundled/genshin-fps-unlock",
      "risk_level": "L3",
      "compatibility": [
        {
          "game_version": "7.0",
          "status": "Compatible",
          "verified_at": "2026-09-18",
          "verified_by": "P0-A §1.1",
          "evidence": ["上游 34736384/genshin-fps-unlock v3.0.4 README：理论上支持后续版本"],
          "notes": "WriteProcessMemory 特征搜索原理跨版本；发行包分离，组件版本随上游 release；升级为 Verified 需本机实测"
        },
        {
          "game_version": "7.x",
          "version_match": "prefix",
          "status": "Compatible",
          "evidence": ["上游 README「理论上支持后续版本，需要时会尽快更新」（P0-A §1.1）"],
          "notes": "live 7.1+ 兜底层（2026-09-18 拍板）；上游声明失效或社区失效报告时降级并补 exact 条目"
        }
      ]
    }
  ]
}
```

### 3.1 条目依据

| 条目 | 状态 | 依据 |
|------|------|------|
| 鸣潮 2.7 × 120FPS | Verified | 网页编辑器标注 verified 2.7 + 2025-12 活跃 SQL 方案（P0-B §6.3） |
| 鸣潮 3.x × 120FPS | Unknown | 当前 3.5 无直接验证证据——**MVP 首个 Unknown 展示案例，恰好验证 B6 设计** |
| 原神 7.0 × FPS 解锁 | Compatible | 上游「理论支持后续版本」声明 + 原理无关版本偏移；非逐版本实测 |
| 原神 7.x × FPS 解锁 | Compatible（prefix 兜底） | 同上声明的 prefix 层；避免 live 7.1+ 常态 Unknown 拦死 B7（2026-09-18 拍板） |

---

## 4. 存储分发与演进

### 4.1 仓库路径（04 技术设计确认）

建议 `data/compatibility/seed.json` 随 repo 版本化；运行时只读加载，缓存于本地。

### 4.2 分发与校验

- MVP：**随发行包内置**，无在线更新（对齐 00「MVP 无遥测、本地优先」）
- 未来：在线增量更新（开源静态托管即可），文件级 SHA256 校验
- 社区贡献走 PR + `verified_by` 署名——Q6「Compatibility 数据贡献条数」的计数口径即本文件 entries 数

### 4.3 演进路线

v0（本记录，维护者手工）→ v1（04 定稿加载器 + CI 校验 schema）→ v2（在线更新 + 贡献流程）。

---

## 5. 文档回写清单（拍板后执行）

| 目标 | 动作 |
|------|------|
| 00 基石 | §10.1 #2 状态更新；§13 Q6 补「贡献条数 = 种子表 entries 数」口径 |
| 01 清单 | F2 标注「v0 已交付（P0-C）」；C4 补 schema 引用 |
| 02 PRD | B6 验收标准补五态行为表引用（本记录 §2.1） |
| 04 技术设计（未来） | 加载器 / 匹配规则（§2.2）/ 仓库路径（§4.1） |

---

## 6. Phase 0 入场条件进度（终版）

| # | 入场条件 | 状态（终态，2026-09-18） |
|---|----------|------|
| 1 | F4 原神解锁上游核实 | 完成（冲刺 A） |
| 2 | F2 兼容性种子表 v0 | **已拍板交付**（本记录） |
| 3 | E1 版本源拍板 | **已拍板**（P0-B §4） |
| 4 | 5 款游戏 Provider 配置声明清单 | **已拍板**（P0-B §6.5） |
| 5 | 项目 License 选择 | **已拍板**（P0-A §2：GPL-3.0-or-later） |

> **五项全部收敛，已拍板（2026-09-18）。Phase 0 关闭，进入 04 技术设计。**

---

## 附录：调研来源

- 00 基石 §7.10 ToolCompatibility 实体、§6 兼容性五态、§11 开放数据集定位
- P0-A §1：原神解锁上游「理论支持后续版本」声明
- P0-B §6.3：鸣潮键语义漂移史、2.7 验证证据、3.x 待实测

## 变更记录

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1 | 2026-09-18 | 初版：Schema 0.1.0 定稿（prefix 匹配 / Verified-Compatible 分级 / Unknown 兜底）、首批 3 条种子数据、入场条件五项收敛 |
| v0.2 | 2026-09-18 | 增补原神 7.x prefix Compatible 兜底层（04 深度审查拍板：exact 7.0 在 live 7.1+ 会常态 Unknown 拦死 B7）；§3.1 补依据行，共 4 条 |
