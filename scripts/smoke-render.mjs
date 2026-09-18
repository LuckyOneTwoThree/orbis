#!/usr/bin/env node
/**
 * 视图渲染冒烟测试（CI 与本地共用）
 *
 * 与 `smoke-mock.mjs` 互补：那个验证**数据与行为**口径，这个验证**渲染路径**。
 * `tsc` 只能保证类型正确，抓不到「首次渲染就崩」（可选链缺失、选择器返回不稳定引用、
 * 空数据分支未处理）。本脚本用真实数据把每个视图渲染一遍。
 *
 * # 为什么用 jsdom 而不是 renderToString
 *
 * zustand v4 的 `useStore` 把 `getInitialState()` 作为 `useSyncExternalStore` 的
 * **server snapshot**，因此在 `renderToString` 下 hook 永远读到「store 创建时」的状态——
 * 无论之前 setState 过什么。实测：`getState().catalog.length === 5` 而
 * `hook 读到 0`。更糟的是空状态会让多数视图提前走「未找到」分支，等于什么都没测。
 *
 * 所以这里起一个 jsdom 环境走**客户端渲染**（`react-dom/client`），与浏览器行为一致。
 *
 * # 额外价值
 *
 * 对渲染产物做**内容断言**：文档要求删除的内容（00 §8.11 安全担保、不存在的指标、
 * 二进制写入语义）一旦被谁加回来，这里会失败 —— 把「已删除」变成回归防护。
 */
import { build } from 'esbuild';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { JSDOM } from 'jsdom';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

let pass = 0;
let failCount = 0;
const check = (name, cond, extra = '') => {
  if (cond) {
    pass++;
    console.log(`  ✓ ${name}`);
  } else {
    failCount++;
    console.log(`  ✗ ${name}${extra ? ` — ${extra}` : ''}`);
  }
};

/** 直接报告一条失败（用于非「条件断言」形态的检查，如遍历禁用词表） */
const reportFail = (message) => {
  failCount++;
  console.log(`  ✗ ${message}`);
};

// ── jsdom 环境（必须在导入 react-dom/client 之前建立）──────────────
const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: 'http://localhost/',
  pretendToBeVisual: true,
});
globalThis.window = dom.window;
globalThis.document = dom.window.document;
globalThis.HTMLElement = dom.window.HTMLElement;
globalThis.Element = dom.window.Element;
globalThis.Node = dom.window.Node;
globalThis.getComputedStyle = dom.window.getComputedStyle;
globalThis.requestAnimationFrame = dom.window.requestAnimationFrame.bind(dom.window);
globalThis.cancelAnimationFrame = dom.window.cancelAnimationFrame.bind(dom.window);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
// Node 18+ 的 globalThis.navigator 是只读 getter，必须走 defineProperty
Object.defineProperty(globalThis, 'navigator', {
  value: dom.window.navigator,
  configurable: true,
  writable: true,
});

// 产物必须落在**项目内**：bare import（react 等）由 Node 依据导入文件所在位置向上查找
// node_modules，放在系统临时目录会 ERR_MODULE_NOT_FOUND。
const outDir = join(ROOT, 'node_modules/.cache/orbis-smoke');
const outFile = join(outDir, 'entry.mjs');
mkdirSync(outDir, { recursive: true });

