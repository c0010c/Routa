# AI 应用页（内置进程分流 + 启动器）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增"AI 应用"页：远程拉取的内置应用卡片（ChatGPT / Claude / Cursor），一个开关让应用走自己选的节点，"启动"按钮自动备好内核、分流服务和规则后打开应用。

**Architecture:** AI 应用的规则就是普通的 `ProcessRule`，存放在 `process_routing.rules` 里，用新字段 `ai_app: Option<String>`（应用 id）做标记。这样隐藏监听器、`decide`、断网守护、退出提示、已知路径学习都能直接复用。进程分流页和简洁版把带标记的规则过滤掉，界面上两边仍然完全分开。新的 Rust 模块 `aiapps` 负责：应用列表（远程拉取 + 缓存 + 内置兜底）、本机检测、启动和置前、规则变换（冲突移除、启用、停用）。`Desktop` 层负责编排"自动备好一切"，并等服务确认规则生效。分流服务改为：换代理目标、删除规则时，也断开旧连接。

**Tech Stack:** Rust（Tauri 2、tauri-specta、tokio、reqwest、`windows` 0.62 / `windows-sys` 0.61），React 19 + MUI 9 + TanStack Query，vitest。

**Spec:** `docs/superpowers/specs/2026-11-10-ai-apps-launcher-design.md`

## Global Constraints

- 仅 Windows。非 Windows 用 `cfg(not(windows))` 桩函数，只为能编译（沿用 `desktop.rs` 的做法）。
- 菜单项"AI 应用"放在"进程分流"上面。简洁版（`SimpleLayout`）**不加**入口。
- AI 应用规则不出现在进程分流列表里，也不出现在简洁版的应用列表里。
- 卡片上只有：图标、名称、节点按钮 + 延迟 + 开关、一个主按钮、右上角小圆点。不要状态文字。
- 远程列表只能描述应用。启动的程序必须来自本机检测结果；列表里不能写命令行或参数。
- 被启动的应用一律以普通用户权限运行：通过 `explorer.exe` 启动，即使 Routa 自己是管理员身份也一样。
- 列表地址：`https://raw.githubusercontent.com/c0010c/Routa/main/presets/ai-apps.json`。依次尝试 `https://gh-proxy.org/<地址>`、`https://ghfast.top/<地址>`、`<地址>`，每个来源超时 10 秒。启动时拉一次，之后每 24 小时拉一次。
- 缓存文件：`<app_data_dir>/ai-apps.json`。内置兜底：`presets/ai-apps.json`，通过 `include_str!` 编译进程序。
- `schema_version` 必须等于 `1`，否则整份列表作废。
- AI 应用规则固定 `reset_on_change = true`。`on_fault` 取列表中的值，缺省为 `block`。
- 分流服务协议版本 `PROTO_VERSION` 从 `5` 提升到 `6`。
- 界面文案（逐字使用）：
  - 卡片按钮："启动" / "运行中" / "未安装 · 下载"
  - 浅色开关的悬停提示："进程分流已关闭，开启后恢复"
  - 冲突框：
    - 标题："与进程分流规则冲突"
    - 每项：`规则「{规则名}」里的 {进程} 也在分流。`
    - 说明：`开启后会把它从该规则中移除，规则里的其他进程不受影响。`；整条规则会被删除时改为：`开启后会删除该规则。`
    - 按钮："取消" / "移除并开启"
  - 断开框：
    - 标题：`断开 {应用名} 的现有连接`
    - 正文：`会断开 {应用名} 已有的连接，让它重新连接。应用可能会短暂显示重连。`
    - 勾选框："下次不再提示"
    - 按钮："取消" / "继续"
  - 设置页开关："AI 应用断开连接前提示"
  - 反向冲突的校验错误：`{进程} 已由 AI 应用「{应用名}」管理，请在 AI 应用页操作`

## Review Focus

1. **远程列表损坏、为空或被篡改**（JSON 坏掉、`schema_version` 不对、`icon` 不是 `data:image/`、链接不是 https、进程名带路径）：应该整份作废，回退到缓存或内置版本，页面照常显示。测试：Task 3 的 `rejects_*`。
2. **列表更新后某个应用的进程清单变了，或者应用被移除**：已有的 AI 规则应该同步成新清单；被移除的应用，其规则应该删除，不能留下一条界面上看不到、也关不掉的规则。测试：Task 4 的 `sync_*`。
3. **手动规则里按完整路径写了 `C:\…\ChatGPT.exe`**，而 AI 规则按进程名匹配：开启时也要算冲突。否则服务端按路径匹配优先，AI 开关会形同虚设。测试：Task 4 的 `conflict_matches_path_file_name`。
4. **开启过程中服务确认失败，或者内核重载失败**：要整体回滚，包括恢复已经被移除的冲突进程，不能只回滚 AI 规则。测试：Task 6 的 `enable_reverts_whole_routing_on_failure`。
5. **应用在运行，但只在托盘里，没有可见窗口**：点"运行中"应该退回"再启动一次"，不能没有反应。测试：Task 5 的 `focus_falls_back_to_launch`（纯逻辑部分）。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `src-tauri/procsvc/src/reconcile.rs` | 修改：第 9 条 Reset 规则 |
| `src-tauri/procsvc-proto/src/lib.rs` | 修改：`PROTO_VERSION = 6` |
| `src-tauri/src/model/types.rs` | 修改：`ProcessRule.ai_app` |
| `src-tauri/src/model/validate.rs` | 修改：新增错误 `AiAppProcess` |
| `src-tauri/src/commands.rs` | 修改：`save_process_rule` 保留 `ai_app`，并拒绝编辑 AI 规则 |
| `presets/ai-apps.json` | 新建：内置兜底，同时也是远程列表的源文件 |
| `src-tauri/src/aiapps/mod.rs` | 新建：模块入口、`AiApps` 状态 |
| `src-tauri/src/aiapps/catalog.rs` | 新建：列表类型、解析、校验 |
| `src-tauri/src/aiapps/fetch.rs` | 新建：镜像拉取、缓存、兜底、定时刷新 |
| `src-tauri/src/aiapps/rules.rs` | 新建：冲突检测、启用、停用、与列表同步（纯函数） |
| `src-tauri/src/aiapps/detect.rs` | 新建：本机检测 |
| `src-tauri/src/aiapps/launch.rs` | 新建：启动、置前 |
| `src-tauri/src/aiapps/commands.rs` | 新建：Tauri 命令和视图类型 |
| `src-tauri/src/desktop.rs` | 修改：`enable_ai_app` / `disable_ai_app` / `prepare_ai_app`、偏好 `skip_ai_reset_prompt` |
| `src-tauri/src/runtime/mod.rs` | 修改：`test_delay_at` |
| `src-tauri/src/lib.rs` | 修改：注册命令、管理状态、启动时拉列表、启动时自动起内核 |
| `src-tauri/Cargo.toml` | 修改：`windows` 增加 features `Management_Deployment`、`Foundation_Collections` |
| `src/bindings.ts` | 重新生成 |
| `src/api/aiApps.ts` | 新建：查询和变更 hooks |
| `src/pages/AiApps.tsx` | 新建：页面、页头总开关、褪色状态 |
| `src/components/AiAppCard.tsx` | 新建：卡片 |
| `src/components/AiAppDialogs.tsx` | 新建：冲突框、断开框 |
| `src/components/Layout.tsx`、`src/App.tsx` | 修改：菜单和路由 |
| `src/pages/ProcessRouting.tsx`、`src/pages/ProcessRuleDialog.tsx`、`src/pages/SimpleConnection.tsx` | 修改：过滤 AI 规则 |
| `src/pages/Settings.tsx`、`src/lib/validation.ts` | 修改：设置项、错误文案 |

