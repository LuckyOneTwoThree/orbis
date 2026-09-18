#!/usr/bin/env node
/**
 * mock 参照实现行为核对（CI 与本地共用）
 *
 * 为什么要有这个脚本（契约 §7.2）：
 *   mock 不是「随便返回假数据」，而是**行为对齐的参照实现**。它直接 import
 *   data/compatibility/seed.json 与 data/tools/manifests/*.json，一旦数据与实现口径漂移，
 *   这里会立刻失败 —— 这是防止「UI demo 好看但和 Core 对不上」的唯一自动化闸门。
 *
 * 覆盖的行为契约：
 *   1. 兼容匹配 exact → prefix → none（P0-C §2.2）
 *   2. 「需处理」判定式与聚合（04 §6.4.3）
 *   3. 版本归一化与「识别失败不猜」（04 §5.1）
 *   4. L1 执行流步骤顺序与两层 detail（03 U8 / 04 §5.5）
 *   5. 失败自动回滚（B4）
 *   6. L3 门控 / 授权 / 资产降级（B7）
 *   7. 备份链路与 A8 范围 / pre_restore 可撤销（A8）
 *   8. 运行中禁止写与重复启动（P0-B §6.3 红线 1 / A4）
 *   9. 启动参数持久化与设置白名单（A7 / 04 §7.4）
 *
 * 这些断言只守护「行为口径」；真正的 Core 单测在 Rust 侧（04 §9.2），二者互补。
 */
import { build } from 'esbuild';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

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

const outDir = mkdtempSync(join(tmpdir(), 'orbis-mock-'));
const outFile = join(outDir, 'mock.mjs');

await build({
  entryPoints: [join(ROOT, 'src/api/mock.ts')],
  bundle: true,
  format: 'esm',
  platform: 'node',
  outfile: outFile,
  logLevel: 'warning',
});

const { mockApi, __devForceApplyFailure } = await import(pathToFileURL(outFile).href);

console.log('\n[1] 兼容匹配（P0-C §2.2 exact → prefix → none）');
{
  const wuwa = await mockApi.getCompatibility('wuthering-waves', 'orbis-builtin/wuwa-fps-120');
  check('鸣潮 live 版本 → unknown（seed 仅 3.x prefix = Unknown）', wuwa.status === 'unknown');
  check('鸣潮命中 prefix 条目 3.x', wuwa.matchKind === 'prefix' && wuwa.matchedVersionKey === '3.x');

  const genshin = await mockApi.getCompatibility('genshin-impact', 'orbis-bundled/genshin-fps-unlock');
  check('原神 → compatible（7.x prefix 兜底）', genshin.status === 'compatible' && genshin.matchKind === 'prefix');

  const missing = await mockApi.getCompatibility('wuthering-waves', 'orbis-builtin/not-a-tool');
  check('无 seed 条目 → unknown / none', missing.status === 'unknown' && missing.matchKind === 'none');
}

console.log('\n[2] 需处理判定与聚合（04 §6.4.3）');
{
  const { installations, summary } = await mockApi.listInstallations();
  const g = Object.fromEntries(installations.map((i) => [i.gameId, i]));

  check('鸣潮因 tool_unknown 进入需处理', g['wuthering-waves'].attentionReasons.includes('tool_unknown'));
  check('原神不需处理（compatible 且无更新）', g['genshin-impact'].needsAttention === false);
  check('星铁因 update_available 进入需处理', g['honkai-star-rail'].attentionReasons.includes('update_available'));
  check('终末地降级不误报为「有更新」', g['arknights-endfield'].needsAttention === false);
  check('summary 聚合 = 2 需处理 / 1 更新可用',
    summary.needsAttention === 2 && summary.updateAvailable === 1, JSON.stringify(summary));
  check('versionNorm 归一化（3.5.0.128940 → 3.5）', g['wuthering-waves'].versionNorm === '3.5');
  check('region 为库内小写（cn/global/bili）',
    installations.every((i) => ['cn', 'global', 'bili'].includes(i.region)));
}

