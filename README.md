# Routa

English | [简体中文](README.zh-CN.md)

A Windows desktop proxy manager built on [mihomo](https://github.com/MetaCubeX/mihomo). Choose a node and connect in Simple mode, or manage multiple proxy ports, routing rules, and network interfaces in Full mode—without writing YAML.

> This README describes the current source. Published release builds may have fewer features. Windows x64 only; the application UI is currently in Simplified Chinese.

## Features

- **Two interface modes**: Simple mode focuses on node selection and connecting; Full mode provides an overview, proxy management, process routing, subscriptions, and nodes. Switch modes in Settings while keeping existing configuration and connections.
- **Three ways to connect**: Windows system proxy, selected applications through process routing, or TUN through a virtual adapter. TUN requires running Routa as administrator.
- **Independent proxy ports**: create `mixed` (HTTP + SOCKS5), `http`, or `socks` listeners. Configure each port's node, proxy group or direct outbound, network interface, authentication, and LAN access together.
- **Network interface isolation**: when a configured outbound becomes unavailable or its bound adapter disconnects, the affected listener is disabled rather than silently switching to direct or the default adapter. It resumes when the adapter is available again.
- **Per-port routing rules**: match an exact domain, domain suffix, or IPv4 CIDR in order, then proxy, connect directly, or reject. Set the action for unmatched traffic and check sample destinations in the editor.
- **Subscriptions and nodes**: import Clash/mihomo YAML or base64 share-link subscriptions, schedule updates, and select a fallback proxy for subscription downloads. Create HTTP/SOCKS5 nodes with a form or import other mihomo node types from YAML, including batch imports. Search nodes, sort by latency, and organize custom groups.
- **Proxy groups and Tailscale**: use `url-test`, `fallback`, `load-balance`, or `select` groups; manage Tailscale nodes, login state, and exit settings.
- **Process routing**: route programs by full executable path to Mixed/Socks listeners, inspect per-process connections and traffic, and choose direct or block on component failure. A separate policy controls behavior when Routa exits normally.
- **Monitoring and diagnostics**: view core and capture status separately, live and total traffic, active connections and their routes, core logs, port conflicts, and component status.
- **Desktop integration**: bundled mihomo core, configuration hot reload, automatic core restart after a crash, Windows startup, optional auto-connect, and a choice of minimizing to the tray or exiting when closing the window.

## Install

Download a Windows x64 build from [Releases](https://github.com/c0010c/Routa/releases). Available package formats are:

| Package | Usage |
| --- | --- |
| `routa_<version>_x64-setup.exe` | NSIS installer |
| `routa_<version>_x64_en-US.msi` | MSI installer |
| `routa_<version>_x64_portable.zip` | Extract and run `routa.exe` |

Verify downloads against `SHA256SUMS.txt` on the corresponding release. Builds bundle mihomo; the source pins its version in [`MIHOMO_VERSION`](src-tauri/binaries/MIHOMO_VERSION). The current source uses the bundled core rather than a custom core path.

The [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) is required.

## Quick start

### Simple mode

1. On first launch, choose **简洁版** (Simple).
2. Import a subscription or add a node manually, then select a node.
3. Choose **系统代理** (System proxy), **指定应用** (Selected apps), or **TUN** and turn the connection on. Selected apps requires installing the process-routing component and adding applications; TUN requires restarting Routa as administrator.
4. Open Settings to change interface mode, auto-connect, startup behavior, or the close-window action.

System proxy affects applications that honor Windows proxy settings. For applications that bypass those settings, use selected-app routing or TUN as appropriate.

### Full mode

1. Add subscriptions or nodes from **订阅** (Subscriptions) or **节点** (Nodes).
2. Create a proxy on **代理** (Proxies): set its port, outbound node/group/direct connection, and optionally routing rules, network interface, authentication, or LAN access.
3. Point a client at that port, or select a default proxy on **概览 → 运行概况** (Overview → Runtime summary) and enable system proxy or TUN. System proxy requires a Mixed listener bound to local IPv4.
4. Use **进程分流** (Process routing) to assign individual applications to proxies. Inspect **概览 → 连接 / 日志** (Overview → Connections / Logs) when troubleshooting.

Stopping system proxy or TUN leaves the core and proxy ports available; manually configured clients and independently enabled process-routing rules may still use them.

## Data and upgrades

Installed builds store data in `%APPDATA%\dev.routa\`:

| File or directory | Contents |
| --- | --- |
| `state.json` | Proxy, node, subscription, and process-routing configuration |
| `ui-preferences.json` | Interface mode, default proxy, and connection preferences |
| `mihomo\` | Core working directory and `mihomo.log` |

When a `portable` file sits next to `routa.exe`, all application data, including the WebView cache, is stored in the adjacent `data\` directory. Remove the marker and restart to use `%APPDATA%\dev.routa\` instead. Switching storage modes does not move data automatically.

- Keep portable builds in a writable directory, outside `Program Files`.
- After moving a portable folder, turn Windows startup off and on again to update its recorded executable path.
- Installed and portable builds cannot run simultaneously.
- Existing Routa configurations open in Full mode when no interface preference has been saved. The current source migrates legacy shared outbounds into per-proxy configurations and saves the original configuration as `state.before-v43.json` before migration.
- Version 0.1.0 used the name `mihomo-mapper` and `%APPDATA%\dev.mihomo-mapper\`. That directory is not imported automatically into Routa.

## Process-routing component

Process routing runs as the `RoutaProcSvc` Windows service, built on [ProxyBridge](https://github.com/InterceptSuite/ProxyBridge) and [WinDivert](https://github.com/basil00/WinDivert). Its files are installed in `%ProgramFiles%\Routa\ProcessRouting`.

The NSIS installer requests UAC at the end of setup to register the service. Declining does not fail installation; enable the component later in the app. MSI and portable builds request UAC when enabling the component from the app.

**Before uninstalling an MSI or removing a portable build, uninstall the component from the app.** NSIS uninstallation requests UAC to remove the service and filter rules, with a retry prompt if removal fails. If the component remains installed, programs configured to block may stay offline. To remove the service and rules manually, run this in an administrator PowerShell:

```powershell
& "$env:ProgramFiles\Routa\ProcessRouting\routa-procsvc.exe" --uninstall
```

The command cannot delete its own running executable; remove the remaining `ProcessRouting` folder afterwards.

Limitations and behavior:

- DNS requests made by the Windows DNS Client service bypass process routing. Process routing alone does not proxy those DNS requests.
- Executable-path rules apply to that program for every Windows user on the computer.
- Targets must be Mixed/Socks listeners with IPv4 endpoints, such as `127.0.0.1` or `0.0.0.0`. HTTP-only and IPv6 listener endpoints are unsupported; IPv6 destinations through an IPv4 proxy endpoint are still supported.
- Block policies use Windows Filtering Platform (WFP) rules that persist across component crashes, service stops, and restarts. Rules configured to go direct on failure may connect directly before the service starts after boot.
- A missing, disabled, or ineffective target listener blocks the routed application. When the core crashes, the rule's failure policy applies; normal exit uses the separate exit policy.
- Disconnecting existing connections affects IPv4 TCP only. Changing the proxy target without changing the routing action does not trigger a disconnect.
- Antivirus software may flag or quarantine WinDivert. Check the component files and service status if routing fails.
- The current process-routing protocol is version 3. Update the GUI and service together. Failed configurations require a manual retry after reconnecting.

## Build from source

Use Node.js 24 and pnpm 10 to match CI, plus the Rust toolchain and [Tauri 2 Windows prerequisites](https://tauri.app/start/prerequisites/) (MSVC build tools and WebView2).

Prepare dependencies and bundled components:

```powershell
pnpm install --frozen-lockfile
pnpm fetch-mihomo       # Download and verify the pinned core
pnpm fetch-windivert    # Download the WinDivert files
pnpm build-proxybridge  # Build ProxyBridgeCore.dll with MSVC
pnpm build-procsvc      # Build the process-routing service
```

Run or package the application:

```powershell
pnpm tauri dev        # Development mode
pnpm tauri build      # NSIS/MSI installers in src-tauri/target/release/bundle
pnpm pack:portable    # Portable folder and ZIP in bundle/portable-<timestamp>
```

`pnpm tauri build` rebuilds the service and frontend automatically. Portable packaging builds the production app and includes `routa.exe`, `mihomo.exe`, `routa-procsvc.exe`, `procsvc\`, the `portable` marker, `LICENSE`, and `build-info.json`.

`build-info.json` records GUI, service, frontend, and file SHA-256 fingerprints. `--skip-build` still verifies source, protocol, and artifact consistency. Use `--out-dir <directory>` to choose the portable output location; existing app folders or ZIPs are rejected to protect any `data` directory. Before a direct `cargo build --release --features custom-protocol`, run `node scripts/build-procsvc.mjs` and `node scripts/build-frontend.mjs`.

### Checks

```powershell
pnpm typecheck
pnpm test
pnpm build
node --test scripts/build-artifacts.test.mjs
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace
```

Some Rust tests are ignored by default because they need administrator rights or a real core. To run the real-core integration test:

```powershell
$env:MIHOMO_BIN = (Resolve-Path src-tauri/binaries/mihomo-x86_64-pc-windows-msvc.exe).Path
$env:NO_PROXY = ''
cargo test --locked --manifest-path src-tauri/Cargo.toml --test e2e_real_core -- --ignored --nocapture
```

Stack: Tauri 2 (Rust and tokio), React, TypeScript, Vite, and MUI. Frontend/backend bindings are generated with tauri-specta.

## License

[MIT](LICENSE). Third-party components retain their respective licenses; see [ProxyBridge's license](third_party/proxybridge/LICENSE) and [upstream notes](third_party/proxybridge/UPSTREAM.md).