---

### Task 1: 分流服务：换代理目标、删除规则时也断开旧连接

**Files:**
- Modify: `src-tauri/procsvc/src/reconcile.rs`（`plan` 第 9 步约 184-191 行；文档注释约 62 行；测试 `reset_only_on_action_change`、`proxy_target_change_edits_config_without_reset` 约 766-810 行）
- Modify: `src-tauri/procsvc-proto/src/lib.rs:5`，以及约 305 行的断言

**Interfaces:**
- Produces: `plan(old, new)` 新增两种情况会产出 `Op::Reset`：
  1. 同 id、同 exe，`reset_on_change`，动作都是 `Proxy` 但目标不同；
  2. 旧规则 `reset_on_change`，新规则集里没有同 id 的规则。
- 每个 exe 最多一次 `Reset`。Reset 永远在最后。
- 新增常量值：`PROTO_VERSION = 6`。

- [ ] **Step 1: 改写两个现有测试，并新增测试**

```rust
#[test]
fn reset_on_action_or_target_change() {
    // 动作种类变化：照旧 Reset
    let ops = plan(&[r(Action::Direct, FaultPolicy::Direct)], &[r(Action::Proxy(target(1)), FaultPolicy::Direct)]);
    assert_eq!(count_resets(&ops), 1, "{ops:?}");
    assert!(matches!(ops.last(), Some(Op::Reset { .. })), "{ops:?}");
    // 换代理目标：现在也 Reset，且排在 SetProxy 之后
    let ops = plan(&[r(Action::Proxy(target(1)), FaultPolicy::Direct)], &[r(Action::Proxy(target(2)), FaultPolicy::Direct)]);
    assert_eq!(count_resets(&ops), 1, "{ops:?}");
    assert!(matches!(ops.last(), Some(Op::Reset { .. })), "{ops:?}");
    // reset_on_change=false：都不 Reset
    let ops = plan(
        &[spec("r", EXE, Action::Proxy(target(1)), FaultPolicy::Direct, false)],
        &[spec("r", EXE, Action::Proxy(target(2)), FaultPolicy::Direct, false)],
    );
    assert_eq!(count_resets(&ops), 0, "{ops:?}");
}

#[test]
fn removed_reset_rule_resets_its_exe() {
    let ops = plan(&[spec("r", EXE, Action::Proxy(target(1)), FaultPolicy::Direct, true)], &[]);
    assert_eq!(ops.last(), Some(&Op::Reset { exe: EXE.into() }), "{ops:?}");
    let ops = plan(&[spec("r", EXE, Action::Proxy(target(1)), FaultPolicy::Direct, false)], &[]);
    assert_eq!(count_resets(&ops), 0, "{ops:?}");
}

#[test]
fn reset_once_per_exe() {
    // 同一 exe 的规则换了 id（删 + 增）且动作变化：只 Reset 一次
    let ops = plan(
        &[spec("a", EXE, Action::Proxy(target(1)), FaultPolicy::Direct, true)],
        &[spec("b", EXE, Action::Proxy(target(2)), FaultPolicy::Direct, true)],
    );
    assert_eq!(count_resets(&ops), 1, "{ops:?}");
}
```

把 `proxy_target_change_edits_config_without_reset` 改名为 `proxy_target_change_edits_config_then_resets`。期望值改为：`[SetProxy{rule:"r", target: target(2)}, Reset{exe: EXE}]`。该测试原有的 `reset_on_change` 取值保持不变；如果原来是 `false`，就把 `spec` 的最后一个参数改成 `true`。

- [ ] **Step 2: 运行测试，确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc reconcile`
Expected: 上述新测试 FAIL（Reset 次数为 0）

- [ ] **Step 3: 改 `plan` 第 9 步**

触发 Reset 的条件改为三种：
- `prev(n)` 为 None 或动作种类不同；
- `prev(n)` 的 action 是 `Proxy` 且与 `n.action` 不相等；
- 旧规则 `o` 设置了 `reset_on_change`，且 `new_of(&o.id)` 为 None。

用 `exe_key` 去重。同时更新函数顶部的文档注释第 9 条。

第 324 行附近 `retry_resets` 的去重逻辑不用改。

- [ ] **Step 4: 提升协议版本**

把 `PROTO_VERSION` 改为 `6`，同步更新 `procsvc-proto/src/lib.rs` 测试里的 `assert_eq!(PROTO_VERSION, 6)`。

- [ ] **Step 5: 运行服务端全部测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc && cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc-proto`
Expected: 全部 PASS。`server.rs` 里断言 Reset 次数的旧测试如果失败，按新语义改期望值，不要改实现。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/procsvc src-tauri/procsvc-proto
git commit -m "feat(procsvc): reset connections on proxy target change and rule removal"
```

---

### Task 2: 模型标记 AI 规则；进程分流页和简洁版不显示、不能编辑 AI 规则

**Files:**
- Modify: `src-tauri/src/model/types.rs:385-432`（`ProcessRule` 及其 `Default`）
- Modify: `src-tauri/src/model/validate.rs`（新增错误变体，并在 `validate_process_routing` 中使用）
- Modify: `src-tauri/src/commands.rs:241-252`（`save_process_rule_seeded`）、`:266`（`delete_process_rule`）
- Modify: `src/pages/ProcessRouting.tsx:426`、`src/pages/ProcessRuleDialog.tsx:101`、`src/pages/SimpleConnection.tsx:138,512`、`src/lib/validation.ts`
- Test: `src-tauri/src/model/validate.rs`、`src-tauri/src/commands.rs` 的 tests 模块，以及 `src/pages/ProcessRouting.test.tsx`、`src/lib/validation.test.ts`

**Interfaces:**
- Produces:
  - `ProcessRule.ai_app: Option<String>`：`#[serde(default)]`，默认 `None`；TS 中为 `ai_app: string | null`。
  - `ValidationError::AiAppProcess { exe_path: String, app: String }`：`app` 是规则的 `name`，也就是应用显示名。
  - TS 侧新增 `isAiRule(r: ProcessRule): boolean`，放在 `src/lib/processRule.ts`。