// 把「应用 + 数据」一起打包，避免 store 单例不同源
await build({
  stdin: {
    contents: `
      export { App } from ${JSON.stringify(join(ROOT, 'src/App.tsx'))};
      export { HomeView } from ${JSON.stringify(join(ROOT, 'src/views/HomeView.tsx'))};
      export { WuwaDashboardView } from ${JSON.stringify(join(ROOT, 'src/views/WuwaDashboardView.tsx'))};
      export { GenshinDashboardView } from ${JSON.stringify(join(ROOT, 'src/views/GenshinDashboardView.tsx'))};
      export { GenericDashboardView } from ${JSON.stringify(join(ROOT, 'src/views/GenericDashboardView.tsx'))};
      export { ConfigHistoryView } from ${JSON.stringify(join(ROOT, 'src/views/ConfigHistoryView.tsx'))};
      export { SettingsView } from ${JSON.stringify(join(ROOT, 'src/views/SettingsView.tsx'))};
      export { OnboardingView } from ${JSON.stringify(join(ROOT, 'src/views/OnboardingView.tsx'))};
      export { L3ConsentModal } from ${JSON.stringify(join(ROOT, 'src/components/modals/L3ConsentModal.tsx'))};
      export { ExecutionFlowModal } from ${JSON.stringify(join(ROOT, 'src/components/modals/ExecutionFlowModal.tsx'))};
      export { TerminateConfirmModal } from ${JSON.stringify(join(ROOT, 'src/components/modals/TerminateConfirmModal.tsx'))};
      export { useAppStore } from ${JSON.stringify(join(ROOT, 'src/store/useAppStore.ts'))};
      export { useGamesStore } from ${JSON.stringify(join(ROOT, 'src/store/useGamesStore.ts'))};
      export { useToolsStore } from ${JSON.stringify(join(ROOT, 'src/store/useToolsStore.ts'))};
      export { useBackupsStore } from ${JSON.stringify(join(ROOT, 'src/store/useBackupsStore.ts'))};
      export { useSettingsStore } from ${JSON.stringify(join(ROOT, 'src/store/useSettingsStore.ts'))};
      export { mockApi } from ${JSON.stringify(join(ROOT, 'src/api/mock.ts'))};
    `,
    resolveDir: ROOT,
    loader: 'ts',
  },
  bundle: true,
  format: 'esm',
  platform: 'node',
  jsx: 'automatic',
  outfile: outFile,
  loader: { '.json': 'json' },
  // 所有 bare import 保持外部化：否则 bundle 内会带一份 React/zustand，
  // 而测试又从 node_modules 引另一份 → hooks dispatcher 为 null。
  packages: 'external',
  // 与 Vite 构建等价的编译期常量
  define: {
    'import.meta.env.DEV': 'false',
    'import.meta.env.PROD': 'true',
    'import.meta.env.VITE_ORBIS_API': '"mock"',
  },
  logLevel: 'error',
});

const { createElement: h, act } = await import('react');
const { createRoot } = await import('react-dom/client');
const {
  App,
  HomeView,
  WuwaDashboardView,
  GenshinDashboardView,
  GenericDashboardView,
  ConfigHistoryView,
  SettingsView,
  OnboardingView,
  L3ConsentModal,
  ExecutionFlowModal,
  TerminateConfirmModal,
  useAppStore,
  useGamesStore,
  useToolsStore,
  useBackupsStore,
  useSettingsStore,
  mockApi,
} = await import(pathToFileURL(outFile).href);

const htmlByView = {};

/**
 * 客户端渲染（走真实 getSnapshot 路径），捕获异常。
 *
 * `settleMs`：组件若有「useEffect 里异步取数」再渲染的分支（如各模态），
 * 需要在 act 内多等一个事件循环，否则断言会打在加载态上。
 */
async function renderView(name, Component, settleMs = 0) {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  try {
    await act(async () => {
      root.render(h(Component));
    });
    if (settleMs > 0) {
      await act(async () => {
        await new Promise((r) => setTimeout(r, settleMs));
      });
    }
    const html = container.innerHTML;
    htmlByView[name] = html;
    return html;
  } catch (e) {
    failCount++;
    console.log(`  ✗ ${name} 渲染抛出异常：${e && e.message ? e.message : e}`);
    const frame = e && e.stack ? e.stack.split('\n').find((l) => l.includes('entry.mjs')) : null;
    if (frame) console.log(`      ${frame.trim()}`);
    htmlByView[name] = '';
    return null;
  } finally {
    await act(async () => {
      root.unmount();
    });
    container.remove();
  }
}

console.log('\n[1] 首屏（空数据）必须能渲染');
{
  const html = await renderView('app-empty', App);
  check('App 空数据渲染不抛异常', html !== null);
  if (html) check('空数据首屏有可读文案（非空白）', html.length > 200, `len=${html.length}`);
}

