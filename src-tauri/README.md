# src-tauri/ —— Tauri 壳（待生成）

**这个目录里的文件不由手写产生**，必须由 Tauri CLI 生成后再按本说明接入。

## 为什么空着

Tauri 壳包含 `tauri.conf.json`（schema 版本敏感）、`build.rs`、`capabilities/*.json`、
`icons/` 等工具链产物。手写这些文件在版本不匹配时会产生难以诊断的构建错误，
且本仓库当前**尚无 Rust 工具链**（开发机 macOS，目标平台 Windows），无法验证。
因此这里只留说明与接入清单，避免制造「看起来能跑但其实是假的」的工程状态。

## 生成与接入步骤（在 Windows 开发机上执行）

```bash
# 1. 工具链
rustup toolchain install stable
rustup target add x86_64-pc-windows-msvc
cargo install tauri-cli --version "^2"

# 2. 生成壳（在本仓库根目录执行，指向已有的前端）
#    frontendDist 指向 Vite 构建产物 dist/，devUrl 指向 http://localhost:3000
cargo tauri init \
  --app-name orbis \
  --window-title Orbis \
  --frontend-dist ../dist \
  --dev-url http://localhost:3000 \
  --before-dev-command "npm run dev" \
  --before-build-command "npm run build"

# 3. 把 src-tauri 加入 workspace members（根 Cargo.toml 已预留注释位置）
```

## 生成后必须补齐的配置

| 项 | 值 / 依据 |
|----|-----------|
| 基准窗口尺寸 | 1280×800 @100%，须支持 125% / 150% DPI（03 §2.3） |
| 深色基底 | 窗口背景 `#050505`，避免白闪（03 §1.1） |
| 单实例互斥 | `tauri-plugin-single-instance`：第二实例只前置已有窗口（04 §5.12） |
| 权限最小化 | 默认普通用户权限，不申请永久管理员（00 §8.9） |
| 命令注册 | 27 条命令见 `docs/ipc-contract.md` §3（v1.0 + v1.1 `listGames`） |
| 类型绑定 | `tauri-specta` 生成 `src/api/bindings.gen.ts`（04 §2.1 / Q3 已定稿引入） |
| 事件桥 | 6 个事件见契约 §4，事件名含 `:`，需确认 Tauri 事件名白名单允许 |
| 打包 | NSIS 优先；无签名兜底 = 白名单引导文档（02 §6 R4） |

## 与前端的分工

`src-tauri` 是**唯一允许依赖全部 crate 的组装根**（04 §4.1）：

```
src-tauri → providers, tools, platform, core
```

它只做三件事：命令注册、生命周期、事件桥。**不得**在此处写业务逻辑
（业务在 core / platform / providers / tools，00 §7.9）。