- [ ] **Step 1: 写失败测试（Rust）**

```rust
// validate.rs tests
#[test]
fn duplicate_with_ai_rule_reports_ai_app() {
    let mut m = Model::default();
    m.process_routing.rules.push(ProcessRule { name: "ChatGPT".into(), ai_app: Some("chatgpt".into()),
        processes: vec![ProcessEntry::name("ChatGPT.exe")], ..Default::default() });
    m.process_routing.rules.push(ProcessRule { processes: vec![ProcessEntry::name("chatgpt.exe")], ..Default::default() });
    let errs = validate(&m);
    assert!(errs.contains(&ValidationError::AiAppProcess { exe_path: "chatgpt.exe".into(), app: "ChatGPT".into() }), "{errs:?}");
    assert!(!errs.iter().any(|e| matches!(e, ValidationError::DuplicateProcessRule { .. })));
}

// commands.rs tests
#[tokio::test]
async fn save_rule_cannot_touch_ai_rule() {
    // 先放一条 ai_app=Some 的规则；用同 id、ai_app=None 去 save，期望 Err，且模型不变
    // 新建规则时传入 ai_app=Some("x")，期望保存后 ai_app 仍为 None（字段被忽略）
}
```

使用 `validate.rs` 里已有的 `validate` 入口函数（按实际名称）。

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- validate commands::tests::save_rule_cannot_touch_ai_rule`
Expected: 编译失败（没有 `ai_app` 字段）

- [ ] **Step 3: 实现**

- `ProcessRule` 加字段 `ai_app`。
- `validate_process_routing` 里，`seen` 改为记录 `(kind, 小写值) → 第一次出现的规则`。重复时：如果两条规则中有一条带 `ai_app`，报 `AiAppProcess`，`app` 取那条 AI 规则的 `name`；否则照旧报 `DuplicateProcessRule`。
- `save_process_rule_seeded` 的处理：
  - 已存在的同 id 规则带 `ai_app` 时，返回 `Err(vec![ValidationError::InvalidName { name: rule.name, reason: "AI 应用的规则请在 AI 应用页修改".into() }])`；
  - 否则强制 `rule.ai_app = None`。
- `delete_process_rule` 只删除 `ai_app.is_none()` 的规则。

- [ ] **Step 4: 前端过滤，并补文案**

- `src/lib/processRule.ts` 新增 `export const isAiRule = (r: ProcessRule) => r.ai_app != null;`
- `ProcessRouting.tsx` 的 `shown` 先过滤 `!isAiRule(r)`。
- `ProcessRuleDialog.tsx:101` 的 `rules` 也过滤掉 AI 规则（用于前端查重和展示）。
- `SimpleConnection.tsx` 第 138 行和第 512 行的规则集合排除 AI 规则。
- `validation.ts` 给 `AiAppProcess` 返回 `${exe_path} 已由 AI 应用「${app}」管理，请在 AI 应用页操作`。

测试要求：
- `ProcessRouting.test.tsx`：model 中放一条 `ai_app: "chatgpt"` 的规则，断言列表里没有它的名字；
- `validation.test.ts`：断言上面那句文案。

在 bindings 重新生成之前，`ai_app` 字段先手动加到 `src/bindings.ts` 的 `ProcessRule` 类型上。Task 6 会整体重新生成。

- [ ] **Step 5: 运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib && pnpm test && pnpm typecheck`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/model src-tauri/src/commands.rs src
git commit -m "feat(model): tag AI app rules and keep them out of process routing UI"
```

---

### Task 3: 应用列表：类型、校验、内置兜底、镜像拉取和缓存

**Files:**
- Create: `presets/ai-apps.json`
- Create: `src-tauri/src/aiapps/mod.rs`、`src-tauri/src/aiapps/catalog.rs`、`src-tauri/src/aiapps/fetch.rs`
- Modify: `src-tauri/src/lib.rs`（加 `mod aiapps;`）

**Interfaces:**
- Produces（`catalog.rs`）：

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct Catalog { pub schema_version: u32, pub apps: Vec<CatalogApp> }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct CatalogApp {
    pub id: String,            // ^[a-z0-9-]{1,32}$，列表内唯一
    pub name: String,          // 非空
    pub icon: String,          // 以 "data:image/" 开头，长度 ≤ 64 KiB
    pub download_url: String,  // https://
    pub test_url: String,      // https://
    #[serde(default = "block")] pub on_fault: FaultPolicy, // JSON 中为 "Direct" / "Block"（沿用 FaultPolicy 的 serde 形式）
    pub install: Vec<InstallSource>, // 非空
    pub processes: Vec<String>,      // 非空；每项满足 routa_procsvc_proto::is_process_name，按小写去重
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InstallSource {
    Store { family: String, app_id: String },  // 都非空，不含 '!'、'\\'、'/'
    Exe { path: String },                      // 以 %LOCALAPPDATA% / %PROGRAMFILES% / %PROGRAMFILES(X86)% / %APPDATA% 之一开头，以 .exe 结尾，不含 ".."
}

pub fn parse_catalog(json: &str) -> Result<Catalog, String>;  // 任一校验失败 → Err，整份作废
pub fn bundled() -> Catalog;  // include_str!("../../../presets/ai-apps.json")，解析失败直接 panic（由测试兜底）
```

- Produces（`fetch.rs`）：

