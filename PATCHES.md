# 补丁清单（实时）

本文件只描述 **当前 `custom` 分支相对上游基线的补丁**，以 HEAD 为准。
补丁被移除或合并进上游后，对应条目直接删除，不保留历史记录（历史见
`git log main..custom`）。

- 基线：`v0.10.3-beta.5`（上游 `9cf0d47e2`；tag 提交为 `5543b66e5`，tag-only）
- 补丁数：**37**
- 维护流程见 [MAINTAINING.md](MAINTAINING.md)

表格以**提交标题**为主键、hash 只在括号里作参考：rebase 会重写 hash，
按标题找永远能定位到当前提交，按 hash 找会在同步后失效。

| # | 提交 | 类型 | 摘要 |
| --- | --- | --- | --- |
| 1 | `02941599f` | fix | 顶栏始终显示会话工作目录 |
| 2 | `d8812d2cc` | fix | 内核忙时等待更久（5s → 30s） |
| 3 | `3d3d62df5` | fix | 从 agent 主题目录解析自定义主题 |
| 4 | `5b934a67f` | fix | 分离进程不再弹出控制台窗口 |
| 5 | `b432b9c82` | feat | 每个被接纳的 turn 前运行 turnHook |
| 6 | `8af0afa8a` | feat | 运行结束后通过 stopHook 写回 |
| 7 | `e78e1a927` | fix | 在运行结束前同步 spawn stopHook |
| 8 | `5a730ac9d` | feat | 用 updateGuard 拦截自更新 |
| 9 | `c9db67a78` | fix | Windows 剪贴板走 UTF-8 控制台 |
| 10 | `2c1c12a89` | fix | 所有多行粘贴折叠为标记 |
| 11 | `ed58d68fe` | feat | 新增 clean-kernel-state 命令 |
| 12 | `ce3bbb7c3` | fix | 空回复自动重试 + 流空闲超时 |
| 13 | `4d0434a99` | fix | 停滞的子代理重新派活 + 重试提示按回合隔离 |
| 14 | `f45a7d32b` | fix | 停滞通知只发给父代理一次 |
| 15 | `5fc6b6f4e` | fix | 收养无 bootstrap 记录的 kernel venv |
| 16 | `beae8d01e` | fix | 空回复重试前固定等 3 秒 |
| 17 | `cd4dbf0e5` | fix | 托盘显示 provider 与配置的模型名 |
| 18 | `e053de177` | fix | 推理档位一路带到线上 |
| 19 | `577e1bc78` | fix | `/speed` 开关持久化 |
| 20 | `09244c28b` | feat | 新增 session clean 清理空会话 |
| 21 | `1f7de0f08` | fix | refine 的 passthrough 头改为合并而非整表覆盖 |
| 22 | `7b3cd948b` | fix | 多行粘贴在写边界被拆开时仍按粘贴处理 |
| 23 | `61c6e2765` | feat | agents view 的 Model 列显示 provider / model |
| 24 | `5f2bcd46d` | fix | 用户消息回显改为纯文本，不再吞字符 |
| 25 | `d1b357e93` | fix | 清掉 fork 的 CI 门（fmt / clippy / 乱码注释） |
| 26 | `7718719b3` | fix | 单独的 `>` 保持字面显示（APPEND 续跑命令） |
| 27 | `3b0008f62` | fix | RPC 的 lease 探测移出 tokio 运行时 |
| 28 | `7591913b8` | fix | RPC 会话接入心跳与阈值自动压缩 |
| 29 | `0e7a77c18` | fix | daemon 把 provider 头贴到 `Model` 上给直连接缝用 |
| 30 | `08a509ec3` | fix | refine 请求保持机器可解析，失败时透出 provider 原话 |
| 31 | `2a79ed82e` | fix | refine 的空回复在解析错误里点名 |
| 32 | `11a542736` | fix | provider 报错不再被当成空回复重试；refine 不再压低输出预算 |
| 33 | `b1d04d02a` | fix | Windows 括号粘贴从按键流重组为一次粘贴 |
| 34 | `a829c0881` | feat | ACP：Python cell 按代码首行命名，并上报上下文用量 |
| 35 | `b1021aa09` | feat | TUI：`PASTE_TRACE` 打印粘贴路径上的原始终端事件 |
| 36 | `b164aae2b` | feat | 按受众（人 / agent）限定可见模型与推理档位：`visibleModels`、`thinkingLevels` |
| 37 | `ecc278f82` | fix | `/model` picker 接上 `visibleModels.human`（收尾见 `5f1a9eb75`） |

补丁之外还有三类提交不进表格：文档提交、版本戳提交，以及 2 个 rebase 修复提交
（`01bfb8634`、`58af8100b`，改 `pa-tui` 的测试调用点，见下文本次 rebase 的三处
源码改动）。文档与版本戳提交随 rebase 改写 hash，按标题找
（`git log --oneline v0.10.3-beta.5..HEAD` 里标题以 `docs:` / `chore: stamp`
开头的那些）；修复提交是重放破损的修补，不是本地补丁。

表格按**行为补丁**计数，所以行数与提交数**不相等**：同一个补丁的后续收尾提交
（对已入表补丁的修正、重构、补测试）并入该行的摘要，不另开一行——`5f1a9eb75`
就是这样并进第 37 行的。想核对总数时用 `git log --oneline v0.10.3-beta.5..HEAD`
与表格、`docs:`、`chore: stamp`、rebase 修复逐一对照，别指望一个固定的算术式：
任何一侧新增提交都会让它过期，所以这里不写死数字。

**`custom.30` 的版本行随 rebase 变了，提交信息没变。** 那个 stamp 提交写于
`v0.10.2` 基线，正文说「stamp as 0.10.2+custom.30」；rebase 把它的 diff 重放到
`v0.10.3-beta.5` 上之后，实际写入的是 `0.10.3-beta.5+custom.30`。于是
`custom.30` 同时指两个东西：安装目录里那份 `0.10.2+custom.30` 的构建，和本次
rebase 之后的新树。**本次同步因此把新树升到 `custom.31`**，`custom.30` 留给
已安装的那份。那个提交正文不追改（改写历史只为一个字符串不值当）。