console.log('\n[2] 载入真实数据后，全部视图可渲染');
let homeHtml = null;
let wuwaHtml = null;
let genshinHtml = null;
let genericHtml = null;
let historyHtml = null;
let settingsHtml = null;
{
  // 用 mock 参照实现填充 store（与浏览器里 initialize() 之后的状态等价）
  const [list, catalog, tools, settings] = await Promise.all([
    mockApi.listInstallations(),
    mockApi.listGames(),
    mockApi.listTools(),
    mockApi.getSettings(),
  ]);
  const wuwa = list.installations.find((i) => i.gameId === 'wuthering-waves');
  const backups = wuwa ? await mockApi.listBackups(wuwa.id) : [];
  const storage = wuwa ? await mockApi.getBackupStorageInfo(wuwa.id) : null;

  useGamesStore.setState({
    catalog,
    installations: list.installations,
    summary: list.summary,
    loading: false,
  });
  useToolsStore.setState({ tools, loading: false });
  useBackupsStore.setState({ backups, storage, loading: false });
  useSettingsStore.setState({ settings, loading: false });

  homeHtml = await renderView('home', HomeView);
  check('S1 首页渲染成功', homeHtml !== null);

  useAppStore.setState({ activeView: 'wuthering-waves' });
  wuwaHtml = await renderView('wuwa', WuwaDashboardView);
  check('S2a 鸣潮 Dashboard 渲染成功', wuwaHtml !== null);

  useAppStore.setState({ activeView: 'genshin-impact' });
  genshinHtml = await renderView('genshin', GenshinDashboardView);
  check('S2b 原神 Dashboard 渲染成功', genshinHtml !== null);

  useAppStore.setState({ activeView: 'generic', activeGameId: 'honkai-star-rail' });
  genericHtml = await renderView('generic', GenericDashboardView);
  check('S2c 通用 Dashboard 渲染成功', genericHtml !== null);

  useAppStore.setState({ activeView: 'config-history', activeGameId: 'wuthering-waves' });
  historyHtml = await renderView('history', ConfigHistoryView);
  check('S5 配置历史渲染成功', historyHtml !== null);

  useAppStore.setState({ activeView: 'settings' });
  settingsHtml = await renderView('settings', SettingsView);
  check('D5 设置页渲染成功', settingsHtml !== null);

  useAppStore.setState({ activeView: 'onboarding' });
  const onboardingHtml = await renderView('onboarding', OnboardingView);
  check('S6 首次引导渲染成功', onboardingHtml !== null);
}

console.log('\n[3] 模态（L3 授权 / D4 执行流 / A4 结束进程）');
{
  useAppStore.setState({ activeView: 'genshin-impact', l3ModalToolId: null });
  const closed = await renderView('l3-closed', L3ConsentModal);
  check('未打开时 L3 模态不渲染任何内容', closed === '', `len=${closed?.length}`);

  useAppStore.setState({ l3ModalToolId: 'orbis-bundled/genshin-fps-unlock' });
  // getTool 是异步的（内部有 await），需等待其 resolve 后再断言
  const l3Html = await renderView('l3-open', L3ConsentModal, 300);
  check('L3 模态渲染成功', l3Html !== null);
  check('L3 模态展示「不承诺绝对安全」的风险披露（00 §8.11）',
    /不承诺绝对安全/.test(l3Html ?? ''), '检查 consentText');
  check('L3 模态列出权限范围（C3 验收）',
    /执行机制与权限范围/.test(l3Html ?? ''));
  check('组件未发布时给出显式降级说明而非报错',
    /组件尚未发布/.test(l3Html ?? ''));

  useAppStore.setState({ l3ModalToolId: null, executionToolId: 'orbis-builtin/wuwa-fps-120' });
  const flowHtml = await renderView('flow', ExecutionFlowModal);
  check('D4 执行流模态渲染成功', flowHtml !== null);
  check('执行流声明写入统一日志（D3/D4）',
    /统一日志/.test(flowHtml ?? ''));

  useAppStore.setState({ executionToolId: null, terminateConfirmId: 'inst-genshin-cn' });
  const termHtml = await renderView('terminate', TerminateConfirmModal);
  check('A4 结束进程确认渲染成功', termHtml !== null);
  check('A4 确认框给出数据损坏风险提示（PRD A4 边界）',
    /进度丢失|损坏/.test(termHtml ?? ''));
}

console.log('\n[4] 数据必须真的渲染出来（防止「渲染成功但内容为空」）');
{
  check('首页渲染出 5 款游戏名', /鸣潮/.test(homeHtml ?? '') && /原神/.test(homeHtml ?? ''));
  check('首页显示需处理计数（来自 Core 的 summary）',
    /个游戏需要处理/.test(homeHtml ?? ''), '应显示 2 个游戏需要处理');
  check('首页渲染出「只看待处理」入口', /只看待处理/.test(homeHtml ?? ''));
  check('首页未安装游戏显示「未安装」', /未安装/.test(homeHtml ?? ''));
  check('鸣潮页显示 L1 风险标签', /L1 配置修改/.test(wuwaHtml ?? ''));
  check('鸣潮页显示归一化版本 3.5', /3\.5/.test(wuwaHtml ?? ''));
  check('原神页显示 L3 风险标签', /L3 进程级/.test(genshinHtml ?? ''));
  check('原神页说明备份「不适用」（A8 范围）', /不适用/.test(genshinHtml ?? ''));
  check('通用页显示「暂不支持配置备份」（01 A8 验收要点）',
    /暂不支持/.test(genericHtml ?? ''));
  check('通用页显示启动参数卡片（A7）', /启动参数/.test(genericHtml ?? ''));
  check('配置历史渲染出备份条目与撤销承诺（03 §5.7）',
    /任何恢复操作都可撤销/.test(historyHtml ?? ''));
  check('配置历史渲染出实际备份记录', /LocalStorage\.db/.test(historyHtml ?? ''));
  check('设置页声明安全机制恒为开启（04 §7.4）', /不提供关闭选项/.test(settingsHtml ?? ''));
}