```rust
pub const CATALOG_URL: &str = "https://raw.githubusercontent.com/c0010c/Routa/main/presets/ai-apps.json";
pub fn sources() -> Vec<String>;  // [gh-proxy.org/<URL>, ghfast.top/<URL>, <URL>]，用 updater::download::MIRRORS 拼接
pub fn choose(remote: Option<&str>, cached: Option<&str>) -> (Catalog, Option<String>);
//   remote 合法 → (remote, Some(remote 原文，用于写缓存))；否则 cached 合法 → (cached, None)；否则 (bundled(), None)
pub async fn fetch_remote(client: &reqwest::Client) -> Option<String>;  // 依次尝试 sources()，每个 10 秒超时，HTTP 200 才返回
```

- Produces（`mod.rs`）：

```rust
pub struct AiApps { catalog: std::sync::RwLock<Catalog>, cache_path: PathBuf }
impl AiApps {
    pub fn new(cache_path: PathBuf) -> Self;   // 初始值 = choose(None, 读缓存文件)
    pub fn catalog(&self) -> Catalog;
    pub fn set(&self, c: Catalog);
    pub fn cache_path(&self) -> &Path;
}
```

- [ ] **Step 1: 写 `presets/ai-apps.json`**

三个应用的数据如下（已在开发机上核实 ChatGPT 和 Claude 的商店包）：

| id | name | install | processes | test_url | download_url |
|---|---|---|---|---|---|
| `chatgpt` | ChatGPT | `store` family `OpenAI.Codex_2p2nqsd0c76g0`，app_id `App` | `ChatGPT.exe`, `Codex.exe`, `codex-command-runner.exe` | `https://chatgpt.com` | `https://chatgpt.com/download` |
| `claude` | Claude | ① `store` family `Claude_pzs8sxrjxfjjc`，app_id `Claude`；② `exe` `%LOCALAPPDATA%\AnthropicClaude\claude.exe` | `claude.exe`, `cowork-svc.exe`, `claude-ssh-proxy.exe`, `claude-ssh-broker.exe` | `https://claude.ai` | `https://claude.ai/download` |
| `cursor` | Cursor | ① `exe` `%LOCALAPPDATA%\Programs\cursor\Cursor.exe`；② `exe` `%PROGRAMFILES%\cursor\Cursor.exe` | `Cursor.exe` | `https://cursor.com` | `https://cursor.com/download` |

- 三个应用的 `on_fault` 都是 `"Block"`。
- `icon` 用一个 48×48 的 SVG data URI：品牌色圆角方块加首字母（ChatGPT `#10a37f` "G"，Claude `#d97757` "C"，Cursor `#222222` "Cu"）。以后要换真实图标，只需改远程列表。
- 不收录 `node.exe`、`python.exe` 等通用进程名。它们会把系统里所有同名程序都拖进代理。

- [ ] **Step 2: 写失败测试（`catalog.rs`、`fetch.rs`）**

```rust
#[test] fn bundled_catalog_is_valid() { let c = bundled(); assert_eq!(c.schema_version, 1);
    assert_eq!(c.apps.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["chatgpt", "claude", "cursor"]); }
#[test] fn rejects_wrong_schema_version() { /* schema_version: 2 → Err */ }
#[test] fn rejects_non_data_icon() { /* icon: "https://x/y.png" → Err */ }
#[test] fn rejects_http_urls() { /* download_url / test_url 为 http:// → Err */ }
#[test] fn rejects_process_with_path() { /* processes: ["C:\\a\\b.exe"] → Err；["a"]（无 .exe）→ Err */ }
#[test] fn rejects_exe_outside_known_roots() { /* path: "C:\\Windows\\cmd.exe" → Err；"%LOCALAPPDATA%\\..\\x.exe" → Err */ }
#[test] fn rejects_duplicate_ids() {}
#[test] fn on_fault_defaults_to_block() {}
#[test] fn sources_try_mirrors_then_github() { assert_eq!(sources(), vec![
    format!("https://gh-proxy.org/{CATALOG_URL}"), format!("https://ghfast.top/{CATALOG_URL}"), CATALOG_URL.to_string()]); }
#[test] fn choose_prefers_valid_remote_then_cache_then_bundled() {
    // (Some(valid_a), Some(valid_b)) → a, Some(_)
    // (Some("garbage"), Some(valid_b)) → b, None
    // (Some("garbage"), Some("garbage")) → bundled(), None
}
```

