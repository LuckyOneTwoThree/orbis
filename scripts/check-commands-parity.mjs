#!/usr/bin/env node
/**
 * 命令面三处一致性检查（契约 §3 / §7.1）
 *
 * 同一份「命令清单」在三个地方各有一份表示，任何一处漏改都会静默失真：
 *   1. `docs/ipc-contract.md` §3  —— 权威定义（冻结）
 *   2. `src/api/tauri.ts` 的 invokeCmd 调用 —— 前端接线（参数名在此固化）
 *   3. `src-tauri/src/lib.rs` 的 `generate_handler!` —— Core 实际注册
 *
 * 另外还比对 `src/api/tauri.ts` 的 `IMPLEMENTED_COMMANDS` 白名单与 (3)：
 * 契约 §7.1 把「未实现命令抛 NOT_IMPLEMENTED」的责任放在 tauri.ts，于是白名单成了
 * 第四份表示。它必须与 Core 真正注册的命令**完全相等** —— 少了会让已实现的命令被误报
 * 成「未实现」，多了会让未实现的命令得不到契约要求的错误码。两者都是静默故障。
 *
 * 本检查同时钉住「命令总数」：文档里出现的计数（契约正文、README、模块注释）
 * 曾长期停留在 27/28，而实际是 32。数字写死必然 stale，故由断言守。
 *
 * 退出码非 0 即阻断 CI。
 */
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const CONTRACT = join(ROOT, 'docs/ipc-contract.md');
const TAURI_TS = join(ROOT, 'src/api/tauri.ts');
const LIB_RS = join(ROOT, 'src-tauri/src/lib.rs');

const errors = [];
const fail = (msg) => errors.push(`  ✗ ${msg}`);

const uniq = (xs) => [...new Set(xs)];

// ── 1. 契约 §3 的命令签名 ─────────────────────────────────
const contract = readFileSync(CONTRACT, 'utf8');
const section3 = /\n## 3\..*?(?=\n## 4\.)/s.exec(contract)?.[0];
if (!section3) {
  fail('docs/ipc-contract.md 里找不到 §3（检查标题格式是否变化）');
}
const contractCommands = uniq(
  [...(section3 ?? '').matchAll(/^\s{0,2}([a-z][A-Za-z0-9]*)\([^)]*\)\s*:\s*Promise</gm)].map((m) => m[1]),
);

// ── 2. tauri.ts 的 invokeCmd 调用 ─────────────────────────
const tauriTs = readFileSync(TAURI_TS, 'utf8');
const wiredCommands = uniq(
  [...tauriTs.matchAll(/\binvokeCmd(?:<[^>]*>)?\(\s*'([a-zA-Z][A-Za-z0-9]*)'/g)].map((m) => m[1]),
);

// ── 3. IMPLEMENTED_COMMANDS 白名单 ────────────────────────
const whitelistBlock = /const IMPLEMENTED_COMMANDS[^=]*=\s*\[([\s\S]*?)\]/.exec(tauriTs)?.[1];
if (whitelistBlock === undefined) {
  fail('src/api/tauri.ts 里找不到 IMPLEMENTED_COMMANDS（契约 §7.1 要求它存在）');
}
const declaredImplemented = uniq(
  [...(whitelistBlock ?? '').matchAll(/'([a-zA-Z][A-Za-z0-9]*)'/g)].map((m) => m[1]),
);

// ── 4. lib.rs 的 generate_handler! ────────────────────────
const libRs = readFileSync(LIB_RS, 'utf8');
const handlerBlock = /generate_handler!\[([\s\S]*?)\]/.exec(libRs)?.[1];
if (handlerBlock === undefined) {
  fail('src-tauri/src/lib.rs 里找不到 generate_handler!');
}
const registered = uniq(
  [...(handlerBlock ?? '').matchAll(/([a-z][A-Za-z0-9]*)\s*,/g)].map((m) => m[1]),
);

// ── 比对 ──────────────────────────────────────────────────
const sortedEq = (a, b) => a.length === b.length && [...a].sort().join(',') === [...b].sort().join(',');

if (contractCommands.length === 0) {
  fail('契约 §3 解析出 0 条命令 —— 解析器与文档格式已不匹配，本检查失效');
}

if (!sortedEq(contractCommands, wiredCommands)) {
  const onlyContract = contractCommands.filter((c) => !wiredCommands.includes(c));
  const onlyWired = wiredCommands.filter((c) => !contractCommands.includes(c));
  fail(
    `契约 §3 与 tauri.ts 的接线不一致（契约 ${contractCommands.length} 条 / 接线 ${wiredCommands.length} 条）` +
      (onlyContract.length ? `\n      仅在契约中：${onlyContract.join(', ')}` : '') +
      (onlyWired.length ? `\n      仅在接线中：${onlyWired.join(', ')}` : ''),
  );
}

if (!sortedEq(declaredImplemented, registered)) {
  const notRegistered = declaredImplemented.filter((c) => !registered.includes(c));
  const notDeclared = registered.filter((c) => !declaredImplemented.includes(c));
  fail(
    `IMPLEMENTED_COMMANDS 与 generate_handler! 不一致（白名单 ${declaredImplemented.length} 条 / 已注册 ${registered.length} 条）` +
      (notRegistered.length ? `\n      白名单有但 Core 未注册（会被误报为未实现）：${notRegistered.join(', ')}` : '') +
      (notDeclared.length ? `\n      Core 已注册但白名单没有（拿不到正确错误码）：${notDeclared.join(', ')}` : ''),
  );
}

// ── 结果 ──────────────────────────────────────────────────
if (errors.length) {
  console.error('\n命令面一致性检查失败：\n');
  console.error(errors.join('\n'));
  console.error(`\n共 ${errors.length} 处问题。\n`);
  process.exit(1);
}

console.log(
  `✓ 命令面一致（契约 §3 ${contractCommands.length} 条 = 前端接线；` +
    `已实现 ${registered.length}/${contractCommands.length} 条：IMPLEMENTED_COMMANDS = generate_handler!）`,
);