**本次 rebase（`v0.10.2` → `v0.10.3-beta.5`）只改了三处源码**，都是重放后的
编译或语义破损，不含新补丁：

1. `crates/pa-tui/src/tool_card/ipython/tests.rs`：上游 `46e823d77` 给 `render`
   加了 `level: Detail` 参数，漏改 5 个测试调用点。**上游自己编不过**——在
   干净的上游 worktree 上 `cargo test -p pa-tui --lib` 复现 5× `E0061`、
   `EXIT=101`。本仓补上实参（`Detail::Overview` 对 `Detail::Overview`，与同文件
   既有 `All, All` 一致）。
2. `crates/pa-tui/src/agents_view_state.rs`：上游 `69a826f98` 把 `build_rows` 的
   activity 参数从 `&HashMap` 换成 `&ActivityCounts`，补丁 23 新增的测试仍在传
   `HashMap`。改为 `ActivityCounts::default()`。
3. `crates/pa-tui/src/input.rs`：`a_round_after_the_window_is_not_joined` 用
   `windows_paste("\rx")` 当第二轮，而补丁 22 的「含换行即粘贴」规则会把
   `"\rx"` 判成粘贴。夹具改为 `"yz"`（无换行），断言仍然只钉合并器。

第 1 条会在上游修好时再冲突一次；届时删掉本地这 5 处改动即可。

**版本号再跳过 `custom.31`**（2026-10-31 补记）：`custom.31` 的 stamp 提交
（`28d5ce9a3`）只改 `Cargo.toml` 与 `Cargo.lock`，它标记的是「rebase 到
`v0.10.3-beta.5` 之后、但**还没写**那两个新设置」的那棵树。按受众限制可见模型与
推理档位的 `visibleModels`、`thinkingLevels` 是在 stamp **之后**才写的，同一个提交
`b164aae2b` 一起引入（`git log --reverse -S'visibleModels' -- crates` 与
`git log --reverse -S'thinkingLevels' -- crates` 的首个命中都是它；`git grep` 在
`28d5ce9a3` 与 `v0.10.3-beta.5` 上对两个键都是 0 命中——注意 `-S` 正向列会列出
后续所有改动过该键的提交，只有 `--reverse` 的首行才是「引入」）。该提交
**当时并没有装进安装目录**：安装目录里那份 `custom.31` 构建于 2026-10-31 01:39
（SHA256 `3C83295C…2968`），它对应的源码树里两个键都不存在。

旧 exe（备份 `prime-agent.exe.bak-0.10.3-beta.5+custom.31`）里能搜到
`thinkingLevels` 字样，但那是**假命中**，而且只有一处（文件偏移 49862776）：
文件条目（`FileEntry`）的 serde 字段名表里，`thinking_level_change` 的
`thinkingLevel` 与紧邻的 `service_tier_change` 的 `serviceTier` 前后排在一起，
而 `thinkingLevel` 后面那个 `s` 正好是 `serviceTier` 的首字母——
`thinkingLevel` + `s` 拼出 `thinkingLevels`。两个键来自
`crates/pa-types/src/session.rs` 里两个**相邻的 struct**
（`ThinkingLevelChangeEntry` / `ServiceTierChangeEntry`），不是同一结构的字段。
它不是设置键，也不是内嵌 runtime/skills 负载。

作为对照，新装的 `custom.32` 有 7 处命中：4 处是同一个 `thinkingLevel`+`serviceTier`
拼串（分布在 4 个不同的元数据块里，其中 `repoUrlcommitbranchthinkingLevelserviceTier`
那一处来自另一张表），3 处是真正的设置键——`"thinkingLevels" allows only …` 的
clamp 文案，以及 `allowedModels visibleModels thinkingLevels thinkingBudgets …` 和
`… visibleModels thinkingLevels treeFilterMode …` 两张字段表。

新构建含这两个设置，若继续叫 `custom.31`，`--version`、`prime-agent.exe.bak-*`
与回滚判断就会指向两份不同的二进制——备份 `prime-agent.exe.bak-0.10.3-beta.5+custom.31`
里两个设置都没有，而新装的那份都有，名字却一样。因此新树升到 `custom.32`，
`custom.31` 留给之前装的那份。这与 `custom.29`、`custom.30` 是同一个坑，第三次踩。

**版本号再升到 `custom.33`**（同一次补记）：`custom.32` 的 stamp 提交
（`50418e750`）之后又落了 `ecc278f82`（`/model` picker 接上 `visibleModels.human`）。
于是同一串 `0.10.3-beta.5+custom.32` 对应两份行为不同的二进制：

| 哈希 | 构建时刻 | `/model` |
|---|---|---|
| `EDBA042B…FFA2` | 14:40:58 | 全量，`visibleModels` 不生效 |
| `FCDF843A…57A7` | 15:21:39 | 已过滤（从未部署，重编后已被覆盖） |

安装目录当时还没有 `.bak-…custom.32`，第一次覆盖就会造出一份同号而异行为的备份，
正是上面 `custom.31` 那段描述的坑，第四次。所以 picker 修复之后升到 `custom.33`
（`A346098F…1EA1`）：`custom.32` = 无 picker 过滤的那份（14:40 构建，装过安装目录），
`custom.33` = 含 `ecc278f82` 的那份。

**部署时必须同步 `package.json` 与 `.prime-agent-install`**：`--version` 优先读
exe 同目录 `package.json` 的 version（`crates/pa-cli/src/config.rs`），不是编进
二进制的。只换 exe 会让新二进制继续报旧号，这次 bump 想消灭的歧义会原样复现。

**版本号跳过 `custom.29`**：那个号属于 2026-10-30 装在安装目录里的构建，它的
工作区改动从未提交（只剩 `prime-agent.exe` 和 `package.json` 里的号）。复用
`custom.29` 会让新构建在 `--version`、备份文件名和回滚判断上与它无法区分，所以
`custom.28` 之后直接跳到 `custom.30`。安装目录里当前那份的备份是
`prime-agent.exe.bak-0.10.2+custom.29`。