- [ ] **Step 3: 运行，确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib aiapps`
Expected: 编译失败（模块不存在）

- [ ] **Step 4: 实现 `catalog.rs`、`fetch.rs`、`mod.rs`**

`fetch.rs` 再提供一个后台任务：

```rust
pub fn spawn_refresh(state: Arc<AiApps>, on_change: impl Fn(&Catalog) + Send + Sync + 'static);
```

- 立即执行一次 `fetch_remote`，之后每 24 小时执行一次。
- `choose` 的输入是这次的远程结果，加上 `cache_path` 中读到的缓存。
- 选出的列表与当前不同：先 `set`，再调用 `on_change`。
- 有新的远程原文时写入缓存文件；写失败只打日志。
- reqwest 客户端单独构建，不经过代理。

- [ ] **Step 5: 运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib aiapps`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add presets src-tauri/src/aiapps src-tauri/src/lib.rs
git commit -m "feat(aiapps): catalog types, validation, bundled list and mirrored fetch"
```

---

### Task 4: 规则变换（纯函数）：冲突、启用、停用、与列表同步

**Files:**
- Create: `src-tauri/src/aiapps/rules.rs`

**Interfaces:**
- Consumes: `CatalogApp`（Task 3），`ProcessRule.ai_app`（Task 2）
- Produces:

```rust
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct AiAppConflict { pub rule_id: ProcessRuleId, pub rule_name: String, pub process: String, pub removes_rule: bool }

pub fn ai_rule<'a>(pr: &'a ProcessRouting, app_id: &str) -> Option<&'a ProcessRule>;
pub fn conflicts(pr: &ProcessRouting, app: &CatalogApp) -> Vec<AiAppConflict>;
/// 移除冲突条目（规则变空就删除），插入或更新 AI 规则（node、processes、on_fault、name、
/// reset_on_change=true、user_enabled=true），并打开总开关。返回被整条删除的规则 id。
pub fn enable(pr: &mut ProcessRouting, app: &CatalogApp, node: NodeRef) -> Vec<ProcessRuleId>;
pub fn disable(pr: &mut ProcessRouting, app_id: &str);  // user_enabled=false，规则和节点保留
/// 让现有 AI 规则跟上列表：更新 name/processes/on_fault；删除列表里已经没有的应用的规则。返回是否有改动。
pub fn sync_with_catalog(pr: &mut ProcessRouting, catalog: &Catalog) -> bool;
```

- 冲突的判定：只看 `ai_app.is_none()` 的规则。条目满足以下任一条件即算冲突（都不区分大小写）：
  - `kind == Name` 且值等于应用的某个进程名；
  - `kind == Path` 且文件名（`validate::process_file_name`）等于应用的某个进程名。
- `removes_rule`：这条规则的所有条目都冲突。
- `rule_name` 用 `validate::process_rule_label`。

- [ ] **Step 1: 写失败测试**

```rust
#[test] fn conflict_matches_name_case_insensitively() {}
#[test] fn conflict_matches_path_file_name() { /* Path "C:\\X\\chatgpt.EXE" 与 "ChatGPT.exe" 冲突 */ }
#[test] fn conflict_ignores_other_ai_rules() {}
#[test] fn enable_removes_only_conflicting_entries() {
    // 规则 {ChatGPT.exe, notepad.exe} → 只剩 notepad.exe；规则 {Codex.exe} → 整条删除，返回其 id
}
#[test] fn enable_creates_tagged_rule_and_turns_routing_on() {
    // ai_app=Some("chatgpt")，processes 全为 Name，reset_on_change，on_fault 取自 app，pr.enabled == true
}
#[test] fn enable_again_updates_node_without_duplicating() {}
#[test] fn disable_keeps_rule_and_node() {}
#[test] fn sync_updates_processes_and_on_fault() {}
#[test] fn sync_removes_rules_of_apps_gone_from_catalog() {}
#[test] fn sync_reports_no_change_when_equal() {}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib aiapps::rules`
Expected: FAIL（函数不存在）

- [ ] **Step 3: 实现 `rules.rs`**

- [ ] **Step 4: 运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib aiapps::rules`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/aiapps/rules.rs src-tauri/src/aiapps/mod.rs
git commit -m "feat(aiapps): conflict detection and AI rule transforms"
```

---

### Task 5: 本机检测、启动、置前

**Files:**
- Create: `src-tauri/src/aiapps/detect.rs`、`src-tauri/src/aiapps/launch.rs`
- Modify: `src-tauri/Cargo.toml:54`（`windows` features 增加 `"Management_Deployment"`、`"Foundation_Collections"`）

**Interfaces:**
- Produces:

```rust
// detect.rs
#[derive(Clone, Debug, PartialEq)]
pub enum LaunchTarget { Aumid(String), Exe(PathBuf) }   // Aumid = "<family>!<app_id>"
/// 按 install 顺序取第一个命中的来源。纯函数：商店包是否存在、文件是否存在、环境变量都由参数提供。
pub fn resolve(sources: &[InstallSource], has_package: impl Fn(&str) -> bool,
               exists: impl Fn(&Path) -> bool, env: impl Fn(&str) -> Option<String>) -> Option<LaunchTarget>;
pub fn detect(app: &CatalogApp) -> Option<LaunchTarget>;  // Windows：PackageManager::FindPackagesByUserSecurityIdPackageFamilyName(空 SID, family) 非空即存在
pub fn running(app: &CatalogApp, procs: &[RunningProcess]) -> bool;  // 任一进程名（不区分大小写）在列表中

// launch.rs
pub fn launch(t: &LaunchTarget) -> Result<(), String>;
//   Exe   → Command::new("explorer.exe").arg(path)
//   Aumid → Command::new("explorer.exe").arg(format!("shell:AppsFolder\\{aumid}"))
//   两种都让程序以普通用户权限运行
pub fn focus_or_launch(app: &CatalogApp, t: &LaunchTarget) -> Result<LaunchOutcome, String>;
#[derive(Serialize, Clone, Copy, Debug, PartialEq, specta::Type)]
pub enum LaunchOutcome { Launched, Focused }
/// 纯逻辑：有可见窗口 → Focused；没有 → 调用 launch → Launched。
pub(crate) fn decide_focus(found_window: bool) -> LaunchOutcome;
```

- 置前的实现：先用 `running_processes()` 取出名字匹配的 pid 集合。再 `EnumWindows`，找第一个满足以下条件的窗口：`IsWindowVisible`、`GetWindow(GW_OWNER)` 为空、`GetWindowThreadProcessId` 的 pid 在集合中。窗口最小化（`IsIconic`）时先 `ShowWindow(SW_RESTORE)`，然后 `SetForegroundWindow`。
- 用到的 `windows-sys` features 已经齐全（`Win32_UI_WindowsAndMessaging`）。

- [ ] **Step 1: 写失败测试**

```rust
#[test] fn resolve_prefers_first_matching_source() { /* claude：store 存在 → Aumid("Claude_pzs8sxrjxfjjc!Claude") */ }
#[test] fn resolve_falls_back_to_exe() { /* store 不存在、exe 存在 → Exe(展开后的路径) */ }
#[test] fn resolve_expands_only_known_roots() { /* env("LOCALAPPDATA")=Some("C:\\U\\AppData\\Local") → "C:\\U\\AppData\\Local\\Programs\\cursor\\Cursor.exe" */ }
#[test] fn resolve_none_when_nothing_found() {}
#[test] fn running_matches_any_process_case_insensitively() {}
#[test] fn focus_falls_back_to_launch() { assert_eq!(decide_focus(false), LaunchOutcome::Launched); assert_eq!(decide_focus(true), LaunchOutcome::Focused); }
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib aiapps::detect aiapps::launch`
Expected: FAIL

- [ ] **Step 3: 实现**

非 Windows 下：`detect` 返回 None，`launch` / `focus_or_launch` 返回 `Err("仅支持 Windows")`。

