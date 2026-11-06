# 端到端验收

## 自动化验证（已完成）

| 项目 | 命令 | 结果 |
| --- | --- | --- |
| Rust 单元/集成测试 | `cargo test --manifest-path src-tauri/Cargo.toml` | 135 passed, 0 failed, 4 ignored（3 个原有 + e2e），无警告 |
| 前端测试 | `pnpm test` | 13 个文件，56/56 通过 |
| 类型检查 | `pnpm typecheck` | 通过，无输出 |
| 真实内核 e2e | `MIHOMO_BIN=<src-tauri/binaries/mihomo-x86_64-pc-windows-msvc.exe> cargo test --test e2e_real_core -- --ignored --nocapture` | 通过（11.77s） |
| bindings 复核 | 运行 debug 版让 tauri-specta 导出，再 `git diff src/bindings.ts` | 除换行符（LF/CRLF）外无差异，无需更新 |
| 安装包构建 | `pnpm tauri build` | 成功，见下 |

### e2e_real_core 覆盖内容

真实 `Runtime` + `ProcessLauncher` + `HttpMihomoApi` + `WindowsPortOwners` + `WindowsInterfaces`，临时 Store 目录，一个直连出口 + 两个 Listener，控制端口和监听端口都取空闲端口：

- `curl -x http://127.0.0.1:<p1> https://www.gstatic.com/generate_204` 返回 204（本机可直连外网，未用本地回退目标）；Listener 2 经本地 HTTP 目标同样返回 204
- 慢请求进行中，`/connections` 中出现 `inboundName == "监听一"`
- 停用 Listener 2（`mutate`）后其端口拒绝连接，Listener 1 仍可用
- `shutdown` 后 mihomo 进程消失，Listener 1 端口关闭

### 安装包

- NSIS：`src-tauri/target/release/bundle/nsis/mihomo-mapper_0.1.0_x64-setup.exe`（18.73 MiB）
- MSI：`src-tauri/target/release/bundle/msi/mihomo-mapper_0.1.0_x64_en-US.msi`（26.64 MiB）
- 未安装。构建时 crates.io 不可达，仅在命令行使用 rsproxy 镜像（未提交任何镜像配置）；NSIS 工具从 GitHub 正常下载。

## 手动验收清单（待用户验证）

先安装 NSIS 安装包并启动。每项完成后在“结果”列填写 通过 / 失败 及备注。

| # | 步骤 | 预期 | 结果 |
| --- | --- | --- | --- |
| 1 | 订阅页添加一个真实的 YAML 格式订阅并更新 | 显示成功及节点数；节点页能看到节点，测速有延迟值 | 待用户验证 |
| 2 | 再添加一个真实的 base64 格式订阅并更新 | 同上 | 待用户验证 |
| 3 | 使用两张网卡（如有线 + Wi-Fi）。建两个出口（直连），分别绑定网卡 A、B；建两个 Listener 各用一个出口。对两个 Listener 端口分别执行 `curl -x http://127.0.0.1:<端口> https://www.gstatic.com/generate_204`，同时用 Wireshark 在两张网卡上抓包 | 经 Listener 1 的流量只从网卡 A 出去，经 Listener 2 的只从网卡 B 出去 | 待用户验证 |
| 4 | 局域网手机设置 HTTP 代理为 `<电脑局域网IP>:<端口>`，Listener 不设认证，访问网页 | 可以上网 | 待用户验证 |
| 5 | 给该 Listener 设置用户名/密码；手机先不填认证，再填正确认证 | 不填认证被拒绝（407 或无法连接）；填写后可上网 | 待用户验证 |
| 6 | 拔掉某个绑定了网卡的网线/断开 Wi-Fi | 3 秒内对应 Listener 被停用，概览页出现横幅；该端口不再放行流量 | 待用户验证 |
| 7 | 把网卡插回/重新连接 | Listener 自动恢复，横幅消失 | 待用户验证 |
| 8 | 把某订阅 URL 改成无效地址并点更新 | 显示更新失败及原因；旧节点仍保留 | 待用户验证 |
| 9 | 在任务管理器中结束本工具启动的 mihomo 进程（不要碰 Clash Verge 的 verge-mihomo.exe） | 工具自动重启 mihomo，Listener 恢复可用 | 待用户验证 |
| 10 | 点击窗口关闭按钮 | 窗口隐藏，托盘图标保留，mihomo 仍在运行 | 待用户验证 |
| 11 | 托盘菜单选“退出” | 工具退出，任务管理器中 mihomo 进程消失 | 待用户验证 |
| 12 | 设置页开启开机自启，重启电脑 | 登录后工具与 mihomo 自动运行，Listener 可用 | 待用户验证 |
| 13 | 设置页把测速间隔设为 1 分钟，保持窗口打开，观察节点页/出口页的延迟数值（测速前可先手动点一次“测速”，或用工具里时间会变化的数值对比） | 延迟数值约每分钟刷新一次（数值会有小幅变化；界面不显示测速时间，需持续观察几分钟） | 待用户验证 |
| 14 | 把某订阅更新间隔设为 2 小时（可输入的最小单位是 1 分钟，如想快速验证可临时设为 2 分钟），记下节点页订阅卡片上的“最后更新”时间 | 约 2 小时（或临时设置的 2 分钟）后，“最后更新”时间前进到新的时间点 | 待用户验证 |