`0e7a77c18`（daemon 把 provider 头贴到 `Model` 上，现为补丁 29）**已定性为正式
补丁，不是多余的**：上游 `c4c238ca0` 的 `opencode_headers::apply` 只在 provider
是 `opencode`/`opencode-go` 时补 `x-opencode-session`，且**要求 `session_id`
先进入 stream options**；而直连接缝（slash 命令、压缩、refine、图像路由）根本
不经过 stream options，拿不到那个头。两者入口不同，不重叠。

补丁 26–31 是补录的历史条目：它们一直在 `custom` 上（均为 HEAD 的祖先），但此前
没进本清单，导致 rebase 时没人知道有这么几处需要重新应用或验证。其中
`7591913b8`（补丁 28）新增了 257 行——`rpc/cron.rs`、`rpc/compaction.rs` 两个
新模块加一个调度器——体量大于清单里一半的条目，风险最高。

**这十条目前只有上面表格里的一行，没有下面的展开章节和验证记录**（下面 ## 1–25 只
覆盖补丁 1–25）。rebase 到这几条时按标题重放，并各自补一次验证——在补齐之前，
7591913b8 的实际行为没有任何测试证据。补丁 33–35 的完整说明在提交正文里
（`git log -1 b1d04d02a`、`git log -1 a829c0881`、`git log -1 b1021aa09`），
含各自的验证命令与结果。

---

## 0. 已移交上游（不要再当本地补丁）

**Authorization 去重**：原 `1004b1698` 已删除，改由上游 `85755cb8a` 承担。
上游的做法是「默认 bearer 先入列，model/caller header 大小写不敏感地覆盖它」，
与我们「删掉已有再写 api_key」相反：`authHeader: true` 时**显式 header 赢**，
而不是强制用 `apiKey`。本机 12 个 provider 全是 `authHeader: false`，
正常路径不带 `Authorization` 头部，两者行为等价。

**`ipython_state` 封顶与去重**：由上游 `e2bcc6add` 承担。名字清单封顶
200 个 / 4KB（超出加 `+N more`），`build_session_context` 只保留最新一份，
摘要调用不再接收该行。这条直接修掉了 p8 会话「压缩后地板过高 → 连环压缩」的病。

**opencode 会话身份**：由上游 `c4c238ca0` 承担。`session_id` 进入
`StreamOptions`，`opencode_headers::apply` 为 `opencode`/`opencode-go`
补 `User-Agent` 与 `x-opencode-session`，且 caller header 优先。

**refine 的团队头（v0.10.2 引入，本仓已合并）**：上游 `1ee259820` 让
`default_refiner_call` 接收会话合并后的请求头（Prime 的 `X-Prime-Team-ID`）
并在请求上设置。我们那条 passthrough 补丁原本是整表赋值，会把它覆盖掉——
rebase 在这里做了「机械合并、语义错误」的典型动作。已改为往现有表里
`entry().or_insert`，两边意图都保住。

## 1. 顶栏显示 cwd（`02941599f`）

**行为**：会话顶栏的显示名附带当前工作目录，便于区分同名会话。

**文件**：`crates/pa-tui/src/session_ui.rs`

## 2. 内核忙等待 30s（`d8812d2cc`）

**问题**：中断或快速切换后内核仍忙时，5 秒超时会过早报 "kernel busy"。

**行为**：`KERNEL_BUSY_REUSE_WAIT_MS` 由 `5_000` 改为 `30_000`。

**文件**：`crates/pa-core/src/kernel/shared.rs`

## 3. 自定义主题（`3d3d62df5`）

**问题**：0.10.0 的 `load_theme` 硬编码只认 `prime/dark/light`，未知名字静默
回退；`theme.rs` 里已有的 `load_theme_from_path` 没有任何调用者。

**行为**：新增 `resolve_theme(name, mode)`：先查内置名，未命中则读取
`<agent_dir>/themes/<name>.json`（沿用既有解析器）。

**文件**：`crates/pa-tui/src/app.rs`、`crates/pa-tui/src/theme.rs`、
`crates/pa-tui/src/session_ui/settings.rs`

## 4. 黑窗修复（`5b934a67f`）

**问题**：Windows 上 `DETACHED_PROCESS` 会静默取消 `CREATE_NO_WINDOW`，
后台进程（MCP、更新器）各自弹出可见控制台窗口。

**行为**：分离 spawn 统一使用
`CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`，不再使用 `DETACHED_PROCESS`。

**文件**：`crates/pa-core/src/platform/process.rs`、
`crates/pa-core/src/update/installer.rs`

**验证**：spawn 前基线枚举窗口，批量触发 MCP/更新路径 0 弹窗。

## 5. turnHook（`b432b9c82`）

**行为**：`settings.json` 配置
`"turnHook": {"command": "...", "timeoutMs": 30000}`。每个被接纳的 turn 在
首个 LLM 请求前执行命令（stdin 收 `{prompt, cwd, session_id}`），stdout 的
`hookSpecificOutput.additionalContext` 追加进本轮 system prompt（请求级，
不写入会话历史）。

**文件**：`crates/pa-core/src/session_engine/{turn_hook,engine,wiring,mod,admission}.rs`、
`crates/pa-core/src/settings/{types,load,mod}.rs`、`crates/pa-agent/src/agent.rs`

**验证**：`cargo test -p pa-core turn_hook --lib`（10 项）+ e2e 假 provider
确认注入标记出现在请求体中、每 turn 一次。

## 6. stopHook（`8af0afa8a`）

**行为**：`settings.json` 配置
`"stopHook": {"command": "...", "timeoutMs": 70000}`。运行结束时把本次转录
按 dcode 格式（`{"schema_version":1,"role":...}` 每行一条）写到
`<agent_dir>/stop-hook/<session_id>.jsonl`，并向命令 stdin 传
`{session_id, transcript_path, cwd}`。用于把对话写回外部记忆系统
（如 hindsight）。

**文件**：`crates/pa-core/src/session_engine/{stop_hook,engine,wiring,mod}.rs`、
`crates/pa-core/src/settings/{types,load,mod}.rs`

