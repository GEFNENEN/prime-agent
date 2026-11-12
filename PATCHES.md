# 补丁清单（实时）

本文件只描述 **当前 `custom` 分支相对上游基线的补丁**，以 HEAD 为准。补丁被移除
或合并进上游后，对应条目直接删除，不保留历史（历史见 `git log main..custom`）。

每条的原理、改动文件与验证记录都在提交正文里（`git log -1 <hash>`），这里只留
一行说清它做了什么。

- 基线：`v0.10.3`（tag）@ `d649b93cf`（2026-11-11；上一个同步点是 `6aaa3a255`）。
  这一版有 tag 覆盖，且 tag 就是 `upstream/main` 的 tip
  （`git rev-list --count v0.10.3..upstream/main` = 0），所以这次按 tag 同步。
- 补丁清单见下表
- 维护流程见 [MAINTAINING.md](MAINTAINING.md)

`提交` 列存的是当前树里的 hash。rebase 会重写 hash，届时这一列会全部失效。

**rebase 前先把这张表备份一份**——旧的 hash 可能随 rebase 一起不可达（本轮那 37
个就只能通过本地 tag `custom-pre-sync-20261104` 找回，且该 tag 没推到 origin），
到时候连标题都取不到。备份之后按下面两步回填：

1. 用 `git log -1 --format=%s <旧hash>` 取原来的英文标题。
2. 用 `git log --fixed-strings --grep=<该标题>` 在 HEAD 里定位（本轮同步就是这样
   把全部 37 条修回来的）。

没有备份、旧 hash 又已不可达时，只剩人工对照：`git log --all --format='%h %s'`
把标题全列出来，按语义认。**注意两件事**：表格「摘要」列是中文转述、不是可以 grep
的键（提交标题是英文，拿中文去 `--grep` 一条都命中不了）；`--grep` 匹配的是**整条
提交消息、含正文**，不只是标题，所以片段一定会连带命中正文里提过它的提交（文档提交
尤其如此）。片段只用来缩小范围，最后要拿**完整标题**定位才唯一。

