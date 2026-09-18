# Orbis

> 跨厂商二次元 PC 游戏中心 —— 统一发现、启动与管理，并为每款游戏提供经过版本适配的 PC Enhancement 与专属工具。

**项目性质**：开源、非商业化 · **License**：GPL-3.0-or-later · **首发平台**：Windows 10 19041+ / Windows 11 x64

---

## 这个仓库当前处于什么阶段

| 层 | 状态 |
|----|------|
| 产品定义（`pm/00`–`pm/02`） | ✅ 完成，Phase 0 五项入场条件已收敛 |
| UI 设计（`pm/03`） | ✅ 设计系统与 S0–S6 页面规格定稿 |
| 技术设计（`pm/04`） | ✅ v0.3，含模块设计、DB schema、状态机、降级策略、待实测清单 |
| 前后端契约（`docs/ipc-contract.md`） | ✅ v1.1 **已冻结** |
| 内置数据（`data/`） | ✅ 种子表 + Manifest + 资产清单 + JSON Schema，CI 校验通过 |
| 前端 UI | 🚧 8 屏骨架完成，走 mock 参照实现（`npm run dev` 可直接看） |
| Rust Core | 🚧 仅有骨架；已实现平台无关的纯逻辑切片（版本口径 / 兼容匹配 / 需处理判定） |
| 真机实测（T1–T8） | ⬜ 待 Windows 开发机执行，见 `docs/实测-T1-T8.md` |

**尚未实现的功能不会假装可用**：契约里的降级路径（如解锁组件未发布）在 UI 上都有显式呈现。

---

## 快速开始（前端）

```bash
npm install
npm run dev        # http://localhost:3000
```

非 Tauri 环境下自动使用 mock 参照实现；在 Tauri 中可用 `VITE_ORBIS_API=mock` 强制回退，
便于对比调试。mock 的行为口径与 `data/` 严格一致（见下方「质量闸门」）。

```bash
npm run verify     # 数据校验 + mock 行为核对 + 类型检查（提交前请跑）
npm run build      # 生产构建
```

## 快速开始（Rust Core）

目标平台是 Windows，但 `orbis-core` **刻意保持零平台依赖**，因此任何平台都能跑它的单测：

```bash
cargo test -p orbis-core
```

Windows 开发机还需完成 Tauri 壳的生成，见 [`src-tauri/README.md`](src-tauri/README.md)。

---

## 仓库结构

```
orbis/
├── pm/                   产品文档（00 产品基石 / 01 功能清单 / 02 PRD / 03 UI / 04 技术设计 / Phase 0 记录）
├── docs/                 工程文档（前后端契约 / 真机实测记录 / 开工前置清单）
├── data/                 内置数据（随发行包分发，含 JSON Schema）
│   ├── compatibility/    兼容性种子表（**兼容性唯一权威**）
│   └── tools/            工具 Manifest 与解锁器资产清单
├── crates/               Rust workspace
│   ├── orbis-core/       领域模型 + 兼容引擎（零游戏知识 / 零平台依赖）
│   ├── orbis-platform/   SQLite / 进程 / 注册表 / 日志 / 备份
│   ├── orbis-providers/  5 款游戏装配（唯一允许出现游戏知识的地方）
│   └── orbis-tools/      Manifest 加载 + L1/L3 执行器
├── src-tauri/            Tauri 壳（由 cargo tauri init 生成，见其 README）
├── src/                  React UI
│   ├── api/              **前后端契约层**：UI 唯一依赖点，禁止在别处直接 invoke
│   ├── store/            Zustand：只调 api，不做业务判定
│   ├── views/            S0–S6 页面
│   └── utils/            纯表现层格式化与错误码文案映射
└── scripts/              CI 与本地共用的校验脚本
```

### 依赖方向（架构不变量）

```
src-tauri → providers, tools, platform, core      唯一组装根
providers → core, platform
tools     → core, platform
platform  → core
core      → （无内部依赖）
```

`orbis-core` 中出现任何具体游戏标识即视为不变量破坏 —— 目的是保证**新增游戏不需要修改 Core**
（`pm/00-产品基石.md` §12.3 规则 1）。CI 对此有 grep 断言。

### 前端分层（`pm/00` §7.9）

```
views/  →  store/  →  src/api/  →  （mock | Tauri IPC）
```

`views/` 与 `store/` 中**不得**出现 `invoke(...)`、路径拼接、SQL、哈希运算。
所有面向用户的文案由 UI 按错误码 / 状态枚举映射，后端只返回结构化数据
（`docs/ipc-contract.md` §1、§7.3）。

---

## 质量闸门

`npm run verify` 串联三道闸门，全部为跨平台，可在 CI 与本地一致执行：