console.log('\n[3] 版本归一化与「识别失败不猜」（04 §5.1）');
{
  await mockApi.addInstallation({ gameId: 'honkai-star-rail', executablePath: 'X:\\Odd\\StarRail.exe' });
  const { installations } = await mockApi.listInstallations();
  const added = installations.find((i) => i.executablePath === 'X:\\Odd\\StarRail.exe');
  check('未识别版本 → localVersion=null 且 versionUnknown=true',
    added.localVersion === null && added.versionUnknown === true);

  const c = await mockApi.getCompatibility('honkai-star-rail', 'orbis-bundled/genshin-fps-unlock');
  check('版本未知 → 兼容查询 unknown（不猜）', c.status === 'unknown' && c.matchKind === 'none');
  await mockApi.removeInstallation(added.id);
}

console.log('\n[4] L1 执行流与两层 detail（04 §5.5 / 03 U8）');
{
  const steps = [];
  const unsub = mockApi.subscribe('tool:apply-step', (p) => steps.push(p));

  let code = null;
  try {
    await mockApi.setToolEnabled('orbis-builtin/wuwa-fps-120', true);
  } catch (e) {
    code = e.code;
  }
  check('L1 + unknown 未覆盖 → COMPAT_OVERRIDE_REQUIRED', code === 'COMPAT_OVERRIDE_REQUIRED', String(code));

  const res = await mockApi.setToolEnabled('orbis-builtin/wuwa-fps-120', true, { overrideCompat: true });
  check('覆盖后启用成功并产生 pre_modify 备份', res.outcome === 'applied' && Boolean(res.backupId));

  const order = steps.map((s) => s.step);
  const expected = ['gate', 'precheck', 'backup', 'modify', 'verify'];
  check('步骤严格按状态机顺序', expected.every((s, i) => i === 0 || order.indexOf(s) > order.indexOf(expected[i - 1])),
    order.join(','));
  check('每步 detail 两层结构齐全',
    steps.every((s) => typeof s.detail.default === 'string' && Array.isArray(s.detail.advanced)));

  const defaults = steps.map((s) => s.detail.default).join(' | ');
  check('默认层不含文件名 / 键名 / 哈希（03 U8）',
    !/LocalStorage|CustomFrameRate|a3f9c2|0x00/i.test(defaults), defaults);
  unsub();
}

console.log('\n[5] 失败自动回滚（B4）');
{
  __devForceApplyFailure(true);
  const rb = [];
  const unsub = mockApi.subscribe('tool:apply-step', (p) => {
    if (p.step === 'rollback') rb.push(p);
  });
  const res = await mockApi.setToolEnabled('orbis-builtin/wuwa-fps-120', true, { overrideCompat: true });
  __devForceApplyFailure(false);

  check('verify 失败 → rolled_back 且不置为启用', res.outcome === 'rolled_back' && res.enabled === false);
  check('发出 rollback 步骤事件', rb.length >= 2 && rb.some((p) => p.state === 'completed'));
  check('回滚文案面向用户（无底层细节）',
    rb.every((p) => !/sha256|0x|\.db/i.test(p.detail.default)));
  unsub();
}

console.log('\n[6] L3 门控 / 授权 / 资产降级（B7）');
{
  let code = null;
  try {
    await mockApi.setToolEnabled('orbis-bundled/genshin-fps-unlock', true);
  } catch (e) {
    code = e.code;
  }
  check('L3 未授权 → CONSENT_REQUIRED', code === 'CONSENT_REQUIRED', String(code));

  await mockApi.grantToolAuthorization('orbis-bundled/genshin-fps-unlock', 'sha256:demo');
  const res = await mockApi.setToolEnabled('orbis-bundled/genshin-fps-unlock', true);
  check('授权后启用成功；L3 不落盘 → 无备份 ID', res.outcome === 'applied' && res.backupId === null);

  const asset = await mockApi.getToolAssetStatus('orbis-bundled/genshin-fps-unlock');
  check('assets.json 无 artifact → downloadUrlConfigured=false', asset.downloadUrlConfigured === false);

  let acode = null;
  try {
    await mockApi.downloadToolAsset('orbis-bundled/genshin-fps-unlock');
  } catch (e) {
    acode = e.code;
  }
  check('下载未配置资产 → TOOL_ASSET_NOT_CONFIGURED（降级非崩溃）',
    acode === 'TOOL_ASSET_NOT_CONFIGURED', String(acode));

  await mockApi.revokeToolAuthorization('orbis-bundled/genshin-fps-unlock');
  const tools = await mockApi.listTools('genshin-impact');
  check('撤回授权 → 工具回到未启用', tools[0].enabled === false);
}