| # | 提交 | 类型 | 摘要 |
| --- | --- | --- | --- |
| 1 | `0ba10af4d` | fix | 顶栏显示会话名与（会话目录, 终端 cwd）；无名会话显示 `unnamed`，两者不一致时 cwd 标红（收尾 `c97207137`） |
| 2 | `cd105a29b` | fix | 内核忙时等待更久（5s → 30s） |
| 3 | `c23b926ed` | fix | 从 agent 主题目录解析自定义主题 |
| 4 | `f8b7fce94` | fix | 分离进程不再弹出控制台窗口 |
| 5 | `4dba733f8` | feat | 每个被接纳的 turn 前运行 turnHook |
| 6 | `5a539c060` | feat | 运行结束后通过 stopHook 写回 |
| 7 | `11ae3d11f` | fix | 在运行结束前同步 spawn stopHook |
| 8 | `57aceaeb9` | feat | 用 updateGuard 拦截自更新 |
| 9 | `e59034d9d` | fix | Windows 剪贴板走 UTF-8 控制台 |
| 10 | `37d8e8deb` | fix | 所有多行粘贴折叠为标记 |
| 11 | `8fbf4dccb` | feat | 新增 clean-kernel-state 命令 |
| 12 | `5c93f35d4` | fix | 流空闲超时：chunk 120s 无数据记为超时连接的失败（空回复自动重试已交还上游 #3968，收尾 `913a5db39`） |
| 13 | `72b8638fa` | fix | 子代理对被注入的消息沉默时通知父代理重新派活（收尾 `913a5db39`：不再自带重试与 nudge） |
| 14 | `68687bafe` | fix | 停滞通知只发给父代理一次 |
| 15 | `a559288eb` | fix | 收养无 bootstrap 记录的 kernel venv |
| 16 | `16378ab98` | fix | 托盘显示 provider 与配置的模型名 |
| 17 | `e5d71ca75` | fix | 推理档位一路带到线上 |
| 18 | `a228a6cf0` | fix | `/speed` 开关持久化 |
| 19 | `325baa964` | feat | 新增 session clean 清理空会话 |
| 20 | `a0031bcfa` | fix | refine 的 passthrough 头改为合并而非整表覆盖 |
| 21 | `1611ae81e` | fix | 多行粘贴在写边界被拆开时仍按粘贴处理 |
| 22 | `ab1e44474` | feat | agents view 的 Model 列显示 provider / model |
| 23 | `c766bb6b6` | fix | 用户消息回显改为纯文本，不再吞字符 |
| 24 | `d91dc44b1` | fix | 清掉 fork 的 CI 门（fmt / clippy / 乱码注释） |
| 25 | `598db2cb3` | fix | 单独的 `>` 保持字面显示（APPEND 续跑命令） |
| 26 | `40e3509eb` | fix | RPC 的 lease 探测移出 tokio 运行时 |
| 27 | `eeb5f8f11` | fix | RPC 会话接入心跳与阈值自动压缩 |
| 28 | `dfd3eb3bc` | fix | daemon 把 provider 头贴到 `Model` 上给直连接缝用 |
| 29 | `ad485cb1f` | fix | refine 请求保持机器可解析，失败时透出 provider 原话 |
| 30 | `495d0611b` | fix | refine 的空回复在解析错误里点名 |
| 31 | `b849979ac` | fix | provider 报错不再被当成空回复重试；refine 不再压低输出预算 |
| 32 | `2a29d8271` | fix | Windows 括号粘贴从按键流重组为一次粘贴 |
| 33 | `91055dd21` | feat | ACP：Python cell 按代码首行命名，并上报上下文用量 |
| 34 | `314e978c7` | feat | TUI：`PASTE_TRACE` 打印粘贴路径上的原始终端事件 |
| 35 | `a57b4302b` | feat | 按受众（人 / agent）限定可见模型与推理档位：`visibleModels`、`thinkingLevels` |
| 36 | `66d2120ae` | fix | `/model` picker 接上 `visibleModels.human`（收尾：`8bb327432`、`eed97a306`） |
| 37 | `66006be23` | feat | MCP 调用默认超时：全局设置 `mcpCallTimeoutMs`，server 自带的 `callTimeoutMs` 优先 |
| 38 | `e450a6171` | feat | MCP stdio 的 `env` 支持字面值（`{"value": ...}` 与 `--env-value CHILD=VALUE`） |
| 39 | `d467fdf24` | fix | `python_repl` 的 per-call 超时上限跟随会话的 `pythonCellTimeoutMs` |

表格按**行为补丁**计数，所以行数与提交数**不相等**：同一个补丁的后续收尾提交
（对已入表补丁的修正、重构、补测试）并入该行的摘要，不另开一行——表末那条就是
这样收了 `66d2120ae` 之后的两个（`8bb327432` 重构、`eed97a306` 补测试）。想核对
总数时用 `git log --oneline v0.10.3..HEAD` 与表格、`docs:`、`chore: stamp`、
rebase 修复、收尾提交、行尾修复逐项对照；任何一侧新增提交都会让固定算术式过期，
所以这里不写死数字。

补丁之外还有五类提交不进表格：文档提交、版本戳提交、2 个 rebase 修复提交
（`0094eb36b`、`08fdd68b0`，改 `pa-tui` 的测试调用点）、已入表补丁的收尾提交、
以及行尾修复提交（`296975cf4`）。文档与版本戳提交随 rebase 改写 hash，按标题找
（`docs:` / `chore: stamp` 开头）；修复提交是重放破损的修补，不是本地补丁。

补丁 26–31 是补录条目：它们一直在 `custom` 上（均为 HEAD 的祖先），此前没进清单，
导致 rebase 时没人知道有这几处需要重新应用或验证。其中 `eeb5f8f11`（补丁 28）
体量最大、风险最高——新增一个 RPC 调度器与阈值压缩（`rpc/cron.rs`、
`rpc/compaction.rs`），rebase 到它时按标题重放并补一次验证。

## 版本号为什么跳号

`--version`、备份文件名、回滚判断都靠这个号区分二进制，所以「同号不同行为」必须
换号。历史坑：

