# orbis-core

领域模型 + 能力 trait + 兼容引擎。**零游戏知识、零平台依赖**（架构不变量 ①，`pm/04-技术设计.md` §9.1）。

## 依赖方向

本 crate **不得**依赖 `orbis-platform` / `orbis-providers` / `orbis-tools`（编译期保证），
也不得出现任何具体游戏标识（5 个 game slug 不得出现在本 crate 的任何文件中）——
新增游戏不需要修改 Core（`pm/00-产品基石.md` §12.3 规则 1 / §12.4 B 组）。

CI 会对上述两条做 grep 断言（`.github/workflows/ci.yml`）。

## 现状

已实现（纯逻辑，**任何平台都可 `cargo test`**，含表驱动用例）：

| 模块 | 内容 | 设计依据 |
|------|------|----------|
| `model` | `GameRuntimeStatus` / `GameId` / `Region` 等枚举口径 | 04 §6.4.1；契约 §2 |
| `version` | 版本归一化 `normalize` + 数值点分比较 | 04 §5.1 / §5.2 |
| `compat` | 兼容匹配 `exact → prefix → none` 与五态 | P0-C §2.2 / 04 §5.6 |
| `attention` | 「需处理」判定式 | 04 §6.4.3 |

尚未实现（需要工具链 / 外部依赖后补）：

- 能力 trait（`VersionSource` / `ConfigSource` / `LaunchSpec` / `DetectRule`）→ 需 `async-trait` 或原生 async，见 04 §4.2
- `seed.json` / Manifest 反序列化 → 需 `serde`；schema 校验在 `data/*.schema.json`
- 错误类型 → 需 `thiserror`，错误码口径见 `docs/ipc-contract.md` §5

## 测试

```bash
cargo test -p orbis-core      # 任意平台均可运行（含 macOS）
```
