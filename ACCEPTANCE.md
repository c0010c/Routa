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
| 13 | 设置测速间隔为 1 分钟，查看日志（设置页“打开日志目录”） | 约每分钟触发一次测速 | 待用户验证 |
| 14 | 把某订阅更新间隔设为 2 小时，查看日志 | 约 2 小时后触发更新 | 待用户验证 |

### 来自 spike S2 的遗留物理检查

spike S2 仅通过代理验证了“网卡不存在时拨号失败且不回落到默认网卡”，下列物理检查延后到此处，对应上表第 3、6、7 项：

- 第二张网卡上的抓包确认（第 3 项）
- 物理拔线（第 6、7 项）
