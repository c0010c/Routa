# Routa

[English](README.md) | 简体中文

基于 [mihomo](https://github.com/MetaCubeX/mihomo) 的 Windows 桌面代理管理工具。简洁版选节点即可连接；完整版可管理多个代理端口、分流规则与出站网卡，全程无需手写 YAML。

> 仅支持 Windows x64，应用界面目前为简体中文。

## 功能

- **双模式界面**：简洁版专注于选节点和连接；完整版提供概览、代理、进程分流、订阅与节点管理。可在设置中切换，保留已有配置和连接。
- **三种接管方式**：Windows 系统代理、通过进程分流代理指定应用，或使用虚拟网卡的 TUN。TUN 需要以管理员身份运行 Routa。
- **独立代理端口**：创建 `mixed`（HTTP + SOCKS5）、`http` 或 `socks` 监听器，在同一处配置端口使用的节点、策略组或直连出口，以及出站网卡、认证和局域网访问。
- **网卡隔离**：配置的出口不可用或绑定网卡断开时，停用受影响的监听器，避免悄悄改走直连或默认网卡；网卡恢复后自动恢复。
- **端口分流规则**：按顺序匹配完整域名、域名后缀或 IPv4 CIDR，选择代理、直连或拒绝；支持设置未匹配流量的去向，并在编辑器内验证示例目标。
- **订阅与节点**：导入 Clash/mihomo YAML 或 base64 分享链接订阅，设置自动更新及订阅下载的备用代理。HTTP/SOCKS5 节点可用表单创建，其他 mihomo 节点可通过 YAML 导入，支持批量导入、搜索、延迟排序和自建分组。
- **策略组与 Tailscale**：支持 `url-test`、`fallback`、`load-balance`、`select` 策略，以及 Tailscale 节点、登录状态和出口设置。
- **进程分流**：以规则为单位，把一组程序的流量交给 Mixed/Socks 监听器，查看各规则的连接与流量。默认按进程名（如 `ChatGPT.exe`）匹配，也可在高级选项中改为按完整路径匹配；按完整路径匹配的规则可设置组件故障时阻断。Routa 正常退出时的策略在“设置 → 进程分流”中单独设置。
- **监控与诊断**：分别查看内核与接管状态、实时及累计流量、活动连接及其实际路径、内核日志、端口冲突和组件状态。
- **桌面集成**：内置 mihomo 内核，配置热重载、内核崩溃后自动重启，支持开机启动、启动后自动连接，以及关闭窗口时最小化到托盘或退出。
- **自动更新**：NSIS 安装版会检查新版本、在后台下载，经你确认后安装；整个升级期间进程分流按“Routa 退出时”的策略处理。

## 安装

从 [Releases](https://github.com/c0010c/Routa/releases) 下载 `routa_<版本>_x64-setup.exe` 并运行。可用对应 Release 中的 `SHA256SUMS.txt` 校验下载文件。构建内置 mihomo，源码使用的固定版本见 [`MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION)。

运行需要 [WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/)。

## 快速开始

### 简洁版

1. 首次启动时选择 **简洁版**。
2. 导入订阅或手动添加节点，然后选择要使用的节点。
3. 选择 **系统代理**、**指定应用** 或 **TUN**，再开启连接。指定应用需要安装进程分流组件并添加应用；TUN 需要先退出 Routa，再以管理员身份运行。
4. 在设置中调整界面模式、启动后自动连接、开机启动和关闭窗口行为。

系统代理只影响遵循 Windows 代理设置的应用。对于不使用系统代理的应用，可按需选择指定应用分流或 TUN。

### 完整版

1. 在 **订阅** 或 **节点** 页面添加资源。
2. 在 **代理** 页面新建代理，配置端口、节点/策略组/直连，以及可选的分流规则、出站网卡、认证和局域网访问。
3. 将客户端指向该端口，或在 **概览 → 运行概况** 中选择默认代理并开启系统代理或 TUN。系统代理要求使用监听本机 IPv4 地址的 Mixed 监听器。
4. 在 **进程分流** 中为应用指定代理；遇到问题时，在 **概览 → 连接 / 日志** 查看实际连接和内核日志。

关闭系统代理或 TUN 后，内核和代理端口仍可使用；手动配置代理的客户端及独立启用的进程分流规则仍可能继续使用代理。

## 进程分流组件

进程分流由 Windows 服务 `RoutaProcSvc` 执行，基于 [ProxyBridge](https://github.com/InterceptSuite/ProxyBridge) 与 [WinDivert](https://github.com/basil00/WinDivert)。安装时会通过 UAC 注册服务；拒绝授权的话，可稍后在应用中启用。

卸载时会一并删除服务。若删除失败，可在管理员 PowerShell 中执行以下命令，再删除 `%ProgramFiles%\Routa\ProcessRouting` 文件夹：

```powershell
& "$env:ProgramFiles\Routa\ProcessRouting\routa-procsvc.exe" --uninstall
```

注意事项：

- 目标必须是监听 IPv4（如 `127.0.0.1`）的 Mixed/Socks 监听器，不支持 HTTP 专用监听器和 IPv6 入口。
- 规则对本机所有 Windows 用户运行的该程序生效；Windows DNS 客户端服务发出的 DNS 查询不经过进程分流。
- 阻断规则在崩溃和重启后仍然生效，组件未运行时被阻断的程序会一直断网；故障时直连的规则在开机后服务启动前可能直连。
- 杀毒软件可能隔离 WinDivert，分流异常时请先检查。

## 从源码构建

建议使用与 CI 一致的 Node.js 24、pnpm 10，以及 Rust 工具链和 [Tauri 2 Windows 构建依赖](https://tauri.app/start/prerequisites/)（MSVC 生成工具、WebView2）。

准备依赖与内置组件：

```powershell
pnpm install --frozen-lockfile
pnpm fetch-mihomo       # 下载并校验固定版本内核
pnpm fetch-windivert    # 下载 WinDivert 文件
pnpm build-proxybridge  # 使用 MSVC 编译 ProxyBridgeCore.dll
pnpm build-procsvc      # 编译进程分流服务
```

运行或打包应用：

```powershell
pnpm tauri dev        # 开发模式
pnpm tauri build      # NSIS 安装包位于 src-tauri/target/release/bundle
```

`pnpm tauri build` 会自动重新构建服务和前端。

直接运行 `cargo build --release --features custom-protocol` 前，需要先执行 `node scripts/build-procsvc.mjs` 和 `node scripts/build-frontend.mjs`。

### 检查

```powershell
pnpm typecheck
pnpm test
pnpm build
node --test scripts/build-artifacts.test.mjs scripts/updater-manifest.test.mjs
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace
```

部分 Rust 测试需要管理员权限或真实内核，默认跳过。运行真实内核集成测试：

```powershell
$env:MIHOMO_BIN = (Resolve-Path src-tauri/binaries/mihomo-x86_64-pc-windows-msvc.exe).Path
$env:NO_PROXY = ''
cargo test --locked --manifest-path src-tauri/Cargo.toml --test e2e_real_core -- --ignored --nocapture
```

技术栈：Tauri 2（Rust、tokio）、React、TypeScript、Vite 和 MUI，前后端绑定由 tauri-specta 生成。

## 许可证

[MIT](LICENSE)。第三方组件遵循各自许可证，另见 [ProxyBridge 许可证](third_party/proxybridge/LICENSE)及[上游说明](third_party/proxybridge/UPSTREAM.md)。
