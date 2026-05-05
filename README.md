# Clash TUI

Clash/Mihomo TUI，随安装包携带 Mihomo 核心。

## 特性

- 🦀 Rust TUI
- 📦 随包携带 Mihomo 核心，不运行时下载
- 🖥️ 直观的 TUI 界面
- 🔧 支持静默守护模式
- 📝 订阅自动更新

## 快速开始

### 安装

```bash
# 方式1: 使用安装脚本
./install.sh

# 方式2: 使用 Make
make install

# 方式3: 手动复制
make release
sudo cp target/release/clash-tui /usr/local/bin/
```

### 使用

```bash
# 启动 TUI 界面（默认）
clash-tui

# 如果已在运行，会显示当前状态
clash-tui
# 输出: clash-tui 已在运行 (PID: 12345)
#       模式: Tui
#       API: http://127.0.0.1:9090
#       ...

# 后台守护模式
clash-tui daemon

# 停止 Mihomo 核心（保持 clash-tui 运行）
clash-tui stop

# 重启 Mihomo 核心
clash-tui restart

# 查看状态
clash-tui status

# 完全退出（clash-tui + Mihomo）
clash-tui quit

# 指定配置文件
clash-tui -c ~/.config/clash/config.yaml
```

## 构建

```bash
# 快速构建
make build

# 发布构建（优化）
make release

# 创建发布包时必须提供 Mihomo core
make dist MIHOMO_BIN=/path/to/mihomo

# 最小体积构建
make mini

# 创建发布包
make dist
```

## 配置

### 配置文件

位置（按优先级）：
1. `-c / --config` 指定的路径
2. `~/.config/clash-tui/config.yaml`
3. `./config.yaml`

### 日志文件

日志默认持久化到文件，按天轮转，自动保留最近7天：

- **macOS**: `~/Library/Application Support/clash-tui/logs/`
- **Linux**: `~/.config/clash-tui/logs/`

查看日志：
```bash
# 查看最新日志
tail -f ~/.config/clash-tui/logs/clash-tui.log

# 查看当天日志
cat ~/.config/clash-tui/logs/clash-tui.log
```

### 日志级别

```bash
# 指定日志级别（默认: info）
clash-tui --log-level debug
clash-tui --log-level warn

# 或通过环境变量
RUST_LOG=debug clash-tui
```

## 系统要求

- Rust 1.75+
- 打包时提供对应平台的 Mihomo 二进制
- macOS / Linux

## 许可证

MIT