**验证**：`cargo test -p pa-core stop_hook --lib`（8 项）+ e2e 假 hindsight
服务收到 `POST /v1/default/banks/<bank>/memories`。

## 7. stopHook 同步 spawn（`e78e1a927`）

**问题**：在 `tokio::spawn` 的分离任务里启动 hook，一次性运行时会在任务
被取消前退出，命令从未执行。

**行为**：改为在 `agent_end` 监听器内同步 spawn 进程，另起
`stop-hook-reaper` 线程负责轮询退出与超时 kill。

**文件**：`crates/pa-core/src/session_engine/{stop_hook,turn_hook,wiring}.rs`

## 8. updateGuard（`5a730ac9d`）

**行为**：`settings.json` 出现 `"updateGuard": {"hold": true, "message": "..."}`
时拦截 `/update`、`prime-agent update` 与包自更新（`hold:false` 放行）；
`--check` 仍可用。用于防止 `/update` 用官方包覆盖定制构建。

**文件**：`crates/pa-core/src/settings/{types,load,manager,mod}.rs`、
`crates/pa-cli/src/{self_update,client_update,client_settings,public_command}.rs`、
`crates/pa-tui/src/{client_settings,session_ui/commands}.rs`

## 9. 剪贴板中文乱码（`c9db67a78`）

**问题**：Windows 的 `clip.exe` 按控制台代码页（部分中文系统为 936/GBK）
解释 stdin，UTF-8 中文粘贴后变乱码。

**行为**：通过隐藏的 `cmd /d /s /c "chcp 65001>nul & clip"` 管道写入，
强制 UTF-8 代码页；不使用 UTF-16LE+BOM（会污染 ASCII 文本）。

**文件**：`crates/pa-tui/src/clipboard.rs`

## 10. 多行粘贴折叠（`2c1c12a89`）

**行为**：任何多行粘贴（≥2 行）都折叠为 `[paste #N +M lines]` 标记，
展开后仍是原文；原阈值 ≥10 行只对超长粘贴生效。

**文件**：`crates/pa-tui/src/editor/mod.rs`

## 11. clean-kernel-state（`ed58d68fe`）

**行为**：新增命令

```text
prime-agent clean-kernel-state [--days <n>] [--dry-run] [--json]
```

递归删除 `<agent_dir>/session-artifacts/**` 下 mtime 超过阈值（默认 1 天）的
`kernel-state.dill`、`kernel-state.json` 与 `kernel-state.*.tmp`。不删除会话、
`scheduled-jobs.json` 或目录结构；不跟随符号链接；占用中的文件只上报。

**文件**：`crates/pa-cli/src/{clean_kernel_state,command_registry,lib,public_command}.rs`

**验证**：6 项单测 + `cargo test -p pa-cli --lib`；实测 dry-run 与正式清理
均正常（可回收数百个文件、数 GB 空间）。

## 12. 空回复重试 + 流空闲超时（`ce3bbb7c3`）

**问题 A**：模型返回 `stopReason=stop` 但内容只有 thinking（无文本、无工具
调用）时，prime-agent 视为正常结束，用户看到"突然停住"。在一份真实会话
日志中观察到 120 次静默结束、17 次用户随后发 `?`/`继续`。

**问题 B**：响应头到达后若连接半死（无 RST），`next_text/next_bytes` 会永久
等待；`timeout_ms` 只覆盖响应头阶段。

**行为**：
- A：daemon 的 turn 循环检测空回复 → 移除已追加的空 assistant → 重发一次，
  并附加请求级提示（"上一条为空，继续任务"，不写入会话历史）；仍为空则发出
  `model_prompt_error` 警告行（TUI 显示为黄色状态）。
- B：流式 chunk 读取加 120s 空闲超时，超时转为
  `ProviderError::Connection{kind: Timeout}`，交给既有重试策略。

**文件**：`crates/pa-daemon/src/agent_engine/turn/model.rs`、
`crates/pa-core/src/session_engine/{retry_nudge,engine,wiring,mod}.rs`、
`crates/pa-ai/src/utils_inner/http.rs`
（测试：`crates/pa-daemon/src/agent_engine/tests/empty_reply.rs`）

**验证**：`cargo test -p pa-daemon empty_reply --lib`（真实 HTTP/SSE：重试请求
携带提示、最终输出正常、提示不落会话文件、连续空回复只产生 1 条警告）。

**限制**：`-p` 打印模式不经过该 daemon 路径，空回复重试仅对 TUI 生效；
流空闲超时两条路径都生效。

## 13. 停滞子代理重新派活 + 重试提示按回合隔离（`4d0434a99`）

**问题 A**：补丁 12 的重试仍失败时，顶层会话会显示警告行，但子代理只会变
为 idle。子代理仅在 **settle**（worker 队列清空）时由父代理收到
`CompletedWithoutReply`；常驻子代理空回复后不触发 settle，父代理永远等不到
结果，用户看到子代理"默默停住"。

**问题 B**：重试提示存在 `Option<String>` 槽里，只由下一次请求的 transform
取走。若提示入槽后、重试请求发出前发生 abort 或 worker 关闭，槽中残留的
提示会混进之后某次无关请求。

**行为**：
- A：检测到空回复且重试已用尽时，除发出警告行外，若当前引擎的
  `rlm_depth > 0`（子代理），通过 `AgentMessageController` 找到父代理
  （roster + durable 父子边）并发一条 agent message 说明"本轮空停、上下文
  未丢、重新派活即可继续"。警告文案按顶层/子代理区分。
- B：每次 attempt 开始时清空提示槽（`clear_retry_nudge`），被放弃的重试
  不会把提示泄漏给后续回合。

**文件**：`crates/pa-daemon/src/agent_engine/turn/model.rs`、
`crates/pa-daemon/src/agent_engine/{config,lifecycle}.rs`、
`crates/pa-daemon/src/agent_engine.rs`、
`crates/pa-core/src/session_engine/wiring.rs`
（测试：`crates/pa-daemon/src/agent_engine/tests/empty_reply.rs`）

**验证**：`cargo test -p pa-daemon empty_reply --lib`（4 项：重试携带提示、
连续空回复告警、子代理通知父代理、残留提示不泄漏）；
`cargo test -p pa-core retry_nudge --lib`（3 项）。