### 来自 spike S2 的遗留物理检查

spike S2 仅通过代理验证了“网卡不存在时拨号失败且不回落到默认网卡”，下列物理检查延后到此处，对应上表第 3、6、7 项：

- 第二张网卡上的抓包确认（第 3 项）
- 物理拔线（第 6、7 项）

## 手动节点 / 节点组 / Tailscale / 总览页验收

### 自动化验证（本次运行）

| 项目 | 命令 | 结果 |
| --- | --- | --- |
| Rust 测试 | `cargo test --manifest-path src-tauri/Cargo.toml` | 218 passed, 0 failed, 4 ignored（lib）；e2e_real_core 1 ignored |
| 前端测试 | `pnpm test` | 17 个文件，111/111 通过 |
| 类型检查 | `pnpm typecheck` | 通过，无输出 |

### 手动验收清单（待人工验收）

先启动 `pnpm tauri dev`（或安装包）。每项完成后在“结果”列填写 通过 / 失败 及备注。

| # | 步骤 | 预期 | 结果 |
| --- | --- | --- | --- |
| 1 | 新建 socks5 手动节点，放进自建组“家里” | 节点与组创建成功，节点页/组中可见 | 待人工验收 |
| 2 | 用粘贴 YAML 的方式建一个 vless 节点，且 YAML 中含 `dialer-proxy` | 创建成功，并提示 `dialer-proxy` 被忽略 | 待人工验收 |
| 3 | 建一个 url-test 出口，成员选“机场A（整组）”加上“家里” | 出口展开后的节点数正确（机场A 全部节点 + 家里组内节点，无重复） | 待人工验收 |
| 4 | 新建监听器 | 自动跳到总览页，并高亮“未接出口”的那一行；在行内选出口后，状态变为“已生效” | 待人工验收 |
| 5 | 在 select 类型出口的行内切换节点 | 切换成功，经该出口的流量改走新节点 | 待人工验收 |
| 6 | 删除正被出口使用的“家里”组 | 删除被拒绝，并提示是哪个出口在用 | 待人工验收 |
| 7 | 把旧版 `state.json` 拷进数据目录后启动 | 原有出口、监听器全部正常工作，无数据丢失 | 待人工验收 |
| 8 | Tailscale 页新建实例（exit-node 填出口节点 IP），点“复制链接”，在浏览器完成授权后状态变为“已登录”；把监听器接到含它的出口，`curl -x http://127.0.0.1:<端口> https://www.gstatic.com/generate_204` 能通，出口 IP 为 exit node 的 IP | 授权后状态变为“已登录”；经监听器的请求出口 IP 为 exit node 的 IP | 待人工验收 |

## 进程分流

### 自动化验证（本次运行）