| 号 | 说明 |
| --- | --- |
| `custom.29` | 跳过。那份构建的改动从未提交，只剩 exe 与 `package.json` 里的号 |
| `custom.30` | 安装目录里是 `0.10.2+custom.30`；rebase 后新树另升 `custom.31`，该号不再给新树 |
| `custom.31` | stamp 早于 `visibleModels`/`thinkingLevels`，且从未装进安装目录 |
| `custom.32` | 不含 `/model` picker 过滤（14:40 构建，装过安装目录） |
| `custom.33` | 含 `66d2120ae` 及收尾 |
| `custom.34` | 本次同步到 `c4d420183`。二进制内容变了，所以升号 |
| `custom.35` | 在 `custom.34` 之上加了 MCP 调用超时（`mcpCallTimeoutMs`）、MCP `env` 字面值，以及 `python_repl` 的 per-call 超时上限（补丁 37–39） |
| `custom.36` | 同步上游到 `6aaa3a255`（25 个提交），并把空回复自动重试交还上游 #3968（收尾 `913a5db39`）。二进制内容变了，所以升号 |
| `custom.37` | 顶栏改成「会话名 (会话目录, 终端 cwd)」，无名会话显示 `unnamed`，两个目录不一致时 cwd 标红（补丁 1 收尾 `c97207137`） |
| `custom.38` | 同步上游到 `v0.10.3`（`d649b93cf`，8 个提交）。基线版本从 `0.10.3-beta.5` 变成 `0.10.3`，所以后缀直接是 `0.10.3+custom.38`；二进制内容变了，所以升号 |

stamp 提交写于哪个基线，rebase 后正文里的版本号可能与实际写入的不一致；那种提交
正文不追改（改写历史只为一个字符串不值当）。唯一例外是**标题行**：`v0.10.3` 那次
基线版本变了（`0.10.3-beta.5` → `0.10.3`），把 `chore: stamp this fork build as
0.10.3-beta.5+custom.N` 的标题统一改成 `0.10.3+custom.N`，否则标题与实际写入的号
对不上；更早的 `0.10.2+custom.N` 标题属于它当时写下的号，保持原样。

## 下次同步注意

**冲突化解（`6aaa3a255` → `v0.10.3` 那次，8 个提交）**

1. 上游有一个 tag-only 的 nightly 版本提交（只改 Cargo.toml / Cargo.lock 的版本行，
   分支上从不落地）：rebase 到它必然冲突，直接 `git rebase --skip` 丢掉。
2. 12 个 `chore: stamp` 提交逐个冲突在 Cargo.toml / Cargo.lock，冲突内容只有版本行。
   仓库里这两个文件是 **CRLF**（`git ls-files --eol` 显示 `i/crlf`，
   `core.autocrlf=false`，没有 `.gitattributes`）：用 `Set-Content` 之类默认换行重写
   会造成整文件冲突，要用 `[IO.File]::ReadAllBytes` + `WriteAllText(..., UTF8Encoding($false))`
   做字节级替换。
3. `build: turn incremental compilation off for the dev profile` 除了 MAINTAINING.md
   还改了 Cargo.toml（在 `[profile.release]` 末尾追加 `[profile.dev] incremental =
   false`），不能用「取 ours 再改版本行」的通用解法——那样会把它真正的改动丢掉，
   提交退化成只剩文档。这类提交要在 `--ours` 之后把那段补回再 `commit --amend`。
4. 基线版本变了以后 stamp 提交的**标题**要一起改（见上一节），否则标题里的
   `0.10.3-beta.5+custom.N` 与树里的 `0.10.3+custom.N` 对不上。

**冲突化解（`c4d420183` → `6aaa3a255` 那次，25 个提交）**

1. `crates/pa-core/src/settings/mod.rs` 的 `pub use types::{...}` 列表连续撞四次
   （turnHook、stopHook、updateGuard、visibleModels 各加一个设置类型）：每次都取
   并集，`WebSearchProvider` 保留。
2. `crates/pa-core/src/session_engine/engine.rs` 的 settings 快照区：上游的
   `turn_hook_settings`/`stop_hook_settings` 与本分支的 `web_search_provider` 并列保留。
