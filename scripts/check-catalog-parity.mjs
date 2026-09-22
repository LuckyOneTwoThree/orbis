#!/usr/bin/env node
/**
 * 游戏目录双源一致性检查（契约 §3.1 / 00 §12.3 规则 1）
 *
 * 契约 §3.1 规定游戏目录只有**一个权威来源**（Core 的 `listGames()`），
 * 而 `src/api/mock.ts` 作为「行为对齐的参照实现」（契约 §7.2）必须自带一份 ——
 * 于是同一份数据存在于两处，且**没有任何机制保证它们继续一致**。
 *
 * 为什么要在前端侧读 Rust 源码：这类漂移只有「同时看见两边」的检查点才能抓到。
 * 藏在 views/store 里的副本由 `check-architecture.sh` 的 [8] 拦；这里补的是
 * 「mock 参照实现 vs Core 权威数据」这一对，二者都不该被改动而不通知另一方。
 *
 * 三方对齐（缺一不可）：
 *   1. crates/orbis-providers/src/catalog.rs  ← 权威数据（编译期常量）
 *   2. src/api/mock.ts 的 GAME_CATALOG        ← 参照实现（无 Rust 时 UI 的唯一数据源）
 *   3. data/tools/manifests/*.json            ← hasBundledComponent 的真实来源
 *
 * 第 3 项的意义：`hasBundledComponent` 在 Rust 侧是**派生值**
 * （`dto.rs`：`manifests.for_game(id).any(|m| m.requires_asset())`，
 * `requires_asset()` = `entry.asset.is_some()`），而 mock 侧是**硬编码字面量**。
 * 若只比对 1 与 2，二者可能一起错；所以期望值要按 manifest 数据**现算**再比对 ——
 * 这样「改了一个 manifest，忘了改 mock」会立刻失败。
 *
 * 退出码非 0 即阻断 CI。
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const RUST_CATALOG = join(ROOT, 'crates/orbis-providers/src/catalog.rs');
const MOCK_CATALOG = join(ROOT, 'src/api/mock.ts');
const MANIFEST_DIR = join(ROOT, 'data/tools/manifests');

const errors = [];
const fail = (msg) => errors.push(`  ✗ ${msg}`);

// ── 1. Rust 权威目录 ──────────────────────────────────────
//
// 解析的是源码文本而不是运行 Rust：这是本脚本的固有代价（要在前端闸门里
// 同时看见两边）。为降低脆弱性，只依赖固定的字段顺序与 `GameEntry { … },` 形状；
// 一旦 catalog.rs 的写法变化导致解析出的条目数与预期不符，脚本会直接报错而不是静默通过。
function parseRustCatalog(src) {
  const entries = [];
  for (const m of src.matchAll(/GameEntry\s*\{([\s\S]*?)\n    \}/g)) {
    const body = m[1];
    const text = (key) => {
      const hit = new RegExp(`${key}:\\s*"([^"]*)"`).exec(body);
      return hit ? hit[1] : null;
    };
    const url = /official_url:\s*Some\("([^"]*)"\)/.exec(body);
    const regions = /regions:\s*&\[([^\]]*)\]/.exec(body);
    entries.push({
      id: text('id'),
      name: text('name'),
      enName: text('en_name'),
      publisher: text('publisher'),
      regions: regions ? [...regions[1].matchAll(/Region::(\w+)/g)].map((r) => r[1].toLowerCase()) : [],
      officialUrl: url ? url[1] : null,
    });
  }
  return entries;
}

// ── 2. mock 参照实现 ──────────────────────────────────────
function parseMockCatalog(src) {
  const start = src.indexOf('const GAME_CATALOG');
  if (start < 0) throw new Error('src/api/mock.ts 里找不到 GAME_CATALOG');
  const body = src.slice(src.indexOf('[', start), src.indexOf('\n];', start));
  // 先剥掉**行首**注释：注释里出现引号或花括号会让下面的对象切分错位。
  // 注意不能写成剥「任意 `//`」—— 那会把 `https://` 也截断（对象切分随之静默变形）。
  const clean = body.replace(/^\s*\/\/.*$/gm, '');
  return [...clean.matchAll(/\{([\s\S]*?)\}/g)].map((m) => {
    const o = m[1];
    const text = (key) => {
      const hit = new RegExp(`${key}:\\s*'([^']*)'`).exec(o);
      return hit ? hit[1] : null;
    };
    const list = (key) => {
      const hit = new RegExp(`${key}:\\s*\\[([^\\]]*)\\]`).exec(o);
      return hit ? [...hit[1].matchAll(/'([^']*)'/g)].map((x) => x[1]) : [];
    };
    const bool = (key) => {
      const hit = new RegExp(`${key}:\\s*(true|false)`).exec(o);
      return hit ? hit[1] === 'true' : null;
    };
    return {
      id: text('id'),
      name: text('name'),
      enName: text('enName'),
      publisher: text('publisher'),
      regions: list('regions'),
      officialUrl: text('officialUrl'),
      hasBundledComponent: bool('hasBundledComponent'),
    };
  });
}

// ── 3. hasBundledComponent 的期望值（按 manifest 现算）──────
function bundledComponentByGame() {
  const expected = new Map();
  for (const file of readdirSync(MANIFEST_DIR)) {
    if (!file.endsWith('.json')) continue;
    const manifest = JSON.parse(readFileSync(join(MANIFEST_DIR, file), 'utf8'));
    // 与 `manifest.rs::requires_asset()` 同口径：entry.asset 存在即「需额外组件」
    const needsAsset = manifest.entry?.asset != null;
    expected.set(manifest.game, (expected.get(manifest.game) ?? false) || needsAsset);
  }
  return expected;
}

// ── 比对 ──────────────────────────────────────────────────
const rust = parseRustCatalog(readFileSync(RUST_CATALOG, 'utf8'));
const mock = parseMockCatalog(readFileSync(MOCK_CATALOG, 'utf8'));
const expectedBundled = bundledComponentByGame();

if (rust.length === 0) {
  fail(`${RUST_CATALOG} 解析出 0 条 GameEntry —— 解析器与源码形状已不匹配，本检查失效`);
}

const rustIds = rust.map((e) => e.id);
const mockIds = mock.map((e) => e.id);
if (rustIds.length !== mockIds.length) {
  fail(`条目数不一致：catalog.rs ${rustIds.length} 条，mock.ts ${mockIds.length} 条`);
}
if (rustIds.join(',') !== mockIds.join(',')) {
  fail(`条目顺序或集合不一致：\n      catalog.rs: ${rustIds.join(', ')}\n      mock.ts   : ${mockIds.join(', ')}`);
  if (rustIds.length !== mockIds.length) {
    // 集合都对不上时逐字段比没有意义，直接给出结论
    console.error('\n游戏目录一致性检查失败：\n');
    console.error(errors.join('\n'));
    console.error(`\n共 ${errors.length} 处问题。\n`);
    process.exit(1);
  }
}

const FIELDS = [
  ['name', '显示名'],
  ['enName', '英文名'],
  ['publisher', '发行商'],
  ['officialUrl', '官方入口'],
];

let checked = 0;
for (const [i, r] of rust.entries()) {
  const m = mock[i];
  for (const [field, label] of FIELDS) {
    if (r[field] !== m[field]) {
      fail(`${r.id} 的${label}（${field}）不一致：catalog.rs = ${JSON.stringify(r[field])}，mock.ts = ${JSON.stringify(m[field])}`);
    }
    checked++;
  }
  const rRegions = [...r.regions].sort().join(',');
  const mRegions = [...m.regions].sort().join(',');
  if (rRegions !== mRegions) {
    fail(`${r.id} 的区服不一致：catalog.rs = [${rRegions}]，mock.ts = [${mRegions}]`);
  }
  checked++;

  // hasBundledComponent：期望值来自 manifest 数据，而不是另外两份代码中的任一份
  const want = expectedBundled.get(r.id) ?? false;
  if (m.hasBundledComponent !== want) {
    fail(
      `${r.id} 的 hasBundledComponent 与 manifest 数据不符：` +
        `mock.ts = ${m.hasBundledComponent}，按 data/tools/manifests/*.json 应为 ${want}` +
        `（Rust 侧由 entry.asset 派生，mock 侧是硬编码 —— 改了 manifest 就要同步这一处）`,
    );
  }
  checked++;
}

// ── 结果 ──────────────────────────────────────────────────
if (errors.length) {
  console.error('\n游戏目录一致性检查失败：\n');
  console.error(errors.join('\n'));
  console.error(`\n共 ${errors.length} 处问题。\n`);
  process.exit(1);
}

console.log(
  `✓ 游戏目录双源一致（providers/catalog.rs ↔ mock.ts，${rust.length} 款 × ${checked / rust.length} 项；` +
    `hasBundledComponent 已按 manifest 数据核对）`,
);
