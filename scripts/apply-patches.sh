#!/usr/bin/env bash
# 将 patches/<client>/ 下的所有补丁应用到对应子模块（按文件名排序）。
# 用法: scripts/apply-patches.sh [desktop|android|ios|all]
set -euo pipefail
cd "$(dirname "$0")/.."

apply() {
    local client=$1
    local dir="patches/$client"
    [ -d "$dir" ] || return 0
    for p in "$dir"/*.patch; do
        [ -e "$p" ] || continue
        echo "==> apply $p -> clients/$client"
        git -C "clients/$client" apply --check "../../$p"
        git -C "clients/$client" apply "../../$p"
    done
}

target=${1:-all}
if [ "$target" = all ]; then
    apply desktop; apply android; apply ios
else
    apply "$target"
fi
echo "All patches applied."