- [ ] **Step 4: 运行测试，并在本机检查**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib aiapps`
Expected: PASS

再在本机临时写一个 `#[ignore]` 测试 `detect_real_machine`：对 `bundled()` 的 chatgpt、claude 调用 `detect`，打印结果。用 `cargo test ... -- --ignored --nocapture` 运行。期望两者都是 `Some(Aumid(..))`。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/aiapps src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(aiapps): detect installed apps, launch unelevated, focus running window"
```

---

### Task 6: 编排与命令：启用、停用、准备、列表视图、按应用测延迟、启动时自动起内核

**Files:**
- Modify: `src-tauri/src/desktop.rs`（`UiPreferences` 加字段；`impl Desktop` 加方法；新增 `#[tauri::command]`）
- Create: `src-tauri/src/aiapps/commands.rs`
- Modify: `src-tauri/src/runtime/mod.rs`（约 1050 行附近加 `test_delay_at`）
- Modify: `src-tauri/src/lib.rs`（`specta_builder` 注册命令；`.manage(Arc<AiApps>)`；setup 中调用 `spawn_refresh`；约 299 行之后加启动时自动起内核）
- Regenerate: `src/bindings.ts`

**Interfaces:**
- Consumes: Task 3–5 的全部接口；现有的 `confirm_or_revert`、`reload_ok`、`procroute_running`、`Runtime::restart_core`、`procroute::service_command(ElevatedCmd::Install)`、`procroute::client::retry()`、`commands::ops`。
- Produces（前端会用到的命令，TS 名称是 camelCase）：

```rust
// UiPreferences
pub skip_ai_reset_prompt: bool,   // 默认 false

// aiapps/commands.rs
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct AiAppView {
    pub id: String, pub name: String, pub icon: String, pub download_url: String,
    pub installed: bool, pub running: bool,
    pub rule_id: Option<ProcessRuleId>, pub enabled: bool, pub node: Option<NodeRef>,
}
#[tauri::command] pub async fn ai_apps_list(apps: State<Arc<AiApps>>, rt: Rt) -> Vec<AiAppView>;
#[tauri::command] pub fn ai_app_conflicts(apps, rt, app_id: String) -> Result<Vec<AiAppConflict>, String>;
#[tauri::command] pub async fn test_ai_app_delay(apps, rt, app_id: String, proxy_name: String) -> Result<DelayResult, String>;
#[tauri::command] pub async fn launch_ai_app(apps, dt, rt, app_id: String) -> Result<LaunchOutcome, String>;
#[tauri::command] pub fn open_ai_app_download(apps, app_id: String) -> Result<(), String>;  // 用 explorer.exe 打开列表里的 download_url（同 commands.rs:613 open_project_link 的做法），不接受前端传入的 URL

// desktop.rs
impl Desktop {
    /// 确保内核在运行、组件为 Running，再执行 rules::enable，并等服务确认；失败时整体回滚 ProcessRouting。
    pub async fn enable_ai_app(&self, rt: &Runtime, app: &CatalogApp, node: NodeRef) -> Result<(), String>;
    /// rules::disable，并等服务确认；失败时回滚。
    pub async fn disable_ai_app(&self, rt: &Runtime, app_id: &str) -> Result<(), String>;
    /// "启动"前的准备：内核、组件、总开关；AI 规则必须已启用，否则返回 Err("请先选择节点")。
    pub async fn prepare_ai_app(&self, rt: &Runtime, app_id: &str) -> Result<(), String>;
}
#[tauri::command] pub async fn enable_ai_app(apps, dt, rt, app_id: String, node: NodeRef) -> Result<(), String>;
#[tauri::command] pub async fn disable_ai_app(dt, rt, app_id: String) -> Result<(), String>;
// 换节点直接复用现有的 change_process_node(rule_id, node)

// runtime/mod.rs
pub async fn test_delay_at(&self, name: String, url: String) -> DelayResult;  // 不写入 DelayBook
```

**行为要点：**

- **确保内核：** `rt.status().core` 是 `Stopped` 或 `Crashed` 时，调用 `rt.restart_core().await`，再最多等 10 秒变成 `Running`，否则返回 `Err("内核未能启动，请查看日志")`。
- **确保组件：** 按 `procroute::client::status().component` 处理：
  - `NotInstalled` 或 `NeedsUpdate` → `service_command(ElevatedCmd::Install)`。用户拒绝 UAC 时，原样返回它的错误。
  - `Stopped` → `client::retry()`。
  - `OwnedByOtherUser` 或 `Error` → 返回对应文案。
  - 之后最多等 10 秒变成 `Running`。
- **回滚：** 修改前先 `let before = rt.model().process_routing.clone()`。回滚闭包为 `|pr| if *pr == after { *pr = before.clone() }`，其中 `after` 是修改后的快照。
- **简洁版遗留 id：** `enable` 返回被删除的规则 id，从 `UiPreferences.managed_rules` 和 `selected_apps` 中移除这些 id，然后保存偏好。
- **确认生效：** 只有 `procroute_running()` 时才需要确认。
- **`launch_ai_app` 的顺序：**
  1. `detect`，结果为 None 时返回 `Err("未安装")`；
  2. `prepare_ai_app`；
  3. `focus_or_launch`。
- **列表同步：** `spawn_refresh` 的 `on_change` 和应用启动各执行一次。先用模型副本试跑 `rules::sync_with_catalog`，有改动时才调用 `rt.mutate_process_routing`。
- **启动时自动起内核：** 在 `lib.rs` 现有的 `auto_connect` 分支之后加判断：`process_routing.enabled`，且存在 `ai_app.is_some() && user_enabled` 的规则，且内核是 `Stopped`，就调用 `rt.restart_core().await`。`auto_connect` 偏好不影响这个判断。
- **`test_ai_app_delay`：** `url` 取该应用的 `test_url`，不接受前端传入的 URL。

- [ ] **Step 1: 写失败测试（`desktop.rs` 的 tests 模块，沿用现有的 `runtime(td)` 测试构造方式）**

把回滚逻辑抽成纯函数 `pub(crate) fn revert_to(before: ProcessRouting, after: ProcessRouting) -> impl FnOnce(&mut ProcessRouting)`，`enable_ai_app` 和 `disable_ai_app` 都使用它。

```rust
#[test] fn enable_reverts_whole_routing_on_failure() {
    // before：手动规则 {ChatGPT.exe, notepad.exe}；after = before 上执行 rules::enable 的结果
    // pr == after 时调用 revert_to(before, after) → pr == before（被移除的 ChatGPT.exe 回来了，AI 规则消失）
    // pr 已被别处改动（≠ after）→ 不动
}
#[tokio::test] async fn enable_fails_without_core_and_leaves_routing_unchanged() {
    // 测试 runtime 用 NoLauncher，内核起不来 → Err("内核未能启动，请查看日志")，process_routing 与之前相等
}
#[test] fn prune_simple_ids_removes_deleted_rules() {}  // 抽出纯函数 prune_simple_ids(prefs: &mut UiPreferences, deleted: &[ProcessRuleId])
#[test] fn startup_core_needed_only_for_enabled_ai_rules() {
    // 抽出纯函数 pub(crate) fn ai_needs_core(model: &Model) -> bool，并测试：总开关关 / 规则停用 / 非 AI 规则 → false
}
```