**限制**：补丁 12/13 的空回复重试与停滞通知都走 `pa-daemon` 的 `AgentSessionEngine`，`-p` 打印模式和 `--mode rpc` 用的都是 `pa-cli/print_runtime.rs` 的 `HeadlessEngine`，不经过它——这两项的 TUI 之外入口都没有这些保护。

## 14. 停滞通知只发给父代理一次（`f45a7d32b`）

**问题**：补丁 13 的通知无条件发送。子代理每开始一个新 turn 又空回复时都会再发
一次，父代理收到通知后回消息、子代理再跑一轮又空又发——形成正反馈，父代理被
同一句话刷屏，即使该子代理已被从 roster 删除。

**行为**：引擎新增 `stalled_run_announced: AtomicBool`。通知只在未置位时发送并
置位；投递失败则复位（一次网络抖动不能永久静默）；出现可见回复也复位（说明停滞
已解除，下次停滞应重新告知）。

**文件**：`crates/pa-daemon/src/agent_engine.rs`、
`crates/pa-daemon/src/agent_engine/{lifecycle,turn/model}.rs`
（测试：`crates/pa-daemon/src/agent_engine/tests/empty_reply.rs`）

**验证**：`cargo test -p pa-daemon empty_reply --lib`（5 项，新增
`a_repeat_stall_does_not_flood_the_parent`：连续两轮空回复只产生 1 条通知，
但每轮各 1 条警告行）。

## 15. 收养无 bootstrap 记录的 kernel venv（`5fc6b6f4e`）

**问题**：`.bootstrap-version` 是 bootstrap 最后一步写的。若 bootstrap 在装完
运行时、写记录前被杀（部署重启、多 worker 同时启动都常见），会留下一个完整可用
但没有记录的 venv。此后每次启动都判定 venv 过期并 `remove_dir_all`；只要还有别的
worker 的 kernel python 在该目录里运行，删除就会以"拒绝访问"失败，并永久重复——
表现为所有 `python_repl` 调用在几毫秒内失败。

**行为**：
- 记录缺失时先探测现有 venv：若其 python 能 import prime-agent runtime 与
  `dill`，则视为可用，补写缺失的记录并继续使用，不再删除。
- 探测不通过时才重建；重建前若旧目录删不掉（仍有 kernel python 在运行），直接报错说明阻塞原因，不把半删状态的目录交给 `uv venv`（它只会报 "already exists" 掩盖真因）。

**文件**：`crates/pa-core/src/kernel/bootstrap/mod.rs`、
`crates/pa-core/src/kernel/bootstrap/venv.rs`、
`crates/pa-core/src/kernel/bootstrap/venv/probe.rs`
（测试：`crates/pa-core/src/kernel/bootstrap/venv/tests.rs`、
`crates/pa-core/src/kernel/bootstrap/mod.rs`）

**验证**：`cargo test -p pa-core bootstrap:: --lib`（32 项，新增收养成功、
运行时损坏不收养、已有记录不动、无记录但可用的 venv 绝不被删除）。

## 16. 空回复重试前固定等 3 秒（`beae8d01e`）

**问题**：补丁 12 的重试是立即重发。空回复多由 provider 瞬时抖动引起，立刻重试
往往撞上同一个抖动，等于白耗掉唯一一次重试。

**行为**：重试前固定等待 3 秒（`EMPTY_REPLY_RETRY_DELAY_MS`）。用固定值而非指数
退避，因为只有一次重试。等待可取消：暂停期间收到 abort 会立即返回中止消息，不会
硬撑着再发请求。

**文件**：`crates/pa-daemon/src/agent_engine/turn/model.rs`

**验证**：`cargo test -p pa-daemon empty_reply --lib`（5 项；整组耗时从 0.11s
增至约 6.2s，证明等待真实生效）。

## 17. 托盘显示 provider 与配置的模型名（`cd4dbf0e5`）

**问题**：托盘右下角只渲染 `state.model.id`。`models.json` 的 `name` 字段完全没进入
UI，`opencode-go` 和 `opencode-go-hr` 上的同一个 `deepseek-v4.1-flash` 因此显示成
一模一样的两行。provider 其实早已在 wire 上（`SessionEngine::model_metadata` 同时给
`id`、`name`、`provider`），是 TUI 侧丢掉了。

**行为**：标签改为 `provider / name:effort`。`name` 缺省时用 id，provider 缺省时
只剩模型名（display-string 形态的 `state.model` 没有 provider）。**不**做「name 里
已含 provider 就省略前缀」的推断：`name` 是给人读的自由文本，写 `astra @ zao` 还是
`GLM-5.3 (YES工程师)` 由配置作者决定，靠字符串匹配猜重复会错判。

**取舍**：`/` 两侧各留一个空格，因为 `name` 里常带括号与 `@`，裸 `/` 会糊成一团。
代价是宽模型名会挤占右侧的上下文用量：本机最宽一条（
`opencode-go / DeepSeek-V4.1-Flash-Free (OpenCode GO):max`）56 个字符格。

**文件**：`crates/pa-tui/src/chrome.rs`（`model_name` 字段 + 拼接）、
`crates/pa-tui/src/snapshot.rs`（`Reconstructed::model_name` +
`model_display_name`）、`crates/pa-tui/src/session_ui.rs`、
`crates/pa-tui/src/session_ui/lifecycle.rs`（attach 折叠）、
`crates/pa-tui/src/session_ui/model_picker.rs`（切换模型；失败分支清空 name，
避免用旧模型的名字标注新模型）。

**验证**：`cargo check -p pa-tui --all-targets`。未加测试、未实机验证（按当时要求
只做编译验证）。

## 18. 推理档位一路带到线上（`e053de177`）

**问题**：选 `max` 从来没有到达 provider。三处独立丢失，任一都足以让档位消失：

1. `ThinkingBudgets` 没有 `xhigh` / `max` 字段——配置了也无处安放。
2. `adjust_max_tokens_for_thinking` 在选预算**之前**调用 `clamp_reasoning`，而后者把
   xhigh/max 映射成 high，于是该函数里 `Xhigh` 和 `Max` 两个匹配分支永远不可达，三档
   全都发出 `budgets.high`。
