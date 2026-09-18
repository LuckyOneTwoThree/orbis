---
title: Phase 0 冲刺 A · 上游核实与 License 决策记录
doc_id: P0-A
type: phase0-research
status: accepted
version: v0.1
date: 2026-09-18
upstream: 00-产品基石.md v0.5（§10.1 入场条件 #1 / #5，§13 Q2 / Q4 / Q5）
tags:
  - phase0
  - research
  - adr
---

# Phase 0 冲刺 A · 上游核实 + License 决策（F4 / Q4 / Q5）

> 对应 00 基石 §10.1 入场条件 **#1（F4 上游核实）** 与 **#5（Q4 License）**，顺带收敛 Q2（L3 风险证据）与 Q5（品牌名）。
> 性质：ADR 风格决策记录，非正式编号文档；**04 技术设计文档的直接输入**。

---

## 1. F4 · 原神 FPS 解锁上游核实

### 1.1 候选项目盘点（2026-09-18 核实）

| 项目 | License | 技术栈 | 维护状态 | 评估 |
|------|---------|--------|----------|------|
| [34736384/genshin-fps-unlock](https://github.com/34736384/genshin-fps-unlock)（原版） | MIT（2021-Present） | C# / .NET 8 | v3.0.4；README 声明「理论上支持后续版本，需要时会尽快更新」 | **集成参考首选** |
| [Genshin-Stella-Mod/Genshin-FPS-Unlocker](https://github.com/Genshin-Stella-Mod/Genshin-FPS-Unlocker)（sefinek fork） | MIT（34736384 + Sefinek 双版权） | C# / .NET 10 | 活跃（VS2026 编译、.NET 10 Runtime） | 集成备选；跟踪其上游活跃度 |

注：Genshin Stella Mod 本体（ReShade + FPS Unlocker + 3DMigoto 集成，v8.12.x，2026 仍活跃）含商业化订阅（Stella Plus），**仅参考其集成架构，不集成、不复用其代码**。

### 1.2 技术事实（04 技术设计的直接输入）

- **原理**：外部进程 `WriteProcessMemory` 写入 FPS 值 + 句柄保护绕过，**无需驱动**
- **启动模型**：解锁器以管理员权限启动并拉起游戏进程，进程需保持运行——与 02 B8「解锁模式启动编排」设计吻合，可编排化
- **游戏发现**：通过注册表自动定位游戏路径——与 A1/J1 自动发现可复用同一机制
- **双区域**：国服 / 国际服均支持（Q7 的 region 维度已有先例）
- **签名事实**：fork 版未签名，触发 Windows Smart App Control 拦截（README 明示需手动关闭）——**印证 R4「签名与分发归属」的必要性**，发行包分离 + 哈希校验 + 明确的签名策略缺一不可

### 1.3 风险证据（Q2 联动，双源一致）

- 原版 README（作者声明）：HoYoverse 知晓该工具，**仅使用 FPS 解锁不会封号**；叠加其他第三方插件风险自负
- 社区指南（2026 年更新）：FPS 解锁被容忍多年，无「仅因解锁 FPS」的封号记录
- **结论**：L3 风险披露文案可如实表述为「社区多年实践被容忍，但非官方授权」——维持默认关闭 + 显式授权 + 发行包分离设计不变

### 1.4 F4 决策建议

选定 `34736384/genshin-fps-unlock`（MIT）为集成参考实现，跟踪 sefinek fork 作为上游活跃度信号。集成遵循 00 §5.2：发行包分离 + 哈希校验 + L3 显式授权流。

---

## 2. Q4 · 项目 License 决策

### 2.1 证据矩阵（竞品与上游 License 核实）

| 项目 | License | 对 Orbis 的借鉴价值 |
|------|---------|---------------------|
| Starward（4.5k stars，C#，活跃） | MIT | 检测 / 游戏时间 / 启动器 UX——任意 License 下均可借鉴代码 |
| TwintailLauncher（v2.4.0，2026-05 更新） | **GPL-3.0-only** | Tauri/Rust 架构、跨厂商适配——仅 GPL 系下可借鉴代码 |
| FPS Unlocker 原版 / fork | MIT | L3 集成——任意 License 下均可借鉴代码 |

### 2.2 决策矩阵

| 维度 | MIT | GPL-3.0 | AGPL-3.0 |
|------|:---:|:-------:|:--------:|
| 借鉴 Twintail 代码 | ✗（仅思路与知识） | ✓ | ✓ |
| 借鉴 Starward / MIT 上游代码 | ✓ | ✓ | ✓ |
| 防闭源 fork 收割 Compatibility 数据资产 | ✗ | ✓ | ✓ |
| 贡献者心理门槛 | 低 | 中 | 中 |
| 对桌面应用的实际增益 | — | 分发传染即保护 | 网络服务条款对桌面应用基本无增益 |

### 2.3 推荐

**GPL-3.0（建议 or-later 表述）**：

1. 唯一能**同时借鉴 GPL（Twintail）+ MIT（Starward、Unlocker 全系）代码**的选项——借鉴面最大化
2. 项目明确非商业化，MIT 的商业友好性无实际价值
3. Compatibility 数据集是核心公共资产（00 §11），GPL 防止闭源分叉收割社区贡献
4. AGPL 对桌面应用无增量保护，徒增理解成本

> **已拍板（2026-09-18）：GPL-3.0-or-later。** 回写已执行（00 基石 v0.6 frontmatter / §1.9 / §8.6），本记录 status 改 `accepted`。

---

## 3. Q5 · 品牌名核查（轻量）

- **Orbis OS = PS4/PS5 操作系统**（FreeBSD fork）——游戏语境下强混淆，玩家搜索「Orbis」大概率命中 PlayStation 内容
- ORBIS Corporation 为注册商标持有方（工业包装领域）——跨类目法律风险低但客观存在
- **建议**：`Orbis` 保留为仓库 / 内部代号；对外产品名建议启动候选征集（语义方向：二游 / 星轨 / 控制台 / 环带），MVP 发布前锁定

---

## 4. 文档回写清单（拍板后执行）

| 目标 | 动作 |
|------|------|
| 00 基石 | frontmatter `license` 字段落定；§8.6 补 Twintail=GPL-3.0-only 证据；§13 Q2/Q4/Q5 标注「已收敛」 |
| 02 PRD | B7 补「上游 MIT 已核实（2026-09-18）」证据引用 |
| 04 技术设计（未来） | 启动编排的注册表发现机制、Smart App Control 应对策略、.NET Runtime 依赖处理 |

---

## 5. Phase 0 入场条件进度

| # | 入场条件 | 状态（终态，2026-09-18） |
|---|----------|------|
| 1 | F4 原神解锁上游核实 | **完成**（本记录 §1，已拍板） |
| 2 | F2 兼容性种子表 v0 | **已交付**（P0-C） |
| 3 | E1 版本源拍板 | **已拍板**（P0-B §4） |
| 4 | 5 款游戏 Provider 配置声明清单 | **已拍板**（P0-B §6.5） |
| 5 | 项目 License 选择 | **已拍板**（本记录 §2，GPL-3.0-or-later） |

---

## 附录：调研来源

- https://github.com/34736384/genshin-fps-unlock（README 双语 / MIT License / v3.0.4）
- https://github.com/Genshin-Stella-Mod/Genshin-FPS-Unlocker（fork README / Smart App Control 说明）
- https://sefinek.net/genshin-stella-mod/docs（Stella Mod 文档：License & Credits 页，MIT 双版权原文）
- Starward：HelloGitHub 收录页（MIT / 4.5k stars / C# / 活跃）
- TwintailLauncher：RPM spec（`License: GPL-3.0-only`）+ GitHub org 仓库页（GPL v3.0，2026-05 更新）
- Voltris《Genshin Impact Ultra (2026)》指南（封号风险社区证据，葡萄牙语）

## 变更记录

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1 | 2026-09-18 | 初版：F4 双候选核实（均 MIT）、Q2 风险双源证据、Q4 决策矩阵与 GPL-3.0 推荐、Q5 品牌名冲突确认 |