console.log('\n[5] 已删除的错误内容不得回归（docs/05 §3 / docs/ipc-contract 分层）');
{
  const all = Object.values(htmlByView).join('\n');
  const FORBIDDEN = [
    // C1–C4：安全剧场（00 §8.11 不允许担保安全）
    ['核心环境监控', '虚构的 Kernel Bridge 模块'],
    ['Kernel Bridge', '虚构的 Kernel Bridge 模块'],
    ['旁路注入保护', '虚构的保护机制'],
    ['驱动重置', '虚构的驱动模块'],
    ['诊断报告', '诊断中心是 P1 且不作发布门槛'],
    ['反作弊纯净合规', '构成安全担保（违反 00 §8.11）'],
    ['ACE/TP', '反作弊合规声明'],
    ['mhyprot2', '虚构的反作弊观察能力'],
    ['Zero-Trace', '「无痕」语义是规避检测，与透明原则对立'],
    ['无痕挂接', '同上'],
    ['KERNEL_READY', '虚构的运行时遥测'],
    ['ZERO_RUNTIME_HOOKS', '虚构的运行时遥测'],
    ['PASSIVE_L1', '虚构的安全分级'],
    ['CRC CHECK', '虚构的校验指标'],
    ['127.0.0.1', 'IPC 不是 TCP 端口（虚构协议）'],
    ['LATENCY', '虚构的延迟指标'],
    // C10–C13：虚构指标
    ['节点已验证', '不存在的指标'],
    ['客户端完整度', '不存在的指标'],
    ['1.4 GB', 'E1 无法得到补丁体积'],
    ['更新并启动', '暗示自研下载引擎（01 §4 明确不做）'],
    ['差分存储', '备份是全量快照，不是差分'],
    ['镜像签名', '不存在的机制'],
    ['隔离副本', '不存在的机制'],
    // 已废弃的写入语义
    ['写入偏移量', '二进制写入语义，与 SQLite 键写入不符'],
    ['0x004F2', '同上'],
    ['FrameRateLimit', '不存在的键名（真实键为 CustomFrameRate）'],
  ];

  let leaked = 0;
  for (const [needle, why] of FORBIDDEN) {
    if (all.includes(needle)) {
      reportFail(`渲染产物中出现了「${needle}」—— ${why}`);
      leaked++;
    }
  }
  if (leaked === 0) {
    check(`已删除的 ${FORBIDDEN.length} 项错误内容均未回归`, true);
  }
}

console.log('\n[6] 两维状态模型与 U8 分层必须体现在渲染产物中');
{
  const wuwa = wuwaHtml ?? '';
  // 鸣潮 live 3.x → 工具 unknown（B6 的 aha 场景）
  check('工具未验证态被显式呈现（而非「已验证」）', /未验证/.test(wuwa));
  check('未验证态给出行动建议（02 B6 要求）',
    /等待验证结果|恢复到修改前的配置/.test(wuwa));
  check('时长按「小时/分钟」呈现并声明口径', /小时|分钟/.test(wuwa));
  check('首页声明时长口径（02 A6：UI 必须明示）',
    /游戏进程存活时长/.test(homeHtml ?? ''));

  // U8：底层术语只允许出现在「高级详情」层级内
  const defaultLayerLeak = ['CustomFrameRate', 'GameQualitySetting', 'a3f9c2'].filter((t) =>
    wuwa.includes(t),
  );
  check(
    '鸣潮页默认层不出现底层键名/哈希（03 U8）',
    defaultLayerLeak.length === 0,
    `需要「高级详情」展开后才应出现：${defaultLayerLeak.join(', ')}`,
  );

  // 展开高级详情后应能看到底层事实
  check('配置历史以弱化次行展示主文件（允许的 Advanced 层）',
    /LocalStorage\.db/.test(historyHtml ?? ''));
}

if (process.env.ORBIS_SMOKE_DUMP) {
  for (const [name, html] of Object.entries(htmlByView)) {
    writeFileSync(join(outDir, `${name}.html`), html);
  }
  console.log(`\n[debug] 渲染产物已转储到 node_modules/.cache/orbis-smoke/`);
}

if (!process.env.ORBIS_SMOKE_KEEP) {
  rmSync(outDir, { recursive: true, force: true });
}

console.log(`\n结果：${pass} 通过 / ${failCount} 失败\n`);
process.exit(failCount === 0 ? 0 : 1);
