# 讨论：取消全局「退出 Routa 后」设置

日期：2026-11-07　状态：**已定稿，并入 `2026-11-07-process-rule-name-path-block-design.md` 第 5 节**

> 用户的决定（2026-11-07）：采纳本提议；选项改名「Routa 或组件不可用时」；**不做老用户迁移**（第 4 节方案 A / B 都不用）；**新规则默认仍为直连**（第 4 节第 2 条不采纳）；与「进程名也能阻断」一起上线；托盘和进程分流页提供「停用进程分流（并退出）」，托盘退出时对阻断规则提示一次。以下为当时的讨论记录，具体以设计文档为准。

## 1. 现状（已按代码核对）

两个设置：

- 全局 `process_routing.on_routa_exit`（`ExitPolicy`，默认 **阻断**，见 `src-tauri/src/model/types.rs` `impl Default for ProcessRouting`）。
- 每条规则 `on_fault`（`FaultPolicy`，默认 **直连**，见 `types.rs` `ProcessRule` 默认值）。

| 场景 | 由谁决定 | 判定位置 | 执行方式 |
|---|---|---|---|
| Routa 正常退出 | 全局 `on_routa_exit` | 服务端 `procsvc/src/fallback.rs`（`RoutaGone::Graceful`） | 仅 ProxyBridge |
| 开机 / 服务重启后 Routa 尚未连上 | 全局 `on_routa_exit` | `fallback.rs`（`RoutaGone::NeverConnected`） | 仅 ProxyBridge |
| 内核正常启动中 | 全局 `on_routa_exit` | `src/procroute/decide.rs` `rule_action`（`CoreState::Starting` 且非恢复中） | 仅 ProxyBridge |
| 内核被用户手动停止 | 全局 `on_routa_exit` | `decide.rs`（`Stopped` + `CoreStopReason::User`） | 仅 ProxyBridge |
| 内核崩溃 | 规则 `on_fault` | `decide.rs`（`CoreState::Crashed`） | ProxyBridge；选阻断时另有 WFP 兜底 |
| 内核恢复中 | 规则 `on_fault` | `decide.rs`（`Starting` 且 `core_recovering`） | 同上 |
| Routa 崩溃 | 规则 `on_fault` | `fallback.rs`（`RoutaGone::Crashed`） | 同上 |
| 新配置无法生效、内核被防泄露停止 | 固定阻断 | `decide.rs`（`CoreStopReason::FailClosed`，优先于一切） | ProxyBridge（`on_fault=阻断` 的规则另有 WFP） |
| 内核非用户原因未运行 | 固定阻断 | `decide.rs`（`Stopped` 其他原因） | 同上 |
| 未选节点 / 节点失效 | 固定阻断 | `decide.rs`（`Running` 分支） | 同上 |

WFP 兜底的条件：`procsvc/src/reconcile.rs` 中 `guarded(r) = r.on_fault == FaultPolicy::Block`——**只看规则的 `on_fault`，与全局设置无关**。被 guarded 的 exe 装永久阻断，非阻断动作时再加随服务会话存在的动态放行（`permitted`）；服务一停，放行消失，阻断生效。

因此：全局「退出 Routa 后：阻断」只靠 ProxyBridge 执行，**分流服务停止时失效**。

补充（代码事实，背景中未提）：按进程名匹配的规则目前 `on_fault` 只能是直连（`types.rs` 注释：「按进程名匹配的规则没法在组件故障时阻断」），因为 WFP 需要完整路径。这影响下文的迁移方案，见第 4 节。

## 2. 提议

取消全局 `on_routa_exit`，上表中原由全局决定的四个场景，统一改为按每条规则自己的 `on_fault` 处理。选项名可考虑改为「Routa 或组件不可用时」，以覆盖退出、未连上、内核启动/手动停止、崩溃等全部场景。

理由：

- 两个设置交叉覆盖不同场景，用户难以理解。
- 全局阻断在服务停止时悄悄失效，名不副实。
- 合并后，凡选阻断的规则都有 WFP 防火墙兜底，行为一致。

## 3. 防漏效果对比

| 方面 | 现状 | 提议后 | 结论 / 补法 |
|---|---|---|---|
| `on_fault=直连` 的规则在 Routa 正常退出、未连上、内核启动中/手动停止时 | 若全局为阻断（默认），这些规则也被阻断 | 按规则直连 | **可能变弱**。补法：迁移方案 A（全局原为阻断 → 所有规则改为阻断）；新规则默认改为阻断 |
| 分流服务停止后，选阻断的规则 | 全局阻断失效；仅 `on_fault=阻断` 的有 WFP | 所有选阻断的规则都有 WFP | **变强** |
| 按进程名的规则 | 无法在故障时阻断，也无 WFP | 依赖「进程名也能阻断、自动学习程序路径」设计落地后，同样受 WFP 保护 | **变强**（前提：那项设计实现） |
| 用户主动给某条规则选直连 | — | 该规则在所有不可用场景直连 | 属明确选择，不算漏 |

总体：按「迁移 A + 新规则默认阻断」，防漏效果不降反升。

## 4. 配套改动

1. **迁移（推荐方案 A）**：老配置中 `on_routa_exit == Block` 时，把所有规则的 `on_fault` 改为阻断（只会更严，且多了 WFP 兜底）；`on_routa_exit == Direct` 时规则保持不变。
   - 注意：在「进程名也能阻断」实现之前，按进程名的规则不能设为阻断。迁移 A 要么与那项设计同时上线，要么对这类规则暂留直连并单独提示。
   - 方案 B：不迁移，只提示一次。设计者不推荐。
2. **新规则默认值**：`on_fault` 默认由直连改为阻断，与原全局默认阻断的防护水平一致。
3. **「停用进程分流」入口**：托盘和主界面提供「停用进程分流」和「停用进程分流并退出」。
4. **退出提示**：直接退出 Routa 且存在阻断规则时，提示一次「这些程序会断网，直到 Routa 再次运行」。
5. 代码层面需要删除/调整：`ExitPolicy` 及 `ProcessRouting.on_routa_exit`、`decide.rs` 的 `exit_action` 分支、`fallback.rs` 的 Graceful/NeverConnected 分支、`DesiredState.on_routa_exit`、相关测试和 UI；旧字段需保留只读反序列化以便迁移。

## 5. 失去的能力及替代

失去：同一条规则「崩溃时阻断、主动退出 Routa 时直连」的组合。

替代：需要退出 Routa 又希望程序能联网时，使用「停用进程分流并退出」（或先「停用进程分流」）。

## 6. 尚未决定、需要用户拍板

1. 是否采纳本提议（取消全局「退出 Routa 后」）。
2. 选项新名称：「Routa 或组件不可用时」还是其他。
3. 迁移方案：A（推荐）还是 B。
4. 迁移与「进程名也能阻断」设计的上线顺序；若先上本提议，按进程名的规则如何处理。
5. 新规则 `on_fault` 默认值是否改为阻断。
6. 「停用进程分流（并退出）」入口放在哪些位置、退出提示的文案与「不再提示」行为。
7. 是否同步修改 `2026-11-07-process-rule-name-path-block-design.md` 中按全局「退出 Routa 后」处理的描述。
