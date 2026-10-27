# mihomo-mapper

[中文](#中文) | [English](#english)

---

## 中文

一个基于 [mihomo](https://github.com/MetaCubeX/mihomo) 内核的 Windows 桌面工具，围绕 **“入口端口 → 出口（节点 / 策略组 / 直连 + 网卡）”** 的映射来管理代理，全程不用手写 YAML。

适合这样的场景：

- 本机和局域网设备通过不同端口接入，每个端口走不同的出口；
- 电脑有多张网卡，不同端口的流量要从指定网卡出去，并且严格隔离。

### 功能

- **订阅**：导入 Clash/mihomo YAML 订阅或 base64 分享链接订阅，每个订阅可单独设置自动更新间隔。
- **手动节点**：`socks5`、`http` 用表单创建，其他类型可粘贴 mihomo 节点 YAML 创建；支持自建节点组。
- **出口**：单个节点、策略组（`url-test` / `fallback` / `load-balance` / `select`，成员可以是节点或整个组）或直连，都可以绑定指定网卡。
- **Listener**：多个 `mixed`（HTTP + SOCKS5）、`http` 或 `socks` 端口，可设置用户名/密码，可开放给局域网。
- **概览（组装页）**：在一张表里给每个 Listener 选出口、新建出口、切换 `select` 组节点，查看实时/累计流量和延迟。
- **Tailscale**：管理 `type: tailscale` 节点，包括登录状态与参数。
- **不泄露**：节点失效或网卡断开时，对应 Listener 直接停用，不会悄悄改走直连或默认网卡；网卡恢复后自动恢复。
- **进程托管**：内置 mihomo 内核，配置变更后热重载，内核意外退出会自动重启；托盘常驻，可开机自启。

### 安装

从 [Releases](https://github.com/c0010c/mihomoGUI/releases) 下载以下任意一个安装包：

- `mihomo-mapper_<版本>_x64-setup.exe`（NSIS 安装程序）
- `mihomo-mapper_<版本>_x64_en-US.msi`（MSI 安装包）

安装包已内置 mihomo 内核（版本见 [`src-tauri/binaries/MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)），也可以在设置页改用自己的内核。

数据保存在 `%APPDATA%\dev.mihomo-mapper\`（`state.json` 为配置，`mihomo\` 为内核工作目录和日志）。

> 仅支持 Windows x64。本工具不提供 TUN 模式、系统代理和按规则分流。

### 从源码构建

需要 Node.js、[pnpm](https://pnpm.io/)、Rust 工具链，以及 Tauri 2 的 [Windows 依赖](https://tauri.app/start/prerequisites/)（WebView2、MSVC 生成工具）。

```bash
pnpm install
pnpm fetch-mihomo     # 下载并校验固定版本的 mihomo 内核到 src-tauri/binaries
pnpm tauri dev        # 开发模式运行
pnpm tauri build      # 生成 NSIS / MSI 安装包，位于 src-tauri/target/release/bundle
```

测试：

```bash
pnpm test                                        # 前端测试
pnpm typecheck                                   # 类型检查
cargo test --manifest-path src-tauri/Cargo.toml  # 后端测试
```

技术栈：Tauri 2（Rust、tokio）+ React + TypeScript + Vite + MUI，前后端类型由 tauri-specta 生成。

### 许可证

[MIT](LICENSE)

---

## English

A Windows desktop tool built on the [mihomo](https://github.com/MetaCubeX/mihomo) core. It manages proxies as a mapping of **inbound port → exit (node / proxy group / direct, plus network interface)**, so you never have to hand-write YAML.

It is made for setups like:

- Local and LAN devices connect through different ports, and each port goes out through a different exit;
- The PC has several network adapters, and traffic from each port must leave through a specific adapter, strictly isolated.

### Features

- **Subscriptions**: import Clash/mihomo YAML subscriptions or base64 share-link subscriptions, each with its own auto-update interval.
- **Manual nodes**: create `socks5` / `http` nodes with a form, or any other type by pasting mihomo proxy YAML; organize them into your own node groups.
- **Exits**: a single node, a proxy group (`url-test` / `fallback` / `load-balance` / `select`, whose members can be nodes or whole groups), or direct — each optionally bound to a network interface.
- **Listeners**: multiple `mixed` (HTTP + SOCKS5), `http` or `socks` ports, with optional username/password, optionally open to the LAN.
- **Overview (assembly page)**: assign an exit to each listener, create exits and switch `select` group members in one table, with live/total traffic and latency.
- **Tailscale**: manage `type: tailscale` nodes, including login state and parameters.
- **No leaks**: when a node disappears or an adapter goes down, the affected listener is stopped instead of silently falling back to direct or the default adapter; it resumes when the adapter comes back.
- **Process management**: bundled mihomo core, hot reload on config changes, automatic restart if the core exits; lives in the system tray and can start with Windows.

### Install

Download either installer from [Releases](https://github.com/c0010c/mihomoGUI/releases):

- `mihomo-mapper_<version>_x64-setup.exe` (NSIS installer)
- `mihomo-mapper_<version>_x64_en-US.msi` (MSI package)

The mihomo core is bundled (version in [`src-tauri/binaries/MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)); you can point the app to your own core in Settings.

Data lives in `%APPDATA%\dev.mihomo-mapper\` (`state.json` holds the configuration, `mihomo\` is the core's working directory and log).

> Windows x64 only. TUN mode, system proxy and rule-based routing are out of scope.

### Build from source

Requires Node.js, [pnpm](https://pnpm.io/), the Rust toolchain and the Tauri 2 [Windows prerequisites](https://tauri.app/start/prerequisites/) (WebView2, MSVC build tools).

```bash
pnpm install
pnpm fetch-mihomo     # download and verify the pinned mihomo core into src-tauri/binaries
pnpm tauri dev        # run in development mode
pnpm tauri build      # build NSIS / MSI installers into src-tauri/target/release/bundle
```

Tests:

```bash
pnpm test                                        # frontend tests
pnpm typecheck                                   # type check
cargo test --manifest-path src-tauri/Cargo.toml  # backend tests
```

Stack: Tauri 2 (Rust, tokio) + React + TypeScript + Vite + MUI, with frontend/backend types generated by tauri-specta.

### License

[MIT](LICENSE)
