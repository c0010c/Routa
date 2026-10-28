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

从 [Releases](https://github.com/c0010c/Routa/releases) 下载最新版本（当前为 [v0.2.0](https://github.com/c0010c/Routa/releases/tag/v0.2.0)），Windows x64 提供以下三种格式，任选其一：

- [routa_0.2.0_x64-setup.exe](https://github.com/c0010c/Routa/releases/download/v0.2.0/routa_0.2.0_x64-setup.exe)（NSIS 安装程序）
- [routa_0.2.0_x64_en-US.msi](https://github.com/c0010c/Routa/releases/download/v0.2.0/routa_0.2.0_x64_en-US.msi)（MSI 安装包）
- [routa_0.2.0_x64_portable.zip](https://github.com/c0010c/Routa/releases/download/v0.2.0/routa_0.2.0_x64_portable.zip)（便携版，解压后运行 `routa.exe`）

各文件的 SHA256 见同一 Release 中的 `SHA256SUMS.txt`。安装包和便携版都已内置 mihomo 内核（版本见 [`MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)），也可以在设置页改用自己的内核。

**安装版**的数据保存在 `%APPDATA%\dev.routa\`（`state.json` 为配置，`mihomo\` 为内核工作目录和日志）。

**便携版**：exe 旁边有 `portable` 文件时，所有数据（包括 WebView 缓存）都保存在 exe 旁边的 `data\` 文件夹中；删除该标记文件并重启即改为保存到 `%APPDATA%\dev.routa\`。切换模式不会自动搬移数据。注意：

- 程序目录必须可写（不要放在 `Program Files` 下），否则启动时会报错；
- 开机自启会记录当前 exe 路径，移动文件夹后需在设置页重新开启；
- 需要系统自带的 WebView2 运行时（Windows 10/11 一般已内置）；
- 便携版与安装版不能同时运行。

**从 v0.1.0 升级**：v0.1.0 仍使用更名前的 `mihomo-mapper` 名称，数据保存在 `%APPDATA%\dev.mihomo-mapper\`。v0.2.0 **不会自动迁移**该目录，升级后不会沿用旧配置；旧版本可在 [v0.1.0 Release](https://github.com/c0010c/Routa/releases/tag/v0.1.0) 下载。

> 仅支持 Windows x64。本工具不提供 TUN 模式、系统代理和按规则分流。

### 从源码构建

构建使用的内核版本见 [`src-tauri/binaries/MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)。

需要 Node.js、[pnpm](https://pnpm.io/)、Rust 工具链，以及 Tauri 2 的 [Windows 依赖](https://tauri.app/start/prerequisites/)（WebView2、MSVC 生成工具）。

```bash
pnpm install
pnpm fetch-mihomo     # 下载并校验固定版本的 mihomo 内核到 src-tauri/binaries
pnpm tauri dev        # 开发模式运行
pnpm tauri build      # 生成 NSIS / MSI 安装包，位于 src-tauri/target/release/bundle
pnpm pack:portable    # 生成便携版 zip，位于 src-tauri/target/release/bundle/portable
```

`pnpm tauri build` 本地生成 `routa_<版本>_x64-setup.exe` 和 `routa_<版本>_x64_en-US.msi`。`pnpm pack:portable` 调用 `pnpm tauri build --no-bundle`，再将 `routa.exe`、`mihomo.exe`、`portable` 标记文件和 `LICENSE` 打包为 `routa_<版本>_x64_portable.zip`，解压后运行 `routa.exe`。

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

Download the latest version from [Releases](https://github.com/c0010c/Routa/releases) (currently [v0.2.0](https://github.com/c0010c/Routa/releases/tag/v0.2.0)). Three Windows x64 formats are available; choose any one:

- [routa_0.2.0_x64-setup.exe](https://github.com/c0010c/Routa/releases/download/v0.2.0/routa_0.2.0_x64-setup.exe) (NSIS installer)
- [routa_0.2.0_x64_en-US.msi](https://github.com/c0010c/Routa/releases/download/v0.2.0/routa_0.2.0_x64_en-US.msi) (MSI package)
- [routa_0.2.0_x64_portable.zip](https://github.com/c0010c/Routa/releases/download/v0.2.0/routa_0.2.0_x64_portable.zip) (portable; unzip and run `routa.exe`)

SHA256 checksums are in `SHA256SUMS.txt` on the same release. The installers and the portable build bundle the mihomo core (version in [`MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)); you can point the app to your own core in Settings.

**Installed builds** store data in `%APPDATA%\dev.routa\` (`state.json` holds the configuration, `mihomo\` is the core's working directory and log).

**Portable build**: when a `portable` file sits next to the exe, all data (including the WebView cache) is stored in a `data\` folder next to the exe; delete that marker file and restart to store data in `%APPDATA%\dev.routa\` instead. Switching modes does not move data automatically. Note:

- The app folder must be writable (not under `Program Files`), otherwise the app reports an error on startup;
- Start with Windows records the current exe path; after moving the folder, turn it on again in Settings;
- The WebView2 runtime is required (usually built into Windows 10/11);
- The portable and installed builds cannot run at the same time.

**Upgrading from v0.1.0**: v0.1.0 still uses the previous `mihomo-mapper` name and stores data in `%APPDATA%\dev.mihomo-mapper\`. v0.2.0 does **not migrate** that directory automatically, so the old configuration is not reused after upgrading. The old version is still available on the [v0.1.0 release](https://github.com/c0010c/Routa/releases/tag/v0.1.0).

> Windows x64 only. TUN mode, system proxy and rule-based routing are out of scope.

### Build from source

The core version used for source builds is pinned in [`src-tauri/binaries/MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION).

Requires Node.js, [pnpm](https://pnpm.io/), the Rust toolchain and the Tauri 2 [Windows prerequisites](https://tauri.app/start/prerequisites/) (WebView2, MSVC build tools).

```bash
pnpm install
pnpm fetch-mihomo     # download and verify the pinned mihomo core into src-tauri/binaries
pnpm tauri dev        # run in development mode
pnpm tauri build      # build NSIS / MSI installers into src-tauri/target/release/bundle
pnpm pack:portable    # build the portable zip into src-tauri/target/release/bundle/portable
```

`pnpm tauri build` produces `routa_<version>_x64-setup.exe` and `routa_<version>_x64_en-US.msi` locally. `pnpm pack:portable` runs `pnpm tauri build --no-bundle`, then packages `routa.exe`, `mihomo.exe`, the `portable` marker and `LICENSE` into `routa_<version>_x64_portable.zip`; unzip it and run `routa.exe`.

Tests:

```bash
pnpm test                                        # frontend tests
pnpm typecheck                                   # type check
cargo test --manifest-path src-tauri/Cargo.toml  # backend tests
```

Stack: Tauri 2 (Rust, tokio) + React + TypeScript + Vite + MUI, with frontend/backend types generated by tauri-specta.

### License

[MIT](LICENSE)