`aiapps/commands.rs` 中的测试：

```rust
#[test] fn view_marks_enabled_only_when_rule_enabled() {}  // 抽出纯函数 build_views(catalog, model, detect, running) 来测
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- aiapps desktop::tests`
Expected: FAIL

- [ ] **Step 3: 实现，并注册命令和状态**

- [ ] **Step 4: 重新生成 bindings，跑全部检查**

Run: `cargo run --manifest-path src-tauri/Cargo.toml -- --export-bindings && cargo test --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings && pnpm typecheck && pnpm test`
Expected: 全部 PASS。`src/bindings.ts` 中出现 `aiAppsList`、`enableAiApp`、`disableAiApp`、`launchAiApp`、`aiAppConflicts`、`testAiAppDelay`。

- [ ] **Step 5: Commit**

```bash
git add src-tauri src/bindings.ts
git commit -m "feat(aiapps): enable/disable/launch orchestration, commands and startup core"
```

---

### Task 7: 前端：菜单、页面、卡片、褪色状态、延迟

**Files:**
- Create: `src/api/aiApps.ts`、`src/pages/AiApps.tsx`、`src/components/AiAppCard.tsx`
- Modify: `src/components/Layout.tsx:27-34`、`src/App.tsx:96-101`
- Test: `src/pages/AiApps.test.tsx`

**Interfaces:**
- Consumes: Task 6 生成的 `commands.*`；现有的 `useSaveProcessRoutingSettings`、`desktopApi.changeNode`、`NodePickerDialog`、`DelayText`、`ruleExit`（`src/lib/hidden`）、`useModel` / `useStatus`（`src/api/client`）
- Produces:

```ts
// src/api/aiApps.ts
export function useAiApps(): UseQueryResult<AiAppView[]>;  // queryKey ["aiApps"]，refetchInterval 3000
export const useEnableAiApp: (app_id, node) => …;  // make(() => commands.enableAiApp)，成功后 invalidate ["aiApps"] 与 model
export const useDisableAiApp; export const useLaunchAiApp; export const useAiAppConflicts;
export function useAiAppDelay(appId: string, proxyName: string | undefined): { result?: DelayResult; measure(): void; pending: boolean };
//   queryKey ["aiDelay", appId, proxyName]，enabled: !!proxyName，staleTime 60_000

// src/components/AiAppCard.tsx
export function AiAppCard(props: { app: AiAppView; model: Model; paused: boolean;
  onToggle(on: boolean): void; onLaunch(): void; onPickNode(node: NodeRef): void }): JSX.Element;
```

**页面和卡片要点：**

- **菜单：** 在 `pages` 数组的"进程分流"之前插入 `{ path: "/ai-apps", label: "AI 应用", icon: <SmartToyOutlined /> }`；完整版路由加 `<Route path="/ai-apps" element={<AiAppsPage />} />`。
- **页头：** 标题"AI 应用"，右侧放总开关（`Switch`，值为 `model.process_routing.enabled`，切换时调用 `useSaveProcessRoutingSettings`）。总开关为关时，开关外圈加一层淡蓝色光晕：`boxShadow: 0 0 0 3px alpha(primary, .25)`。
- **卡片网格：** 用 CSS grid，`repeat(auto-fill, minmax(180px, 1fr))`。
- **未安装的卡片：** `opacity: .45`，只显示图标、名称和"未安装 · 下载"链接。链接调用 `commands.openAiAppDownload(app.id)`。
- **已安装的卡片：**
  - 图标为 `<img src={app.icon}>`，48px。
  - 节点按钮：已选节点时显示 `nodeLabel`；未选时显示"选择节点"。
  - 延迟：`DelayText`，点击后重测。`proxyName = ruleExit(model, rule)?.name`，其中 `rule = model.process_routing.rules.find(r => r.id === app.rule_id)`；没有规则或节点时不显示延迟。
  - 开关：`Switch`，值为 `app.enabled`。
  - 主按钮：`app.running` 时为 outlined 的"运行中"，否则为 contained 的"启动"。
  - 小圆点：读取 `useProcRouteStatus().data.actions[app.rule_id]`（`RuleView { action, reason }`）。`enabled && !paused && action === "Proxy"` 时为绿色；`enabled && !paused` 且有 `reason` 时为红色，悬停显示 `reason`；其余情况为灰色。
- **`paused`（即 `!process_routing.enabled`）时：**
  - 图标、节点、延迟加上 `filter: grayscale(1); opacity: .55`；
  - 开着的开关用 `color="default"` 显示浅色，外层套 `Tooltip title="进程分流已关闭，开启后恢复"`；
  - "启动"按钮保持可用；
  - 页面上不出现任何横幅或提示文字。

- [ ] **Step 1: 写失败测试（`AiApps.test.tsx`，mock 方式同 `ProcessRouting.test.tsx`）**

```ts
it("在进程分流上方显示 AI 应用菜单", …)              // Layout 渲染后，"AI 应用"排在"进程分流"之前
it("未安装的卡片只显示下载链接", …)                  // 断言出现 "未安装 · 下载"，没有 role=switch
it("运行中的应用按钮为 运行中", …)
it("总开关关闭时开着的开关有暂停提示且无横幅文字", …)  // hover 后出现 "进程分流已关闭，开启后恢复"；没有 "进程分流已关闭，以下应用"
it("页头总开关切换调用 saveProcessRoutingSettings", …)
```

- [ ] **Step 2: 运行，确认失败**

Run: `pnpm vitest run src/pages/AiApps.test.tsx`
Expected: FAIL

- [ ] **Step 3: 实现 `aiApps.ts`、`AiAppCard.tsx`、`AiApps.tsx`、菜单和路由**

`onToggle`、`onLaunch`、`onPickNode` 在本任务里先直接调用 mutation（没有对话框），Task 8 再接入对话框流程。

- [ ] **Step 4: 运行测试**