3. `crates/pa-core/src/models/registry/tests.rs`：两边各在文件末尾加测试，冲突区间
   连带吃掉共享的那个 `}`——合并后在上游测试最后一个断言
   （`assert_eq!(overridden.serving, None);`）后补 `}`。
4. rustc 才发现的静默破裂：`crates/pa-ai/src/providers/anthropic/params.rs` 的
   `Model { .. }` 字面量要补 `serving: None`（#4009 新增字段）。
5. rustc 才发现的静默破裂：`crates/pa-cli/src/print_runtime.rs` 的
   `HeadlessEmbedding::Rpc` 变成结构体变体（#4007），本分支的 `Rpc =>` 要写成
   `Rpc { .. } =>`。

后两处用完 `cargo check --workspace --all-targets` 才暴露；本机 `rerere` 是开着的，
同一处重复冲突（`engine.rs` 第二次）会自动套用上次的解法。

**冲突化解（`v0.10.2` → `v0.10.3-beta.5` 那次，`c4d420183` 同步同样适用）**

1. `#3849` 的 `rlm_child_model::resolve_child_cwd` 与 `rlm_children/host.rs` 里原有
   的内联 `match &request.cwd` 语义重叠（那个 `match` 是上游 `v0.10.2` 就有的，
   不是本分支补丁）：**采纳上游**，内联 `match` 换成 `resolve_child_cwd`，
   `rlm_children.rs` 与 `rlm_children/host.rs` 补 import。
2. `#3777` 给 `RlmSessionIdentity` 加 `factory_child` 字段：
   `agent_engine/tests/empty_reply.rs` 两处结构体字面量补 `factory_child: false`。
3. 本地把 `assert_thinking_supported` 换成 `clamp_child_thinking` 时删过 `bail`
   import，上游仍用 `bail!`：`rlm_child_model.rs` 要加回。

解决冲突时容易「删标记行连带吃掉相邻 `}`」，测试文件尤其常见——rebase 前先跑一次
`cargo check -p <crate> --all-targets`。`crates/pa-tui/src/input.rs` 承载补丁
22/33/35，是 rebase 高发区，且历史上被误写成过 CRLF（`296975cf4` 只转回 LF）；
同步前用 `git diff --ignore-cr-at-eol` 确认没有整文件噪声。

**两条不当门禁、也不当故障的现象**

- `cargo clippy -D warnings` 在基线上同样失败：本机工具链比上游 CI 新，新 lint 会
  命中上游代码（`vendor/crossterm`、`pa-telemetry`、`pa-ai` 生成文件）。用
  `cargo fmt --check` + `cargo check --all-targets`。
- `list-models` 打印完不退出是既有行为，与版本无关；`-p` 也观察到过无输出挂起
  （20 秒 stdout/stderr 全空，原因未查清）。别用它们做验收。

## 已移交上游（不要再当本地补丁）

- **Authorization 去重**：原 `1004b1698` 已删除，由上游 `85755cb8a` 承担——默认
  bearer 先入列，model/caller header 大小写不敏感地覆盖它；`authHeader: true` 时
  显式 header 赢。12 个 provider 全为 `authHeader: false`，两条路径打的
  `Authorization` 头等价。
- **`ipython_state` 顶栏上限**：上游 `e2bcc6add` 已限 200 行 / 4KB 并加 `+N more`。
- **opencode 会话头**：上游 `c4c238ca0` 已把 `session_id` 放进 `StreamOptions`。
  但那只覆盖 stream options 路径；直连接缝（slash 命令、压缩、refine、图像路由）
  不经过它，所以补丁 29 仍是必需的，两者不重叠。
- **refine 信任头**：`v0.10.2` 起上游已合并。
- **空回复自动重试**：上游 `d919386ab`（#3968）把「用户回合的静默回复」判成可重试的
  `incomplete_reply` 失败——同一条回合链里重试一次，仍空就以
  `The model ended its reply with no visible answer` 收尾。本分支原来的重试循环、
  3 秒等待、请求级 nudge 与可见警告行随之删除（收尾 `913a5db39`）。留下的只是上游
  不覆盖的那半：子代理对**被注入**的消息沉默时（`owes_answer == false`）通知父代理
  重新派活，见补丁 13。
