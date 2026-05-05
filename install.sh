#!/bin/bash
# Clash TUI 安装脚本

set -e

BINARY_NAME="clash-tui"
INSTALL_DIR="/usr/local/bin"
CORE_INSTALL_DIR="/opt/clashtui"
REPO_URL="https://github.com/yourname/clash-tui"

echo "🚀 Clash TUI 安装脚本"
echo ""

# 检测平台
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Linux*)     PLATFORM=linux;;
    Darwin*)    PLATFORM=macos;;
    CYGWIN*)    PLATFORM=windows;;
    MINGW*)     PLATFORM=windows;;
    *)          PLATFORM="unknown";;
esac

echo "检测到平台: $PLATFORM ($ARCH)"

# 检查是否有预编译二进制
if [ -f "target/release/${BINARY_NAME}" ]; then
    echo "📦 使用本地构建的二进制文件"
    BINARY="target/release/${BINARY_NAME}"
elif [ -f "${BINARY_NAME}" ]; then
    echo "📦 使用当前目录的二进制文件"
    BINARY="${BINARY_NAME}"
else
    echo "❌ 未找到二进制文件"
    echo "请先构建: make release"
    exit 1
fi

CORE_BINARY=""
if [ -f "bin/mihomo" ]; then
    CORE_BINARY="bin/mihomo"
elif [ -f "third_party/mihomo/${PLATFORM}-${ARCH}/mihomo" ]; then
    CORE_BINARY="third_party/mihomo/${PLATFORM}-${ARCH}/mihomo"
elif [ -f "mihomo" ]; then
    CORE_BINARY="mihomo"
else
    echo "❌ 未找到随包 Mihomo core"
    echo "请把 mihomo 二进制放到 ./bin/mihomo 后再安装"
    exit 1
fi

# 安装
echo ""
echo "📥 安装 ${BINARY_NAME} 到 ${INSTALL_DIR}..."
if [ -w "$INSTALL_DIR" ]; then
    install -m 755 "$BINARY" "$INSTALL_DIR/${BINARY_NAME}"
else
    echo "需要 sudo 权限..."
    sudo install -m 755 "$BINARY" "$INSTALL_DIR/${BINARY_NAME}"
fi

echo "📥 安装 Mihomo core 到 ${CORE_INSTALL_DIR}..."
if [ -w "$(dirname "$CORE_INSTALL_DIR")" ]; then
    install -d "$CORE_INSTALL_DIR"
    install -m 755 "$CORE_BINARY" "$CORE_INSTALL_DIR/mihomo"
else
    echo "需要 sudo 权限..."
    sudo install -d "$CORE_INSTALL_DIR"
    sudo install -m 755 "$CORE_BINARY" "$CORE_INSTALL_DIR/mihomo"
fi

# 创建配置目录
echo "📁 创建配置目录..."
CONFIG_DIR="${HOME}/.config/clash-tui"
mkdir -p "$CONFIG_DIR"

# 创建默认配置（如果不存在）
if [ ! -f "$CONFIG_DIR/config.yaml" ]; then
    echo "📝 创建默认配置..."
    cat > "$CONFIG_DIR/config.yaml" << 'EOF'
# Clash TUI 默认配置
mixed-port: 7890
external-controller: 127.0.0.1:9090
log-level: info
mode: rule

proxy-providers: {}

proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT

rules:
  - MATCH,Proxy
EOF
fi

echo ""
echo "✅ 安装完成!"
echo ""
echo "使用方法:"
echo "  ${BINARY_NAME} tui      # 启动 TUI 界面"
echo "  ${BINARY_NAME} daemon   # 静默模式"
echo "  ${BINARY_NAME} status   # 查看状态"
echo "  ${BINARY_NAME} --help   # 查看帮助"
echo ""
echo "配置文件: ${CONFIG_DIR}/config.yaml"