| 项目 | 命令 | 结果 |
| --- | --- | --- |
| Rust 测试 | `cargo test --manifest-path src-tauri/Cargo.toml --workspace` | routa 286 passed / 5 ignored；routa-procsvc 77 passed / 6 ignored（需要管理员）；proto 4 passed；e2e 1 ignored |
| 前端测试 | `pnpm test` | 25 个文件，240/240 通过 |
| 类型检查 | `pnpm typecheck` | 通过，无输出 |
| 安装包构建 | `pnpm fetch-mihomo && pnpm fetch-windivert && pnpm build-proxybridge && pnpm tauri build` | 生成 `routa_0.2.0_x64-setup.exe`（19.05 MiB）和 `routa_0.2.0_x64_en-US.msi`（27.04 MiB） |
| 便携版 | `pnpm pack:portable --skip-build` | zip 内含 `routa-procsvc.exe` 和 `procsvc\` 下 3 个文件 |

单元测试覆盖 spec 第 9 节的状态 → 实际动作映射、下发顺序、助手的全量同步 / 断开 / 重连 / 版本不一致，以及规则对话框的校验。

### 打包与安装钩子

- NSIS 安装包、MSI、便携版 zip 都包含 `routa-procsvc.exe` 和 `procsvc\`（`ProxyBridgeCore.dll`、`WinDivert.dll`、`WinDivert64.sys`）。
- NSIS 安装包按当前用户安装、本身不提权：安装完成后单独弹一次 UAC，以管理员身份运行 `routa-procsvc.exe --install` 注册服务；卸载前同样弹一次 UAC 运行 `--uninstall`（服务和组件目录都不存在时跳过；安装程序覆盖安装时调用的卸载也跳过，见 `src-tauri/nsis/hooks.nsh`）。拒绝 UAC 或命令失败不影响安装 / 卸载，之后可在“进程分流”页面启用或卸载组件。静默安装（`/S`）且安装程序不在前台时，UAC 可能只在任务栏闪烁，安装会一直等到用户响应。
- MSI 安装时不注册服务，和便携版一样在页面上点“启用进程分流”时弹 UAC。**卸载 MSI 前请先在页面上点“卸载组件”**，否则服务和防护规则会留在系统里（之后可用便携版或 `routa-procsvc.exe --uninstall` 清理）。

### 准备

- 需要管理员账号（UAC）。WinDivert 驱动可能被 Windows Defender 或其他杀毒软件报毒、隔离：如出现组件文件缺失、服务启动失败，请把安装目录（安装版为 `%LOCALAPPDATA%\routa`，组件目录为 `%ProgramFiles%\Routa\ProcessRouting`）加入排除项；开发时还要排除 `src-tauri\binaries\procsvc` 和 `third_party\windivert`。
- 准备一个 Mixed 或 Socks Listener，记下它的出口 IP；用 `C:\Windows\System32\curl.exe` 作为被管程序。
- 检查是否残留 Routa 的 WFP 过滤器：管理员命令行执行 `netsh wfp show filters file=wfp.xml`，在 `wfp.xml` 中搜索 `5f3b2c1e-7a44-4d2b-9c51-0b6f2a7e1d0`（Routa 的 provider / sublayer GUID 前缀）。

### 手动验收清单（待人工验收）

每项完成后在“结果”列填写 通过 / 失败 及备注。“Block 规则”“Direct 规则”指“组件故障时”分别设为阻断、直连的规则。

| # | 步骤 | 预期 | 结果 |
| --- | --- | --- | --- |
| 1 | 安装 NSIS 安装包，在安装末尾弹出的 UAC 中点“是” | `sc query RoutaProcSvc` 为 RUNNING；进程分流页组件状态为“运行中” | 待人工验收 |
| 2 | 新建规则：`curl.exe` → 该 Listener；执行 `curl.exe https://api.ipify.org` | 返回 Listener 出口的 IP；规则行防护状态为“已代理”，展开后能看到连接和速率 | 待人工验收 |
| 3 | 修改规则的目标 Listener（“断开旧连接”分别为开 / 关），期间保持一个长连接（如 `curl.exe --limit-rate 50k -o NUL <大文件 URL>`） | 不重启组件，新连接走新 Listener；更换代理目标不算动作切换，开关为开或关时均不主动断开旧连接（2026-10-29 用户确认） | 待人工验收 |
| 4 | 在任务管理器中结束 `routa-procsvc.exe`（服务会按故障恢复自动重启，结束后立刻测试） | Block 规则的程序连不上外网，Direct 规则的程序能直连；服务恢复后重新走代理 | 待人工验收 |
| 5 | 管理员命令行 `sc stop RoutaProcSvc`；之后重启电脑 | 两种情况下 Block 规则的程序都保持阻断 | 待人工验收 |
| 6 | 让 Listener 被停用（停用该 Listener，或让它的出口节点失效 / 网卡断开） | 无论“组件故障时”设为什么，程序都被阻断，状态显示原因（如“Listener 未生效”） | 待人工验收 |
| 7 | 在任务管理器中结束 Routa 启动的 mihomo 进程 | Direct 规则改走直连，Block 规则阻断；mihomo 自动恢复后切回代理 | 待人工验收 |
| 8 | 分别：从托盘“退出” Routa；手动停止内核 | 按“Routa 不提供代理时”的设置直连或阻断 | 待人工验收 |
| 9 | 让新配置无法生效导致内核被自动停止（例如占用某 Listener 的端口后修改配置） | 一律阻断 | 待人工验收 |
| 10 | 用任务管理器结束 Routa | 按每条规则自己的“组件故障时”设置 | 待人工验收 |
| 11 | 重启电脑，登录后不启动 Routa | 按“Routa 不提供代理时”的设置；开机到服务启动前的短暂时间内，Direct 规则的程序可能直连 | 待人工验收 |
| 12 | 同时安装并运行官方 ProxyBridge | 两边都正常工作（中转端口不冲突）；组件状态卡提示可能冲突 | 待人工验收 |
| 13 | 把 `curl.exe` 复制到中文路径（如 `C:\测试\curl.exe`）并为它建规则 | 规则能匹配，流量走指定 Listener | 待人工验收 |
| 14 | 给 `curl.exe` 建 Block 规则并让它处于阻断状态（如停用 Listener）；在本机起一个 HTTP 服务（如 `python -m http.server 8000 --bind ::`），执行 `curl.exe http://127.0.0.1:8000/` 和 `curl.exe http://[::1]:8000/` | 两个请求都成功；访问外网仍被阻断 | 待人工验收 |
| 15 | 关闭进程分流全局开关；再打开 | 关闭后程序按系统默认直连、不残留阻断；打开后恢复 | 待人工验收 |
| 16 | 页面上点“清理残留阻断规则”、“卸载组件” | 清理后 WFP 中搜不到 Routa 的阻断过滤器；卸载后服务和 `%ProgramFiles%\Routa\ProcessRouting` 都不存在 | 待人工验收 |
| 17 | 便携版：解压后首次点“启用进程分流” | 弹一次 UAC，服务注册并运行 | 待人工验收 |
| 18 | 保持一条 Block 规则处于阻断状态（如停用其 Listener），持续执行 `curl.exe https://api.ipify.org`。用 NSIS 安装包覆盖安装：同版本时分别选“添加/重新安装”和“卸载”；（可临时改版本号构建一个更高版本）升级时分别选“先卸载再安装”和“不卸载”；每种情况下都分别在 UAC 中点“是”和“否”各试一次 | 整个过程中 curl 始终被阻断；卸载阶段不弹 UAC、不删除服务（详细日志显示“覆盖安装，保留服务和防护规则”）；安装末尾的 UAC 选“否”时服务和阻断仍在 | 待人工验收 |
| 19 | Routa 运行时卸载 NSIS 安装版，在“Routa 正在运行”的提示中点“取消” | 卸载中止，服务和防护规则都还在（未弹 UAC） | 待人工验收 |
| 20 | 卸载 NSIS 安装版，在 UAC 中点“是” | `sc query RoutaProcSvc` 报服务不存在；`%ProgramFiles%\Routa\ProcessRouting` 不存在；`netsh wfp show filters` 的输出中搜不到 Routa 的 GUID | 待人工验收 |
| 21 | 使一条代理规则应用失败，保持设置不变并观察多次状态刷新，再点击错误提示中的“重试” | 确认阻断成功才显示“已阻断”；无法确认时显示“状态未知”；不自动重发失败配置，点击后只按当前设置重试一次；成功清除错误，失败继续提示 | 待人工验收 |
| 22 | 模拟断开旧 IPv4 TCP 连接失败，规则本身成功；故障消失后点击“重试” | 代理动作保持成功，单独显示断连失败；刷新状态不反复断连；点击后重试失败的断连，成功后清除提示 | 待人工验收 |
| 23 | 新建规则选择监听 `::1` / `::` 的 Listener；将已被分流规则引用的 IPv4 Listener 改为 IPv6；载入旧的 IPv6 入口规则 | 前两者保存失败并明确提示仅支持 IPv4 入口；旧规则运行时阻断并显示原因，即使故障策略为直连也不换地址或直连；未被进程分流引用的 IPv6 Listener 仍允许保存 | 待人工验收 |
| 24 | 设置中把控制端口改成已有 Listener 的端口并保存，再修改为其他端口 | 第一次保存失败，控制端口标红并显示冲突原因及可识别的 Listener 名称，旧设置继续生效；编辑后清除旧错误，手动再次保存才提交 | 待人工验收 |
| 25 | 配置“故障时直连、正常退出时阻断”，制造一次应用失败后断开并恢复 GUI 与服务的管道；不点击重试，正常退出 GUI | 重连只恢复会话登记、不重试失败配置，正常退出仍阻断；另测服务重启后重连及 GUI 再次崩溃 | 待人工验收 |
| 26 | 保持游戏或语音 UDP 会话，另建多条连接到延迟或不响应握手的 SOCKS5 代理；在握手期间更改规则、停止并重新启动组件 | 已有会话继续收发；慢握手超时只影响自身；旧会话待发数据不进入新会话；停止无需等待握手截止时间 | 待人工验收 |
