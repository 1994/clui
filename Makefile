# Clash TUI Makefile
# 快速构建和打包工具

# 配置
BINARY_NAME = clash-tui
DIST_DIR = dist
TARGET_DIR = target
RELEASE_BIN = $(TARGET_DIR)/release/$(BINARY_NAME)
MIHOMO_BIN ?= third_party/mihomo/$(PLATFORM)-$(ARCH)/mihomo
CORE_INSTALL_DIR = /opt/clashtui

# 检测平台
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
    PLATFORM = macos
    ifeq ($(UNAME_M),arm64)
        ARCH = aarch64
    else
        ARCH = x86_64
    endif
else ifeq ($(UNAME_S),Linux)
    PLATFORM = linux
    ARCH = $(UNAME_M)
endif

VERSION = $(shell grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
RELEASE_NAME = $(BINARY_NAME)-v$(VERSION)-$(PLATFORM)-$(ARCH)

.PHONY: all build release mini run clean install dist help check-core

# 默认目标
all: release

# 快速开发构建
build:
	@echo "🔨 快速构建..."
	cargo build

# 发布构建（优化）
release:
	@echo "🚀 构建发布版本..."
	cargo build --release
	@echo "✅ 构建完成: $(RELEASE_BIN)"
	@ls -lh $(RELEASE_BIN)

# 最小体积构建（极端优化）
mini:
	@echo "📦 最小体积构建..."
	cargo build --profile mini
	@echo "✅ 构建完成: $(TARGET_DIR)/mini/$(BINARY_NAME)"
	@ls -lh $(TARGET_DIR)/mini/$(BINARY_NAME)

# 运行（开发模式）
run:
	@echo "▶️  运行开发版本..."
	cargo run

# 运行 TUI 模式
run-tui:
	@echo "▶️  运行 TUI..."
	cargo run -- tui

# 运行静默模式
run-daemon:
	@echo "▶️  运行静默模式..."
	cargo run -- daemon

# 停止服务
stop:
	@echo "🛑 停止 Mihomo..."
	./$(RELEASE_BIN) stop 2>/dev/null || cargo run -- stop

# 查看状态
status:
	@echo "📊 查看状态..."
	./$(RELEASE_BIN) status 2>/dev/null || cargo run -- status

# 清理构建
clean:
	@echo "🧹 清理构建..."
	cargo clean
	rm -rf $(DIST_DIR)

# 检查随包 Mihomo core
check-core:
	@if [ ! -f "$(MIHOMO_BIN)" ]; then \
		echo "❌ 未找到 Mihomo core: $(MIHOMO_BIN)"; \
		echo "请先把对应平台的 mihomo 二进制放到该路径，或执行: make dist MIHOMO_BIN=/path/to/mihomo"; \
		exit 1; \
	fi

# 安装到系统（需要 sudo）
install: release check-core
	@echo "📥 安装到 /usr/local/bin..."
	install -m 755 $(RELEASE_BIN) /usr/local/bin/$(BINARY_NAME)
	@echo "📥 安装 Mihomo core 到 $(CORE_INSTALL_DIR)..."
	install -d $(CORE_INSTALL_DIR)
	install -m 755 $(MIHOMO_BIN) $(CORE_INSTALL_DIR)/mihomo
	@echo "✅ 安装完成: /usr/local/bin/$(BINARY_NAME)"

# 卸载
uninstall:
	@echo "🗑️  卸载..."
	rm -f /usr/local/bin/$(BINARY_NAME)
	rm -f $(CORE_INSTALL_DIR)/mihomo
	@echo "✅ 已卸载"

# 创建发布压缩包
dist: release check-core
	@echo "📦 创建发布包..."
	@mkdir -p $(DIST_DIR)/$(RELEASE_NAME)
	
	# 复制二进制文件
	cp $(RELEASE_BIN) $(DIST_DIR)/$(RELEASE_NAME)/
	mkdir -p $(DIST_DIR)/$(RELEASE_NAME)/bin
	cp $(MIHOMO_BIN) $(DIST_DIR)/$(RELEASE_NAME)/bin/mihomo
	chmod +x $(DIST_DIR)/$(RELEASE_NAME)/bin/mihomo
	
	# 创建默认配置
	@mkdir -p $(DIST_DIR)/$(RELEASE_NAME)/config
	@echo "mixed-port: 7890" > $(DIST_DIR)/$(RELEASE_NAME)/config/config.yaml
	@echo "external-controller: 127.0.0.1:9090" >> $(DIST_DIR)/$(RELEASE_NAME)/config/config.yaml
	@echo "log-level: info" >> $(DIST_DIR)/$(RELEASE_NAME)/config/config.yaml
	@echo "mode: rule" >> $(DIST_DIR)/$(RELEASE_NAME)/config/config.yaml
	
	# 创建 README
	@echo "# $(BINARY_NAME) v$(VERSION)" > $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "## 使用方法" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "1. 启动 TUI:" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "   ./$(BINARY_NAME) tui" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "2. 静默模式:" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "   ./$(BINARY_NAME) daemon" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "3. 停止服务:" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	@echo "   ./$(BINARY_NAME) stop" >> $(DIST_DIR)/$(RELEASE_NAME)/README.txt
	
	# 创建启动脚本
	@echo '#!/bin/bash' > $(DIST_DIR)/$(RELEASE_NAME)/start.sh
	@echo 'cd "$$(dirname "$0")"' >> $(DIST_DIR)/$(RELEASE_NAME)/start.sh
	@echo './$(BINARY_NAME) tui' >> $(DIST_DIR)/$(RELEASE_NAME)/start.sh
	@chmod +x $(DIST_DIR)/$(RELEASE_NAME)/start.sh
	
	# 压缩
	cd $(DIST_DIR) && tar czf $(RELEASE_NAME).tar.gz $(RELEASE_NAME)
	
	@echo "✅ 发布包已创建: $(DIST_DIR)/$(RELEASE_NAME).tar.gz"
	@echo "📊 文件大小:"
	@ls -lh $(DIST_DIR)/$(RELEASE_NAME).tar.gz

# 查看二进制大小
size: release
	@echo "📊 二进制大小分析:"
	@ls -lh $(RELEASE_BIN)
	@echo ""
	@echo "📦 依赖数量:"
	@cargo tree | wc -l

# 测试
test:
	@echo "🧪 运行测试..."
	cargo test

# 代码检查
check:
	@echo "🔍 代码检查..."
	cargo check
	cargo clippy -- -D warnings

# 格式化
fmt:
	@echo "📝 格式化代码..."
	cargo fmt

# 帮助
help:
	@echo "Clash TUI 构建工具"
	@echo ""
	@echo "使用方法:"
	@echo "  make build      - 快速开发构建"
	@echo "  make release    - 发布构建（优化）"
	@echo "  make mini       - 最小体积构建"
	@echo "  make run        - 运行开发版本"
	@echo "  make dist       - 创建发布压缩包"
	@echo "  make install    - 安装到 /usr/local/bin"
	@echo "  make clean      - 清理构建"
	@echo "  make size       - 查看二进制大小"
	@echo "  make help       - 显示帮助"