3. anthropic 的 budget 分支只发 `budget_tokens`。非 adaptive 路由没有 `thinking` 对象
   可以承载档位，token 数就是唯一痕迹；一旦模型输出上限把两档压成同一个数字，痕迹也
   没了。

**行为**：

- 默认阶梯延伸到 `max`（xhigh 32,768 / max 65,536），模型声明的输出上限仍然生效——
  那是服务端的事。
- anthropic budget 分支额外发 `output_config.effort`（映射后的档位名）。
- Bedrock 的自定义预算查找同样改为先按**请求档位**查，只有配置里该档为空才回退到
  clamp 后的档位。
- `openai-completions` 区分两种 `supportsReasoningEffort: false`：无声明时继承的是本
  crate 对服务端的**探测猜测**（prime-inference 被探测为 false 是因为它忽略该字段），
  这时不再压制模型已声明的档位；路由自己 compat 块里写的 false 是**显式声明**，仍然
  遵守。

**不改什么**：provider 拿这个值做什么，不在补丁范围内。本补丁只保证调用方的选择离开
本进程。

**文件**：`crates/pa-types/src/ai/mod.rs`、
`crates/pa-ai/src/providers/simple_options.rs`、
`crates/pa-ai/src/providers/anthropic/mod.rs`、
`crates/pa-ai/src/providers/anthropic/params.rs`、
`crates/pa-ai/src/providers/bedrock/mod.rs`、
`crates/pa-ai/src/providers/openai_completions/params.rs`

**配置侧**（`models.json`，不入库）：`yes` 的 `supportsReasoningEffort` 改为 true；
`opencode-go` 从 `anthropic-messages` 切到 `openai-completions`（baseUrl 补 `/v1`，
compat 补 `thinkingFormat: deepseek`）；三个 provider 的 `deepseek-v4.1-flash`
`maxTokens` 提到 50,000，否则模型上限会把 xhigh 与 max 压成同一个数字。

**验证**：`cargo test -p pa-ai --lib`（373 项通过；`oauth::anthropic_callback`
因端口占用失败，改动前即失败）。新增 5 个测试钉住行为，其中
`the_switched_opencode_go_route_sends_max` 断言报文含
`reasoning_effort: "max"` 与 `thinking: {type: enabled}`。
## 19. `/speed` 开关持久化（`577e1bc78`）

**问题**：`/speed on` 只改内存字段，退出重开就没了。同类的 `chatDetail`（Ctrl+O）
有持久化，`speed` 缺了那一半：`client_settings` trait 里根本没有 `speed_display`
的读写口，初始化处硬编码 `speed_display_enabled: false`。

**行为**：新增 `speedDisplay` 设置键（`settings.json` 全局作用域）。`/speed on|off`
写入该键，客户端启动时读回。未设置读作关闭，与历史行为一致。

**文件**：`crates/pa-core/src/settings/types.rs`（字段）、
`crates/pa-core/src/settings/load.rs`（`KNOWN_FIELDS`）、
`crates/pa-core/src/settings/manager.rs`（`get_speed_display`，读合并文档的
getter 都在这里）、`crates/pa-core/src/settings/interactive_settings.rs`
（`set_speed_display`）、`crates/pa-tui/src/client_settings.rs`（trait 默认实现）、
`crates/pa-cli/src/client_settings.rs`（`setting!` 接线）、
`crates/pa-tui/src/session_ui/stream.rs`（写入）、
`crates/pa-tui/src/session_ui/lifecycle.rs`（启动读回）

**验证**：`cargo test -p pa-core --lib settings::`（45 项通过，含新增
`speed_display_persists_and_reads_back`，断言写入只动目标键、重载后读回为 true、
再关掉读回为 false）。`cargo check --workspace --all-targets` 通过。
## 20. 新增 `session clean` 清理空会话（`09244c28b`）

**问题**：`prime-agent` 每次启动都创建会话文件，所以在第一个 prompt 之前关掉的启动会
留下一个只有 bootstrap 记录的残file。它们不会自己消失，也不在任何列表里提示：本机
136 个会话里有 82 个是这种，合计 60 KB，单个都不超过 1.3 KB。

**判定**：按**记录类型**判断，不用字节阈值。一个会话为空，当且仅当它每一行都是
bootstrap 记录（`session` / `model_change` / `thinking_level_change` /
`service_tier_change` / `session_state`）；出现任意一条 `message` 或
`custom_message` 就不是空。用大小阈值会误删——一行回复和裸 `/compact` 都比这些空
文件还小。

**三道保护**：`session_info` 命名的会话默认保留（`--include-named` 才删）；
比 `--min-age-hours`（默认 1 小时）新的保留，避免误伤「刚启动还在等第一个 prompt」
的会话；任何一行解析失败就把该文件标记为不可读并保留，绝不当作空。

**用法**：
```bash
prime-agent session clean --dry-run      # 只列清单
prime-agent session clean                # 执行
prime-agent session clean --json         # 供脚本消费
```

**接线**：`session` 命名空间现在有 `export` 和 `clean` 两个子命令，
`rewrite_nested_command`（把一个命名空间压成单个内部 flag）不再覆盖它，所以分发处
多了一个分支。

**文件**：`crates/pa-cli/src/session_clean.rs`（新增）、
`crates/pa-cli/src/public_command.rs`、`crates/pa-cli/src/command_registry.rs`、
`crates/pa-cli/src/lib.rs`

**验证**：`cargo test -p pa-cli --lib`（295 项通过，含 8 个新增测试，覆盖：删空会话、
保留一行消息、保留只有 `/compact` 的、保留命名会话、保留过新的、`--dry-run` 不落盘、
坏行保留、选项解析）。本机实跑：136 → 54，删除 82 个（59.3 KB），保留 54 个
（52 个有内容 + 2 个命名），54 个有内容会话字节数无变化。删除前已备份到
`sessions-backup/empty-<时间戳>/`，82 个文件全部校验可解析。

