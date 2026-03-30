#!/usr/bin/env bash
# 交叉编译 codex-switch 到主流平台
# 依赖：
#   - rustup（含所需 targets）
#   - musl-tools（apt install musl-tools，用于 x86_64-linux-musl）
#   - cross（cargo install cross，用于 aarch64-linux-musl / windows-gnu）
#   - docker（cross 的依赖，须在后台运行）
#
# 用法：
#   ./scripts/build-release.sh              # 构建所有 Linux 目标
#   ./scripts/build-release.sh --all        # 所有目标（包括 Windows）
#   ./scripts/build-release.sh --target x86_64-unknown-linux-musl
set -euo pipefail

BINARY="codex-switch"
DIST="dist"
PROFILE="release"

# ── 解析参数 ──────────────────────────────────────────────────────────────────
ALL=false
SPECIFIC_TARGET=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --all)       ALL=true; shift ;;
        --target)    SPECIFIC_TARGET="$2"; shift 2 ;;
        *)           echo "未知参数: $1"; exit 1 ;;
    esac
done

# ── 检查工具 ──────────────────────────────────────────────────────────────────
check_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo "❌ 未找到 $1，请先安装：$2"
        exit 1
    fi
}

check_tool rustup "https://rustup.rs"
check_tool cargo  "https://rustup.rs"

# ── 目标定义 ──────────────────────────────────────────────────────────────────
# [target, artifact_name, 构建方式: native/cross]
declare -A BUILD_METHOD=(
    ["x86_64-unknown-linux-musl"]="native"
    ["aarch64-unknown-linux-musl"]="cross"
    ["x86_64-pc-windows-gnu"]="cross"
)

declare -A ARTIFACT_NAME=(
    ["x86_64-unknown-linux-musl"]="${BINARY}-linux-amd64"
    ["aarch64-unknown-linux-musl"]="${BINARY}-linux-arm64"
    ["x86_64-pc-windows-gnu"]="${BINARY}-windows-amd64.exe"
)

if $ALL; then
    TARGETS=("x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl" "x86_64-pc-windows-gnu")
elif [[ -n "$SPECIFIC_TARGET" ]]; then
    TARGETS=("$SPECIFIC_TARGET")
else
    TARGETS=("x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl")
fi

# ── 准备输出目录 ──────────────────────────────────────────────────────────────
mkdir -p "$DIST"

# ── 构建循环 ──────────────────────────────────────────────────────────────────
for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━ $TARGET ━━━━━━━━━━━━━━━━━━━━"
    METHOD="${BUILD_METHOD[$TARGET]:-cross}"
    ARTIFACT="${ARTIFACT_NAME[$TARGET]:-${BINARY}-${TARGET}}"

    # 添加 Rust target
    rustup target add "$TARGET" 2>/dev/null || true

    case "$METHOD" in
        native)
            # x86_64-linux-musl 可在 Linux 上原生编译，需要 musl-tools
            if [[ "$TARGET" == *"-musl" ]] && ! command -v "musl-gcc" &>/dev/null; then
                echo "⚠️  缺少 musl-gcc，尝试安装 musl-tools..."
                if command -v apt-get &>/dev/null; then
                    sudo apt-get install -y musl-tools
                else
                    echo "❌ 请手动安装 musl-tools，然后重试"
                    exit 1
                fi
            fi
            cargo build --$PROFILE --target "$TARGET"
            BIN_PATH="target/$TARGET/$PROFILE/${BINARY}"
            ;;
        cross)
            check_tool cross "cargo install cross --git https://github.com/cross-rs/cross"
            check_tool docker "https://docs.docker.com/engine/install/"
            cross build --$PROFILE --target "$TARGET"
            # Windows binary 有 .exe 后缀
            if [[ "$TARGET" == *"-windows-"* ]]; then
                BIN_PATH="target/$TARGET/$PROFILE/${BINARY}.exe"
            else
                BIN_PATH="target/$TARGET/$PROFILE/${BINARY}"
            fi
            ;;
    esac

    # 复制到 dist/
    cp "$BIN_PATH" "$DIST/$ARTIFACT"
    echo "✅ $DIST/$ARTIFACT  ($(du -sh "$DIST/$ARTIFACT" | cut -f1))"
done

# ── 生成 SHA256 校验文件 ──────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━ 生成校验和 ━━━━━━━━━━━━━━━━━━━━"
(cd "$DIST" && sha256sum ${BINARY}-* > SHA256SUMS)
echo "✅ $DIST/SHA256SUMS"
cat "$DIST/SHA256SUMS"

echo ""
echo "🎉 构建完成！产物在 $DIST/"
echo ""
echo "macOS / Windows 原生二进制请在对应系统上构建，或使用 GitHub Actions CI 自动发布。"
