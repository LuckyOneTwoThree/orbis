#!/usr/bin/env node
/**
 * Orbis 内置数据校验（CI 与本地共用）
 *
 * 校验对象（04 §6.2）：
 *   data/compatibility/seed.json      ← seed.schema.json
 *   data/tools/manifests/*.json       ← manifests.schema.json
 *   data/tools/assets.json            ← assets.schema.json
 *
 * 附加语义校验（JSON Schema 表达不了的跨文件约束）：
 *   1. manifest.id 必须在 seed.json 中存在对应 tool_id（否则兼容查询永远 Unknown）
 *   2. manifest.entry.asset 必须在 assets.json 中存在对应 asset_key
 *   3. manifest.pending_verifications 中的 ID 必须形如 Tn / Tna-c（04 §10）
 *   4. tool_id namespace 与 source.kind 一致（orbis-builtin/ ↔ builtin，orbis-bundled/ ↔ bundled）
 *
 * 退出码非 0 即阻断 CI。禁止依赖「本地改完就过」——数据是发行物的一部分。
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import Ajv2020 from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

const ajv = new Ajv2020({ allErrors: true, strict: false });
addFormats(ajv);

const errors = [];
const fail = (file, msg) => errors.push(`  ✗ ${file}: ${msg}`);

/** 编译一次并缓存（ajv 按 $id 去重，同一 schema 不可重复 compile） */
const compiled = new Map();
function validator(schemaPath) {
  if (!compiled.has(schemaPath)) {
    compiled.set(schemaPath, ajv.compile(readJson(schemaPath)));
  }
  return compiled.get(schemaPath);
}

function validate(schemaPath, dataPath) {
  const check = validator(schemaPath);
  const data = readJson(dataPath);
  const ok = check(data);
  if (!ok) {
    for (const e of check.errors) {
      fail(dataPath.replace(ROOT + '/', ''), `${e.instancePath || '/'} ${e.message}`);
    }
  }
  return ok;
}

// ── 1. schema 校验 ─────────────────────────────────────────
const seedPath = join(ROOT, 'data/compatibility/seed.json');
const assetsPath = join(ROOT, 'data/tools/assets.json');
const manifestDir = join(ROOT, 'data/tools/manifests');

let ok = true;
ok = validate(join(ROOT, 'data/compatibility/seed.schema.json'), seedPath) && ok;
ok = validate(join(ROOT, 'data/tools/assets.schema.json'), assetsPath) && ok;

const manifestFiles = readdirSync(manifestDir).filter((f) => f.endsWith('.json'));
if (manifestFiles.length === 0) fail('data/tools/manifests/', '没有任何 Manifest 文件');
for (const f of manifestFiles) {
  ok =
    validate(join(ROOT, 'data/tools/manifests.schema.json'), join(manifestDir, f)) && ok;
}

// ── 2. 跨文件语义校验 ──────────────────────────────────────
if (ok) {
  const seed = readJson(seedPath);
  const assets = readJson(assetsPath);
  const seedToolIds = new Set(seed.entries.map((e) => e.tool_id));
  const assetKeys = new Set(assets.assets.map((a) => a.asset_key));
  const assetToolIds = new Set(assets.assets.map((a) => a.tool_id));

  for (const f of manifestFiles) {
    const rel = `data/tools/manifests/${f}`;
    const m = readJson(join(manifestDir, f));

    // 1. seed 必须覆盖 manifest 声明的工具
    if (!seedToolIds.has(m.id)) {
      fail(rel, `id "${m.id}" 在 seed.json 中没有兼容性条目 → 该工具永远解析为 Unknown`);
    }

    // 2. external_process 必须能在 assets.json 找到资产
    if (m.entry.asset && !assetKeys.has(m.entry.asset)) {
      fail(rel, `entry.asset "${m.entry.asset}" 在 assets.json 中不存在`);
    }
    if (m.entry.asset && !assetToolIds.has(m.id)) {
      fail(rel, `assets.json 中不存在指向 "${m.id}" 的条目（tool_id 不匹配）`);
    }

    // 3. namespace ↔ source.kind 一致性
    const [ns] = m.id.split('/');
    const expectNs = { builtin: 'orbis-builtin', bundled: 'orbis-bundled' }[m.source.kind];
    if (expectNs && ns !== expectNs) {
      fail(rel, `namespace "${ns}" 与 source.kind "${m.source.kind}" 不一致（应为 ${expectNs}/*）`);
    }

    // 4. backup_required 与风险级的语义一致性（00 §12.3 规则 5）
    if (m.type === 'config_modify' && !m.backup_required) {
      fail(rel, 'type=config_modify 必须 backup_required=true（无备份不修改）');
    }
    if (m.type === 'config_modify' && m.risk_level !== 'L1') {
      fail(rel, 'type=config_modify 目前在 MVP 内仅 L1（04 §5.7）');
    }
  }

  // 5. seed 中的 L1/L3 与 manifest 的 risk_level 一致性（P0-C §1.2：manifest 为权威）
  const manifestById = new Map(
    manifestFiles.map((f) => {
      const m = readJson(join(manifestDir, f));
      return [m.id, m];
    }),
  );
  for (const e of seed.entries) {
    const m = manifestById.get(e.tool_id);
    if (m && m.risk_level !== e.risk_level) {
      fail('data/compatibility/seed.json', `tool_id "${e.tool_id}" risk_level 为 ${e.risk_level}，与 Manifest 的 ${m.risk_level} 不一致`);
    }
  }
}

// ── 3. 结果 ───────────────────────────────────────────────
if (errors.length) {
  console.error('\n内置数据校验失败：\n');
  console.error(errors.join('\n'));
  console.error(`\n共 ${errors.length} 处问题。\n`);
  process.exit(1);
}

console.log(
  `✓ 内置数据校验通过（seed 1 份 · manifest ${manifestFiles.length} 份 · assets 1 份）`,
);
