# Routa

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

#### 已发布版本：v0.1.0（预发布）

截至 2026-10-27，[Releases](https://github.com/c0010c/Routa/releases) 仅有 [v0.1.0 预发布版本](https://github.com/c0010c/Routa/releases/tag/v0.1.0)，提供以下两个 Windows x64 安装包，任选其一：

- [mihomo-mapper_0.1.0_x64-setup.exe](https://github.com/c0010c/Routa/releases/download/v0.1.0/mihomo-mapper_0.1.0_x64-setup.exe)（NSIS 安装程序）
- [mihomo-mapper_0.1.0_x64_en-US.msi](https://github.com/c0010c/Routa/releases/download/v0.1.0/mihomo-mapper_0.1.0_x64_en-US.msi)（MSI 安装包）

项目现名为 **Routa**，但上述发行包仍使用更名前的 `mihomo-mapper` 名称。该版本没有 `routa_` 前缀的安装包，也没有便携版 ZIP；后续版本请以 Releases 的实际附件清单为准。

这两个安装包已内置 mihomo 内核（版本见 [v0.1.0 的 MIHOMO_VERSION](https://github.com/c0010c/Routa/blob/v0.1.0/src-tauri/binaries/MIHOMO_VERSION)），也可以在设置页改用自己的内核。

**v0.1.0 已发布安装版**的数据保存在 `%APPDATA%\dev.mihomo-mapper\`（`state.json` 为配置，`mihomo\` 为内核工作目录和日志）。该版本不支持下述 `portable` 标记机制。

#### main 分支：Routa 更名与便携模式（需从源码构建）

`main` 已包含 [Routa 更名](https://github.com/c0010c/Routa/commit/d39b70b2e99c2c5466f64896a59eab783aff9987)和[便携版支持](https://github.com/c0010c/Routa/commit/767be63ad0806e979809aff051c9903cd498c2d2)，但上述 v0.1.0 发行包不包含这些变更。构建方式见下方“从源码构建”。

**更名后的 main 普通模式**使用应用标识 `dev.routa`，数据保存在 `%APPDATA%\dev.routa\`（`state.json` 为配置，`mihomo\` 为内核工作目录和日志）。程序**不会自动迁移**旧的 `%APPDATA%\dev.mihomo-mapper\` 数据目录，切换到 main 构建不会自动沿用旧配置。

**main 便携模式**：exe 旁边有 `portable` 文件时，所有数据（包括 WebView 缓存）都保存在 exe 旁边的 `data\` 文件夹中；删除该标记文件并重启即恢复为 main 普通模式。切换模式不会自动搬移数据。注意：

- 程序目录必须可写（不要放在 `Program Files` 下），否则启动时会报错；
- 开机自启会记录当前 exe 路径，移动文件夹后需在设置页重新开启；
- 需要系统自带的 WebView2 运行时（Windows 10/11 一般已内置）；
- main 构建的便携版与安装版不能同时运行。

> 仅支持 Windows x64。本工具不提供 TUN 模式、系统代理和按规则分流。

### 从源码构建

以下命令适用于包含更名和便携版支持的 `main` 源码；不适用于 `v0.1.0` 标签。构建使用的内核版本见 [`src-tauri/binaries/MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)。

需要 Node.js、[pnpm](https://pnpm.io/)、Rust 工具链，以及 Tauri 2 的 [Windows 依赖](https://tauri.app/start/prerequisites/)（WebView2、MSVC 生成工具）。

```bash
pnpm install
pnpm fetch-mihomo     # 下载并校验固定版本的 mihomo 内核到 src-tauri/binaries
pnpm tauri dev        # 开发模式运行
pnpm tauri build      # 生成 NSIS / MSI 安装包，位于 src-tauri/target/release/bundle
pnpm pack:portable    # 生成便携版 zip，位于 src-tauri/target/release/bundle/portable
```

`pnpm tauri build` 本地生成 `routa_<版本>_x64-setup.exe` 和 `routa_<版本>_x64_en-US.msi`。`pnpm pack:portable` 调用 `pnpm tauri build --no-bundle`，再将 `routa.exe`、`mihomo.exe`、`portable` 标记文件和 `LICENSE` 打包为 `routa_<版本>_x64_portable.zip`，解压后运行 `routa.exe`。这些是本地构建产物名称，不代表 Releases 已提供同名附件。

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

#### Published release: v0.1.0 (pre-release)

As of 2026-10-27, [Releases](https://github.com/c0010c/Routa/releases) contains only the [v0.1.0 pre-release](https://github.com/c0010c/Routa/releases/tag/v0.1.0), with two Windows x64 installers. Choose either one:

- [mihomo-mapper_0.1.0_x64-setup.exe](https://github.com/c0010c/Routa/releases/download/v0.1.0/mihomo-mapper_0.1.0_x64-setup.exe) (NSIS installer)
- [mihomo-mapper_0.1.0_x64_en-US.msi](https://github.com/c0010c/Routa/releases/download/v0.1.0/mihomo-mapper_0.1.0_x64_en-US.msi) (MSI package)

The project is now called **Routa**, but these release packages still use the previous `mihomo-mapper` name. This release has no installers with the `routa_` prefix and no portable ZIP. For later releases, check the actual asset list on Releases.

Both installers bundle the mihomo core (version in [MIHOMO_VERSION at v0.1.0](https://github.com/c0010c/Routa/blob/v0.1.0/src-tauri/binaries/MIHOMO_VERSION)); you can point the app to your own core in Settings.

**The published v0.1.0 installers** store data in `%APPDATA%\dev.mihomo-mapper\` (`state.json` holds the configuration, `mihomo\` is the core's working directory and log). This release does not support the `portable` marker described below.

#### main branch: Routa rename and portable mode (build from source)

`main` includes the [Routa rename](https://github.com/c0010c/Routa/commit/d39b70b2e99c2c5466f64896a59eab783aff9987) and [portable support](https://github.com/c0010c/Routa/commit/767be63ad0806e979809aff051c9903cd498c2d2), but the v0.1.0 release packages above do not include these changes. See “Build from source” below.

**Normal mode on main after the rename** uses the application identifier `dev.routa` and stores data in `%APPDATA%\dev.routa\` (`state.json` holds the configuration, `mihomo\` is the core's working directory and log). The old `%APPDATA%\dev.mihomo-mapper\` data directory is **not migrated automatically**; switching to a main build does not automatically reuse the old configuration.

**Portable mode on main**: when a `portable` file sits next to the exe, all data (including the WebView cache) is stored in a `data\` folder next to the exe; delete that marker file and restart to return to normal mode on main. Switching modes does not move data automatically. Note:

- The app folder must be writable (not under `Program Files`), otherwise the app reports an error on startup;
- Start with Windows records the current exe path; after moving the folder, turn it on again in Settings;
- The WebView2 runtime is required (usually built into Windows 10/11);
- Portable and installed builds from main cannot run at the same time.

> Windows x64 only. TUN mode, system proxy and rule-based routing are out of scope.

### Build from source

These commands apply to `main` source with the rename and portable support, not the `v0.1.0` tag. The core version used for source builds is pinned in [`src-tauri/binaries/MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION).

Requires Node.js, [pnpm](https://pnpm.io/), the Rust toolchain and the Tauri 2 [Windows prerequisites](https://tauri.app/start/prerequisites/) (WebView2, MSVC build tools).

```bash
pnpm install
pnpm fetch-mihomo     # download and verify the pinned mihomo core into src-tauri/binaries
pnpm tauri dev        # run in development mode
pnpm tauri build      # build NSIS / MSI installers into src-tauri/target/release/bundle
pnpm pack:portable    # build the portable zip into src-tauri/target/release/bundle/portable
```

`pnpm tauri build` produces `routa_<version>_x64-setup.exe` and `routa_<version>_x64_en-US.msi` locally. `pnpm pack:portable` runs `pnpm tauri build --no-bundle`, then packages `routa.exe`, `mihomo.exe`, the `portable` marker and `LICENSE` into `routa_<version>_x64_portable.zip`; unzip it and run `routa.exe`. These are local build output names, not a list of assets already available on Releases.

Tests:

```bash
pnpm test                                        # frontend tests
pnpm typecheck                                   # type check
cargo test --manifest-path src-tauri/Cargo.toml  # backend tests
```

Stack: Tauri 2 (Rust, tokio) + React + TypeScript + Vite + MUI, with frontend/backend types generated by tauri-specta.

### License

[MIT](LICENSE)