| 闸门 | 命令 | 守护什么 |
|------|------|----------|
| 内置数据 | `npm run validate:data` | `seed.json` / Manifest / `assets.json` 的 schema 与**跨文件一致性**（manifest.id ↔ seed.tool_id ↔ assets.asset_key、namespace 与 source.kind 一致、`config_modify` 必须 `backup_required`） |
| mock 行为 | `npm run test:mock` | mock 参照实现的行为口径：兼容匹配 exact→prefix→none、需处理判定、执行流步骤顺序与两层 detail、失败回滚、L3 门控、运行中禁写、设置白名单等 **42 项断言** |
| 视图渲染 | `npm run test:render` | jsdom 客户端渲染全部页面与模态，并断言**真实数据落到 DOM**；含 **26 项禁词回归防护**（文档要求删除的安全担保与虚构指标不得复活）与 U8 分层检查，共 **38 项断言** |
| 架构不变量 | `npm run check:arch` | Core 无游戏知识、依赖方向、views/store 不直连 IPC、默认层不泄漏底层术语等 **8 项断言** |
| 类型 | `npm run typecheck` | 契约 DTO ↔ UI 的类型一致性 |

三道脚本闸门都做过**注入式反向测试**（故意写违规内容确认能拦住），不是空跑；
断言刻意只用 POSIX 字符类，因为 `\s` / `\b` 在 macOS BSD grep 下会静默失效。

Rust 侧另有 `cargo fmt --check` / `clippy -D warnings` / `cargo test` / `cargo deny check`
与架构 grep 断言，见 `.github/workflows/ci.yml`。

> 数据是发行物的一部分：`data/` 的改动**必须**同时通过 schema 与跨文件校验，
> 不允许「本地改完就过」。

---

## 安全模型（不可关闭）

Orbis 的信任基础不是「我们保证安全」，而是**每一步都可回滚、每一处降级都可见**：

- 修改配置前**强制备份**，无备份不修改
- 修改后**读回验证**，不一致即失败
- 失败或 panic **自动回滚**，工具链路不影响启动器主流程
- L3（进程级）工具**默认关闭**，启用必经版本化风险文案 + 显式授权
- 游戏版本更新后未验证的工具**被门控阻止**，而不是静默生效
- 任何降级都必须显式可见，**禁止静默失败**

这些机制在设置页里**不提供关闭开关**（唯一的例外是 L1 工具在未验证版本下的「逐次继续使用」，
它仍然走完整的备份 → 修改 → 验证链路）。

> 我们不承诺「绝对不会封号」。进程级工具属于社区多年实践、非官方授权，
> 相关风险由用户知情后自主承担（`pm/00-产品基石.md` §8.11）。

---

## 开源合规

- 项目自身：GPL-3.0-or-later（全文见 [LICENSE](LICENSE)）
- 第三方组件与上游工具的声明见 [NOTICE](NOTICE)
- **GPL 系工具只作外部进程调用，不嵌入、不静态链接**（`pm/00` §8.6）
- CI 通过 `cargo deny` 校验依赖 License 白名单与安全公告

---

## 文档索引

| 文档 | 作用 |
|------|------|
| [`pm/00-产品基石.md`](pm/00-产品基石.md) | 定位、边界、架构、安全与验收的**唯一权威** |
| [`pm/01-MVP功能清单.md`](pm/01-MVP功能清单.md) | 范围契约（P0 / P1 / 不做） |
| [`pm/02-MVP-PRD.md`](pm/02-MVP-PRD.md) | 每条 P0 的用户故事与 Given/When/Then 验收 |
| [`pm/03-UI设计文档.md`](pm/03-UI设计文档.md) | 设计系统、S0–S6 页面规格、验收清单 |
| [`pm/04-技术设计.md`](pm/04-技术设计.md) | 技术方案、模块设计、DB schema、降级策略、待实测 T1–T8 |
| [`docs/ipc-contract.md`](docs/ipc-contract.md) | 前后端契约 v1.1（**已冻结**，字段级唯一来源） |
| [`docs/05-开工前置准备清单.md`](docs/05-开工前置准备清单.md) | 开工前审计结论与处理状态 |
| [`docs/实测-T1-T8.md`](docs/实测-T1-T8.md) | 真机实测记录（Windows） |

文档冲突时的优先级：`00` 基石 > `01` 清单 > `02` PRD > `04` 技术设计 > 契约 > 代码。
范围争议回 `01` 评审，验收争议回 `02`，**不允许实现侧单方面放宽**。

---

## 贡献

当前阶段（单人主导 + 待社区接入）优先接受：

- **真机实测结论**：`docs/实测-T1-T8.md` 中的待办项，附环境与复现步骤
- **兼容性数据**：向 `data/compatibility/seed.json` 提交条目（附证据链接与署名），
  这是产品长期最核心的开放数据资产
- **新游戏接入**：新增 `orbis-providers` 模块 + catalog 条目 + seed 数据，
  **不得**修改 `orbis-core`

提交前请确保 `npm run verify` 与 `cargo test` 通过。