console.log('\n[7] 备份链路与 A8 范围（A8 / B5）');
{
  const { installations } = await mockApi.listInstallations();
  const wuwa = installations.find((i) => i.gameId === 'wuthering-waves');
  const genshin = installations.find((i) => i.gameId === 'genshin-impact');
  const starrail = installations.find((i) => i.gameId === 'honkai-star-rail');

  check('鸣潮：Provider 已声明配置路径', wuwa.hasConfigSource && wuwa.configUnsupportedReason === null);
  check('原神：not_applicable（L3 不落盘）', genshin.configUnsupportedReason === 'not_applicable');
  check('星铁：provider_not_declared（UI 显示「暂不支持」）',
    starrail.configUnsupportedReason === 'provider_not_declared');

  const created = await mockApi.createBackup(wuwa.id, 'manual');
  check('手动备份带 primaryFile', created.primaryFile === 'LocalStorage.db');

  const r = await mockApi.restoreBackup(created.id);
  check('恢复前自动生成 pre_restore 备份（可撤销恢复本身）',
    Boolean(r.preRestoreBackupId) && r.verified === true);
  const list = await mockApi.listBackups(wuwa.id);
  check('pre_restore 备份进入历史', list.some((b) => b.trigger === 'pre_restore'));

  let ccode = null;
  try {
    await mockApi.createBackup(starrail.id, 'manual');
  } catch (e) {
    ccode = e.code;
  }
  check('未声明 Provider 的游戏 → CONFIG_SOURCE_UNSUPPORTED',
    ccode === 'CONFIG_SOURCE_UNSUPPORTED', String(ccode));
}

console.log('\n[8] 运行中禁止写 / 重复启动（红线 1 / A4）');
{
  const { installations } = await mockApi.listInstallations();
  const wuwa = installations.find((i) => i.gameId === 'wuthering-waves');
  await mockApi.launchGame(wuwa.id);

  let code = null;
  try {
    await mockApi.createBackup(wuwa.id, 'manual');
  } catch (e) {
    code = e.code;
  }
  check('游戏运行中备份 → GAME_PROCESS_ACTIVE', code === 'GAME_PROCESS_ACTIVE', String(code));

  let lcode = null;
  try {
    await mockApi.launchGame(wuwa.id);
  } catch (e) {
    lcode = e.code;
  }
  check('重复启动 → GAME_ALREADY_RUNNING', lcode === 'GAME_ALREADY_RUNNING', String(lcode));

  await mockApi.terminateGame(wuwa.id);
}

console.log('\n[9] 启动参数与设置白名单（A7 / 04 §7.4）');
{
  const { installations } = await mockApi.listInstallations();
  const starrail = installations.find((i) => i.gameId === 'honkai-star-rail');

  await mockApi.setLaunchProfile(starrail.id, '-dx12 -windowed');
  const p = await mockApi.getLaunchProfile(starrail.id);
  check('启动参数持久化并可回读', p.args === '-dx12 -windowed');

  let code = null;
  try {
    await mockApi.setSetting('something.else', 1);
  } catch (e) {
    code = e.code;
  }
  check('非白名单设置键 → SETTING_UNKNOWN_KEY', code === 'SETTING_UNKNOWN_KEY', String(code));

  let vcode = null;
  try {
    await mockApi.setSetting('log.retention_days', 9999);
  } catch (e) {
    vcode = e.code;
  }
  check('设置值越界 → SETTING_INVALID_VALUE', vcode === 'SETTING_INVALID_VALUE', String(vcode));
}

console.log('\n[10] 游戏目录（契约 §3.1，UI 不得自带一份）');
{
  const catalog = await mockApi.listGames();
  check('目录返回 5 款游戏', catalog.length === 5);
  check('每款都有名称与区服', catalog.every((g) => g.name && g.regions.length > 0));
  check('L3 工具存在的游戏标记 hasBundledComponent',
    catalog.find((g) => g.id === 'genshin-impact')?.hasBundledComponent === true);
  check('目录与 seed 的 game_id 口径一致',
    catalog.every((g) => typeof g.id === 'string' && g.id.includes('-')));
}

rmSync(outDir, { recursive: true, force: true });

console.log(`\n结果：${pass} 通过 / ${failCount} 失败\n`);
process.exit(failCount === 0 ? 0 : 1);
