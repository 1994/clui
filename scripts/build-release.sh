#!/bin/bash
# 构建发布版本脚本

set -e

BINARY_NAME="clash-tui"
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

echo "🔨 构建 ${BINARY_NAME} v${VERSION}..."

# 清理旧构建
cargo clean

# 构建 release
echo "📦 构建 release..."
cargo build --release

# 显示大小
echo ""
echo "📊 构建结果:"
ls -lh target/release/${BINARY_NAME}

# 运行 strip（如果可用）
if command -v strip &> /dev/null; then
    echo "✂️  strip 二进制..."
    strip target/release/${BINARY_NAME}
    ls -lh target/release/${BINARY_NAME}
fi

echo ""
echo "✅ 构建完成!"
echo "二进制: target/release/${BINARY_NAME}"