## 21. refine 的 passthrough 头改为合并（`1f7de0f08`）

**问题**：上游 `v0.10.2`（`1ee259820`）让 `default_refiner_call` 接收会话合并后的
请求头——Prime 的 `X-Prime-Team-ID` 走这条路——并在请求上设置它们。我们早先那条
passthrough 是整表赋值：

```rust
stream_options.base.headers = Some(HashMap::from([("X-WB2A-Prompt-Mode", "passthrough")]));
```

于是每次 refine 调用都静默丢掉 provider 的路由身份。两处改动落在同一行、意图不同，
正是 rebase 会「机械合并、语义错误」的那类冲突：文本不冲突，语义冲突。

**行为**：改为往现有表里插入，且只在键不存在时写。

**文件**：`crates/pa-core/src/session_engine/refine.rs`

**验证**：`cargo test -p pa-core --lib refine`（60 项通过；两个失败
`refine_request_is_identical_across_extraction_paths` 与
`historical_refinement_is_read_only_on_trigger_not_in_hot_cache` 在 stash 掉本改动后
同样失败，是上游 `v0.10.2` 的既有问题，与本补丁无关）。

## 22. 多行粘贴在写边界被拆开时仍按粘贴处理（`7b3cd948b`）

**问题**：往输入框粘贴多行文本会一行一行发出去，最后一行留在框里。把「一次终端写入」
识别成一次粘贴的判定，要求换行串**两侧都有文本**：

```rust
if newline_run_start > 0 { return true }   // 前面必须有字符
```

终端把一次 Ctrl+V 按内核读边界切开，切出来的块可能**以换行开头**或**整块就是换行**。
这些块判定失败，退回逐事件派发，块里的每个 `Enter` 都提交一次 prompt。本仓早先那次
修复只覆盖了「整块写入」的情形，所以症状会回来。

**行为**：判据改为「含换行的写入就是粘贴，除非它恰好是一个换行（裸 `Enter` 按键）」。
不含换行的写入仍是普通打字。编辑器的粘贴处理本来就会把多行折叠成
`[paste #N +K lines]`，marker 不含换行，所以被识别出的粘贴不可能自我提交。

**已知的过度匹配**：同一写入里「先打字再按回车」。以人类的打字速度，配合一个每次写入
都即时 drain 的读取线程，这不会发生；万一发生，后果是出现一个 marker 而不是提交——
方向与这个判定要防的错误相反。

**文件**：`crates/pa-tui/src/input.rs`

**验证**：`cargo test -p pa-tui --lib input`（47 项通过）。改写了一个既有测试
（`a_forwarded_chunk_ends_with_its_marker`）：它用 `/l` + Enter 当样本，而那个样本
现在被正确地当成粘贴；该测试断言的是 chunk-end marker，与按键形态无关，故去掉 Enter。
`cargo test -p pa-tui --lib` 1514 通过 / 8 失败，8 个失败属 `autocomplete`、
`incident_notices`、`session_open_error`，是上游 `v0.10.2` 的既有问题。

## 23. agents view 的 Model 列显示 `provider / model`（`61c6e2765`）

**问题**：会话列表的 Model 列只渲染裸模型 id，`session_model` 还主动剥掉了 vendor 前缀
（`id.rsplit('/').next()`）。多个 provider 提供同一个模型 id，
`deepseek-v4.1-flash` 无法说明会话跑在哪一个上——与托盘标签加 provider 是同一个理由。

**行为**：显示 `provider / model:level`。provider 取自模型对象（`model.provider`，实时
worker 的形状）或 summary 顶层的 `provider`（seeded / saved 的形状）。**已经带
`vendor/` 前缀的 id 保持裸写**：`internal/glm-5.3-fast` 是标识符而不是 provider 路径，
写成 `prime-inference / internal/glm-5.3-fast` 会读成两个 provider。

**列宽**：Model 列原上限 32 格，装不下
`opencode-go-hr / deepseek-v4.1-flash:max`（40 格），会让每一条带 provider 的行都截断。
提到 44：80 列终端能完整显示，窄终端仍优先收缩 Model 列而不是挤掉 Session 名。

**文件**：`crates/pa-tui/src/agents_view_forest/summary.rs`、
`crates/pa-tui/src/agents_view_state.rs`

**验证**：`cargo test -p pa-tui --lib agents_view`（210 项通过）。新增两个测试：
`session_model_names_the_provider_for_a_shared_model_id`（三个 provider 同一 id，
以及实时/顶层两种 provider 形状）和
`the_model_column_fits_a_qualified_provider_model_pair`（80 列完整、60 列收缩且保留
Session 名列下限）。两处既有断言从裸 id 更新为带 provider 的形态。
`cargo test -p pa-tui --lib` 1516 通过 / 8 失败，8 个失败为上游既有问题。

## 24. 用户消息回显改为纯文本（`5f2bcd46d`）

**问题**：提交后的用户消息块走 markdown 渲染，之后又把这个块里所有 span 的样式
统一改写成 `userMessageText` 一种颜色。也就是说 **markdown 产生的粗体/斜体/删除线
全被抹掉**，只剩「吞分隔符」这一半行为：编辑器里写的是 `__init__.py`，回显成了
`init.py`。

**实测丢失清单**（改前，用户消息块）：`__init__.py`→`init.py`、`_private`→`private`、
`2*3*4`→`234`、`x = 2 * 3 * 4`→`x = 2  3  4`、`~~del~~`→`del`、两个反引号→空、
`` `code` ``→`code`、`#`/`##`→空行、`# comment`→`comment`、`---`→分隔线、
`> a`/`>>`/`>=`→`│ a`/`│ >`/`│ =`、`\*`→`*`、`[a](b)`→`a (b)`、
`说明：_下划线_ 很重要`→下划线消失。共 22 项。

注意 `snake_case_name`、`MAX_TOKENS`、`1_000_000` 本来是对的——CommonMark 有词内
下划线规则。出错的是**词边界上的**下划线，也就是 `__init__`、`__pycache__` 这类，
正是之前那次「`_` 不显示」的实际场景。