Run: `pnpm test && pnpm typecheck`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat(ui): AI apps page with launcher cards and paused state"
```

---

### Task 8: 前端：首次选节点、冲突框、断开框、设置项

**Files:**
- Create: `src/components/AiAppDialogs.tsx`
- Modify: `src/pages/AiApps.tsx`、`src/pages/Settings.tsx`（"进程分流"标签页，约 289 行附近）
- Test: `src/pages/AiApps.test.tsx`（追加）、`src/pages/Settings.test.tsx`（追加）

**Interfaces:**
- Consumes: `commands.aiAppConflicts`、`enableAiApp`、`disableAiApp`、`launchAiApp`、`desktopApi.changeNode`；偏好 `skip_ai_reset_prompt`（通过 `desktopApi.preferences` 保存）
- Produces:

```ts
export function ConflictDialog(props: { conflicts: AiAppConflict[]; onCancel(): void; onConfirm(): void }): JSX.Element;
export function ResetDialog(props: { appName: string; onCancel(): void; onConfirm(dontAskAgain: boolean): void }): JSX.Element;
/** 页面内的流程函数：按顺序执行"选节点 → 冲突确认 → 断开确认 → 调用后端"；任一步取消则什么都不做。 */
function useAiAppFlow(): { toggle(app, on): Promise<void>; launch(app): Promise<void>; changeNode(app, node): Promise<void>; dialogs: JSX.Element };
```

**流程（全部在 `useAiAppFlow` 中实现）：**

- **开启（开关拨到开，或"启动"时规则还没启用）：**
  1. 没有节点时，先打开 `NodePickerDialog`；用户取消就结束。
  2. 调用 `aiAppConflicts`，结果非空时显示 `ConflictDialog`。
  3. `app.running && !prefs.skip_ai_reset_prompt` 时显示 `ResetDialog`。
  4. 调用 `enableAiApp(app.id, node)`。
- **"启动"：** 规则已启用时，直接调用 `launchAiApp`。规则未启用时，先走完上面的开启流程，成功后再调用 `launchAiApp`。
- **关闭开关：** `running && !skip` 时显示 `ResetDialog`，然后调用 `disableAiApp`。
- **换节点：** `enabled && running && !skip` 时显示 `ResetDialog`，然后调用 `desktopApi.changeNode(app.rule_id, node)`。规则还没启用时只记录节点：调用 `changeNode` 即可，规则会保持停用状态。规则不存在时，只在本地记住选中的节点，等用户开启时使用。
- **"下次不再提示"：** `ResetDialog` 确认时，如果勾选了这一项，就把偏好 `skip_ai_reset_prompt` 设为 `true` 并保存。
- **设置项：** 设置页"进程分流"标签加一个开关"AI 应用断开连接前提示"。开关的值为 `!skip_ai_reset_prompt`，切换时写回该偏好。
- **出错提示：** 错误文案通过页面现有的通用错误提示显示（同 `ProcessNodeSelect` 的 `errorMessage`）。

- [ ] **Step 1: 写失败测试**

```ts
it("首次开启先选节点，取消则不调用 enableAiApp", …)
it("有冲突时显示冲突框，文案含 规则「工作软件」里的 ChatGPT.exe 也在分流。", …)
it("整条规则会被删除时显示 开启后会删除该规则。", …)
it("应用运行中开启时显示断开框；勾选下次不再提示后保存 skip_ai_reset_prompt=true", …)
it("skip_ai_reset_prompt=true 时不显示断开框", …)
it("关闭开关和换节点在运行中也弹断开框", …)
it("启动：未启用时先走开启流程再调用 launchAiApp", …)
// Settings.test.tsx
it("AI 应用断开连接前提示 开关写回 skip_ai_reset_prompt", …)
```

- [ ] **Step 2: 运行，确认失败**

Run: `pnpm vitest run src/pages/AiApps.test.tsx src/pages/Settings.test.tsx`
Expected: FAIL

- [ ] **Step 3: 实现**

- [ ] **Step 4: 运行测试**

Run: `pnpm test && pnpm typecheck`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat(ui): AI app node pick, conflict and reset confirmations"
```

---

### Task 9: 真机验收

**Files:** 无代码改动。发现问题时回到对应任务修复。

- [ ] **Step 1: 启动开发版**

Run: `pnpm tauri dev`

- [ ] **Step 2: 逐项验收（开发机已装 ChatGPT 商店版和 Claude 商店版；没装 Cursor）**

1. **菜单和卡片：** 左侧"AI 应用"在"进程分流"上面。ChatGPT、Claude 卡片正常显示；Cursor 卡片置灰，显示"未安装 · 下载"，点击后用浏览器打开 `https://cursor.com/download`。
2. **首次启动：** 内核停止、ChatGPT 未运行时点 ChatGPT 的"启动"，依次出现：选节点 → 内核启动 → ChatGPT 打开。之后在进程分流页的连接列表里，能看到 `ChatGPT.exe` 的连接走所选节点。
3. **运行中置前：** 把 ChatGPT 窗口最小化，点"运行中"，窗口恢复到最前面。
4. **断开框：** ChatGPT 运行中时换节点，弹出断开框。勾选"下次不再提示"后继续，再换一次节点，不再弹框。设置页里恢复提示开关。
5. **冲突：** 在进程分流页新建一条规则 `{ChatGPT.exe, notepad.exe}`，然后在 AI 应用页开启 ChatGPT。冲突框出现；确认后，那条规则只剩 `notepad.exe`。
6. **反向冲突：** 在进程分流页新建一条 `Claude.exe` 规则（此时 Claude 已在 AI 页开过一次），报错 `claude.exe 已由 AI 应用「Claude」管理，请在 AI 应用页操作`。
7. **总开关褪色：** 关掉页头总开关，卡片褪色，开着的开关变浅色，悬停出现提示，没有横幅；重新打开后，之前开着的应用恢复。
8. **重启自动起内核：** 退出 Routa 再打开，有开着的 AI 应用时，内核自动启动。
9. **权限：** 以管理员身份运行 Routa，再启动 Claude。在任务管理器"详细信息"里，Claude 的"提升的权限"列显示"否"。
10. **服务升级：** 已安装旧版服务（协议 5）的机器启动新版后，组件状态显示需要更新，重装一次后正常。

- [ ] **Step 3: 记录结果**

把各项结果（通过、失败、修复所在的 commit）写进 PR 描述。
