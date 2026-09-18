#!/usr/bin/env bash
#
# 架构不变量断言（pm/00-产品基石.md §12.4 B 组 / pm/04-技术设计.md §9.1）
#
# 这些不变量是「新增游戏 / 新增工具不需要修改 Core」的机器可验证形式。
# 语言层面（Cargo 依赖方向）已由编译期保证，本脚本额外守护**源码级**约束。
#
# 可在本地直接运行：bash scripts/check-architecture.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failures=0
note() { printf '  %s\n' "$1"; }
fail() { failures=$((failures + 1)); printf '  ✗ %s\n' "$1"; }
ok() { printf '  ✓ %s\n' "$1"; }

echo ""
echo "[1] Core 不得包含游戏知识（00 §12.3 规则 1）"
# 5 个 game slug 不得出现在 orbis-core 的任何文件中
GAME_SLUGS=(
  "genshin-impact"
  "honkai-star-rail"
  "zenless-zone-zero"
  "wuthering-waves"
  "arknights-endfield"
)
core_hits=0
for slug in "${GAME_SLUGS[@]}"; do
  if grep -rIn --include='*.rs' --include='*.toml' -- "$slug" crates/orbis-core >/dev/null 2>&1; then
    fail "crates/orbis-core 中出现了游戏标识：$slug"
    grep -rIn --include='*.rs' --include='*.toml' -- "$slug" crates/orbis-core | sed 's/^/      /'
    core_hits=$((core_hits + 1))
  fi
done
[ "$core_hits" -eq 0 ] && ok "crates/orbis-core 无任何游戏标识"

echo ""
echo "[2] Core 不得依赖任何其它 orbis crate（04 §4.1）"
for dep in orbis-platform orbis-providers orbis-tools; do
  if grep -q "^${dep}" crates/orbis-core/Cargo.toml 2>/dev/null; then
    fail "crates/orbis-core 依赖了 $dep"
  fi
done
ok "crates/orbis-core 无内部依赖"

echo ""
echo "[3] providers 与 tools 之间禁止横向依赖（04 §4.1）"
if grep -q "^orbis-tools" crates/orbis-providers/Cargo.toml 2>/dev/null; then
  fail "orbis-providers 依赖了 orbis-tools"
else
  ok "orbis-providers 未依赖 orbis-tools"
fi
if grep -q "^orbis-providers" crates/orbis-tools/Cargo.toml 2>/dev/null; then
  fail "orbis-tools 依赖了 orbis-providers"
else
  ok "orbis-tools 未依赖 orbis-providers"
fi

echo ""
echo "[4] platform 不得反向依赖（04 §4.1）"
for dep in orbis-providers orbis-tools; do
  if grep -q "^${dep}" crates/orbis-platform/Cargo.toml 2>/dev/null; then
    fail "orbis-platform 依赖了 $dep"
  fi
done
ok "orbis-platform 只依赖 orbis-core"

echo ""
echo "[5] 前端分层：views/store 不得直连 IPC（00 §7.9 / 契约 §7.1）"
# 注意：正则刻意只用 POSIX 字符类（[[:space:]] 等）—— \s / \b 是 GNU 扩展，
# 在 macOS 的 BSD grep 下会静默不匹配，让断言形同虚设。
ipc_hits=$(grep -rIn --include='*.tsx' --include='*.ts' -E "from '@tauri-apps/api" src/views src/store 2>/dev/null || true)
if [ -n "$ipc_hits" ]; then
  fail "views/ 或 store/ 中直接引用了 Tauri API，应经 src/api："
  printf '%s\n' "$ipc_hits" | sed 's/^/      /'
else
  ok "views/ 与 store/ 未直接引用 Tauri API"
fi

# invoke( 左边是单词结尾即可 —— 无需 \b，因为 "invokeCmd(" 不会误命中
invoke_hits=$(grep -rIn --include='*.tsx' --include='*.ts' -E "invoke\(" src/views src/store 2>/dev/null || true)
if [ -n "$invoke_hits" ]; then
  fail "views/ 或 store/ 中出现了 invoke(...)，业务必须留在 Core："
  printf '%s\n' "$invoke_hits" | sed 's/^/      /'
else
  ok "views/ 与 store/ 未出现 invoke(...)"
fi

echo ""
echo "[6] 契约层不得引用 UI（依赖方向只能向内）"
ui_hits=$(grep -rIn --include='*.ts' -E "from '\.\./views|from '\.\./store|from '\.\./components" src/api 2>/dev/null || true)
if [ -n "$ui_hits" ]; then
  fail "src/api 引用了 UI 层："
  printf '%s\n' "$ui_hits" | sed 's/^/      /'
else
  ok "src/api 未引用 UI 层"
fi

echo ""
echo "[7] 用户可见文案不得无条件泄漏底层术语（00 §12.2 / 03 U8）"
# 判据：出现底层术语（文件名 / 键名 / 二进制偏移）的 UI 文件，必须同时提供
# 「高级详情」这类可折叠层级 —— 允许术语存在于高级层，不允许它成为默认层。
# 纯注释行不计入。
TERM_PATTERN='LocalStorage\.db|CustomFrameRate|0x00[0-9A-F]{2}'
COMMENT_PATTERN='^[^:]*:[0-9]+:[[:space:]]*(//|\*|/\*)'
ui_files=$(grep -rIl --include='*.tsx' -E "$TERM_PATTERN" src/views src/components 2>/dev/null || true)
if [ -z "$ui_files" ]; then
  ok "默认层未出现底层术语"
else
  leaked=0
  while IFS= read -r file; do
    [ -z "$file" ] && continue
    # 去掉纯注释行后再找术语
    effective=$(grep -vE "$COMMENT_PATTERN" "$file" | grep -E "$TERM_PATTERN" || true)
    if [ -z "$effective" ]; then
      continue
    fi
    if grep -qE '高级详情|advanced' "$file"; then
      note "· $file 含底层术语，但提供「高级详情」层级 → 通过（03 U8 允许）"
    else
      fail "$file 出现底层术语且没有「高级详情」层级，默认层会泄漏实现细节："
      printf '%s\n' "$effective" | sed 's/^/      /'
      leaked=$((leaked + 1))
    fi
  done <<< "$ui_files"
  [ "$leaked" -eq 0 ] && ok "底层术语仅出现在「高级详情」层级内"
fi

echo ""
echo "[8] 前端不得自带游戏目录（契约 §3.1）"
# 游戏显示名必须来自 Core 的 listGames()，UI 侧不得硬编码游戏名清单。
game_names='星穹铁道|绝区零|终末地'
catalog_hits=$(grep -rIn --include='*.tsx' --include='*.ts' -E "$game_names" src/views src/store 2>/dev/null \
  | grep -vE "$COMMENT_PATTERN" || true)
if [ -n "$catalog_hits" ]; then
  fail "views/ 或 store/ 中硬编码了游戏名，应经 api.listGames() 取得："
  printf '%s\n' "$catalog_hits" | sed 's/^/      /'
else
  ok "views/ 与 store/ 未硬编码游戏名清单"
fi

echo ""
if [ "$failures" -gt 0 ]; then
  echo "架构不变量断言失败：$failures 处"
  echo ""
  exit 1
fi

echo "架构不变量断言全部通过"
echo ""