**行为**：回显按字面排版（按宽度折行，一个源行一行），保留两项能力——`@path` /
`--flag` 仍通过既有的 prompt-token mask 高亮；裸 URL 仍带 OSC 8 区域，终端里可点。
**输入的字符一个都不会被消费**。URL 识别复用 markdown inline 渲染器用的同一套 GFM
`url` 语法（`autolink.rs` 里把 backpedal 抽成共享的 `backpedal_range`），所以
`see https://x.dev/a.` 只链接到 `.dev/a`，句号留在文本里。

**行数一致性**：`user_block_row_count` 改成调用与绘制同一个 `wrap_user_text`，
布局不会错位。

**取舍**：用户消息里的 markdown 链接语法（`[a](b)`）不再被解释成「标签 + 链接」，
而是原样显示。这是刻意的——回显的契约是「显示编辑器里实际写的字符」。

**验证**：`cargo test -p pa-tui --lib chat::`（27 项通过）。新增
`user_block_never_interprets_markdown`（24 个回归形状逐字符比对）。两个 view 测试
原本断言「用户消息的相对链接按所属视图 cwd 解析」，该路径已按设计移除，改为在
**assistant 消息**上断言，保住 cwd 解析这条契约本身。

## 25. 清掉 fork 的 CI 门（`d1b357e93`）

**问题**：merge gate 是 `cargo fmt --all --check` +
`cargo clippy --workspace --all-targets -- -D warnings`，两条都不过。

- **fmt**：33 处 diff / 16 个文件，全部是本 fork 新增或改动的行（各文件的上游版本
  我用 `git show upstream/main:<file>` 复核过，都是干净的）。
- **clippy**（在 CI 固定的 1.98.1 上）：`map_or(false, ..)` 4 处（应为
  `is_ok_and`，因为 `get_shell_config` 返回 `Result`）、`retry_nudge.rs` 的通配
  分支、`empty_reply.rs` 3 处 `push_str(&format!(..))`、`clipboard.rs` 的
  const 位置、`prompt_commands.rs` 缺 `# Panics`、`model_resolution.rs` 的文档
  反引号与冗余 clone、`bootstrap/mod.rs` 可折叠 `if`、`session_clean.rs`
  两个分支相同的 `if`（原文两侧都打印 `""`，属于缺陷残留，dry-run 提示本就在
  下方单独打印）。
- **乱码**：`turn/model.rs` 里两个 U+FFFD，原文应是破折号。全仓受版本控制的
  文本文件扫过，只此一处。

**工具链陷阱（下次评审注意）**：本地是 1.99.1，直接用它会报 **264** 条告警，
其中 **242** 条是 1.99 新加进 pedantic 的 `assert!` 空值判定 lint，全部落在
本 fork 没碰过的文件里。按 CI 固定的 1.98.1 复跑只剩 18 条，修完为 0 错误。
**判断这道门必须用 1.98.1**（`rustup toolchain install 1.98.1 --component clippy`）。

**验证**：1.98.1 下 `cargo fmt --all --check` 干净、
`cargo clippy --workspace --all-targets -- -D warnings` 零错误。
整体测试与既有基线一致（`pa-cli` 295/0，`pa-ai` 1 项端口占用，`pa-tui` 8 项，
`pa-core` 41 项环境/上游失败）。顺带清掉了测试污染产生的
`crates/pa-core/C:Users1...resolve-retry-<pid>` 垃圾文件。

## 32. provider 报错不再冒充空回复 + refine 不再压低输出预算（`11a542736`）

两个行为缺陷，同一个来源：把「没拿到东西」的两种完全不同的原因混为一谈。

**A. provider 失败被当成空回复重试**

`agent_loop` 报 provider 失败的方式是**正常返回 `Ok(())`**，把错误放在一条
assistant 行里：`content` 为空、`stop_reason` 是 `Error`（用户取消是 `Aborted`）。
`run_turn_once` 把这条行原样交回，而 `is_empty_reply` 只看 content。链路于是变成：

```
provider 400/401 → content 空、stop_reason=Error
  → is_empty_reply == true
  → 空等 3 秒（EMPTY_REPLY_RETRY_DELAY_MS）
  → 同一个必然失败的请求再发一次
  → 仍然空 → 打「模型返回空回复」警告
```

三重代价：多付一次请求（可能计费）、白等 3 秒、**诊断指向错误的原因**——这正是
补丁 30/31 在 refine 路径上花力气修掉的那类误导，在 daemon 这条路径上原样留着。
它还会把 `stalled_run_announced` 的门关掉，于是父代理收到「我空了」的通知，而真实
原因是 provider 拒绝。

修改：`is_empty_reply` 对 `Error`/`Aborted` 返回 false。错误行照常进 transcript —
—`finalize_assistant_error` 在 `stop_reason == "error"` 时渲染 `errorMessage`，
用户看到的是 provider 的原话（或既有的 "Error" 兜底），不再是编造的空回复。

**B. refine 把每个模型的输出预算钉成 8 192**

原来那行注释写「仅在模型未声明上限时加地板，绝不覆盖声明值」，但
`base.max_tokens` 在这里**恒为 `None`**（`SimpleStreamOptions` 由
`..Default::default()` 构造），所以 `get_or_insert(8_192)` 每次都触发。它还越过了
本已正确的解析：`build_base_options` 会回退到 `default_request_max_tokens(model)`，
显式 `maxTokens` 原样放行、catalog 值按 32 000 封顶。声明 50 000 输出预算的模型
被静默压到 8 192，让注释担心的「输出截断」在机器解析的 JSON 上**更**容易发生。

修改：删掉那一行，由 `build_base_options` 决定。

**验证**：`cargo test -p pa-daemon --lib empty_reply`（7 项通过，含两个新测试，
分别钉住谓词的两个方向：`Error`/`Aborted` 不进重试、`stop` + 空内容仍进重试）、
`cargo test -p pa-daemon --lib agent_engine::turn::model::tests`（4 项通过）。
`cargo test -p pa-core --lib session_engine::refine` 12 通过 / 1 失败——该失败是
`refine_request_is_identical_across_extraction_paths`，基线同样失败（断言在
`refine_transcript_parts` 上，与请求选项无关）。
