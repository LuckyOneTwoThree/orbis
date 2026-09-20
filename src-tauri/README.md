# src-tauri/ —— Tauri 桌面壳

**状态**：已实现并接入 workspace（曾计划由 `cargo tauri init` 生成；为达成「能产出 Windows
安装包」这一目标改为手写最小壳，因为壳是打包的前置条件）。壳内**不含业务逻辑**。

## 目录

```
src-tauri/
├── Cargo.toml              # 唯一允许依赖全部 4 个领域 crate 的组装根（04 §4.1）
├── build.rs                # tauri-build：校验 capabilities、生成权限 schema 到 gen/
├── tauri.conf.json         # 窗口 / 打包 / CSP 配置
├── capabilities/default.json  # 主窗口最小权限集
├── icons/                  # 仅保留 Windows 打包实际消费的图标（见下）
└── src/
    ├── main.rs             # 入口（发布构建 windows_subsystem = "windows"）
    └── lib.rs              # windowControl 命令 + 单实例互斥 + 启动自检
```

## 关键配置决策

| 配置 | 取值 | 原因 |
|------|------|------|
| `windows[0].decorations` | `false` | 03 §5.1 要求自绘标题栏（左字标 + 右窗口控制）。**代价**：必须由 `Header` 上的 `data-tauri-drag-region` 提供拖动，且窗口按钮必须真的可用 —— 因此 `windowControl` 不是可选功能 |
| `windows[0].backgroundColor` | `#050505` | 避免 WebView 首帧前的白闪；与 03 §1.1 的纯黑基底一致 |
| `windows[0]` 尺寸 | 1280×800，最小 1024×640 | 03 §2.3 的基准窗口 |
| `bundle.targets` | `["nsis"]` | MVP 只做 Windows；NSIS 比 MSI 更适合无签名分发（02 §6 R4） |
| `bundle.windows.nsis.installMode` | `currentUser` | 00 §8.9 最小权限：安装不触发 UAC，不申请永久管理员 |
| `nsi.languages` | `SimpChinese` + `English` | 中文优先，关闭语言选择器（MVP 中文单语） |
| `bundle.windows.webviewInstallMode` | 未设置 → 默认 `downloadBootstrapper` | 目标平台 Win10 19041+/Win11 均已预装 WebView2，通常无需下载；不选 `offlineInstaller`（+约 130MB） |
| `app.security.csp` | 严格，仅 `'self'` + `ipc:` | 可能，是因为字体已自托管（见 `src/styles/fonts.css`）且无任何外部请求。`style-src` 需 `'unsafe-inline'`（组件使用内联 `style`） |

## 命令实现状态

契约共 **28 条命令**（`docs/ipc-contract.md` §3）。当前实现 **1 条**：

- ✅ `windowControl`（契约 §3.10）—— 自绘标题栏的可用性前提
- ⬜ 其余 27 条：待 `orbis-core` / `orbis-platform` 落地后逐条实现

**命名规则（重要）**：契约 §1 规定命令名为 camelCase，且 `tauri-specta` 会把 Rust 函数名
直接镜像成 TS 侧命令名。因此 `lib.rs` 刻意使用非 snake_case 函数名（文件顶部有
`#![allow(non_snake_case)]` 与说明），以保证「契约 / Rust 函数名 / TS 调用名」三者一致。

`tauri-specta` 本身尚未接入：当前没有任何业务命令可绑定，此时接入只会产生空绑定。
它应在第一批业务命令落地的同时引入，并随后替换 `src/api/types.ts` 的手写 DTO。

## 图标

```bash
npx tauri icon ui/icon.png      # 重新生成（会同时产出 iOS / Android / MSIX 变体）
```

生成后**需要手动清理**：本项目 MVP 只做 Windows 桌面，`icons/` 里仅保留 Windows 打包
实际消费的 6 个文件（`32x32` / `64x64` / `128x128` / `128x128@2x` / `icon.png` / `icon.ico`）。
`android/`、`ios/`、`icon.icns`、`Square*Logo.png`、`StoreLogo.png` 已删除，合计省下约 3.4MB ——
它们在 Windows NSIS 构建中完全不参与。将来新增平台时重跑上面的命令即可。

**已知不足**：`ui/icon.png` 是 1254×1254 的 **RGB（无 alpha 通道）** 图，因此生成的应用图标
是实心方块，没有透明边缘。它可用但不达设计标准，发布前应替换为带透明通道的正式图标。

## 本地验证范围

Windows 开发机自 2026-09-20 起具备 Rust 工具链（rustc 1.98.1 / stable-msvc + MSVC 18 BuildTools），
以下内容**已可在本地验证**：

- `cargo check -p orbis` —— 首次 Windows 编译已通过（此前只能靠 CI）
- `cargo fmt --check` / `clippy -D warnings` / `cargo test` —— 本地与 CI 同源

仍需在真机上验证（本机缺少待测游戏与窗口交互环境）：

- Windows 上的 NSIS 打包是否成功
- `decorations: false` 下的拖动、最大化、关闭行为

> **编译前必须先构建前端**：`tauri::generate_context!` 会在编译期校验 `frontendDist`
> （`tauri.conf.json` = `../dist`）是否存在，缺失会直接 proc macro panic。
> 所以流程是 `npm run build` → `cargo check -p orbis`。CI 的 `shell` job 已同步加上该步骤。

## Cargo.lock

`Cargo.lock` 已于 2026-09-20 在本机生成（477 个包，含完整 Tauri 依赖树）。
按依赖治理约定（04 §2.2）它应当入库以获得可复现构建；入库后 CI 的
`cargo check -p orbis` 应升级为 `--locked`。
