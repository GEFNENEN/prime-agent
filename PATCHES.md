# 补丁清单（实时）

本文件只描述 **当前 `custom` 分支相对上游基线的补丁**，以 HEAD 为准。补丁被移除
或合并进上游后，对应条目直接删除，不保留历史（历史见 `git log main..custom`）。

每条的原理、改动文件与验证记录都在提交正文里（`git log -1 <hash>`），这里只留
一行说清它做了什么。

- 基线：`upstream/main` @ `32f83edb1`（2026-11-25；上一个同步点是 `upstream/main` @
  `bb21eae7b`）。同步时上游距稳定 tag `v0.10.5` 112 个提交，无新稳定版；本轮把
  `upstream/main` 头部的 async kernel、session cost ledger 与 daemon 可靠性改动一并带入。
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
| 1 | `1c7389011` | fix | 顶栏显示会话名与（会话目录, 终端 cwd）；无名会话显示 `unnamed`，两者不一致时 cwd 标红（收尾 `23827d358`） |
| 2 | `57281ac9f` | fix | 内核忙时等待更久（5s → 30s） |
| 3 | `141ad2bff` | fix | 从 agent 主题目录解析自定义主题 |
| 4 | `b3e7d0de0` | fix | 分离进程不再弹出控制台窗口 |
| 5 | `b23bd8f9d` | feat | 每个被接纳的 turn 前运行 turnHook |
| 6 | `d7f0911a4` | feat | 运行结束后通过 stopHook 写回 |
| 7 | `cbbe0a736` | fix | 在运行结束前同步 spawn stopHook |
| 8 | `bed189a88` | feat | 用 updateGuard 拦截自更新 |
| 9 | `a716af122` | fix | Windows 剪贴板走 UTF-8 控制台 |
| 10 | `3d9b3eda8` | fix | 所有多行粘贴折叠为标记 |
| 11 | `ecddd5276` | feat | 新增 clean-kernel-state 命令 |
| 12 | `2b8a9bdc7` | fix | 流空闲超时：chunk 120s 无数据记为超时连接的失败（空回复自动重试已交还上游 #3968，收尾 `5de1cc0d5`） |
| 13 | `5b3a02afb` | fix | 子代理对被注入的消息沉默时通知父代理重新派活（收尾 `5de1cc0d5`：不再自带重试与 nudge） |
| 14 | `6af95e354` | fix | 停滞通知只发给父代理一次 |
| 15 | `7bae9d82e` | fix | 收养无 bootstrap 记录的 kernel venv |
| 16 | `3a95342b7` | fix | 托盘显示 provider 与配置的模型名 |
| 17 | `1acecc76a` | fix | 推理档位一路带到线上 |
| 18 | `722a03c2e` | fix | `/speed` 开关持久化 |
| 19 | `e44911081` | feat | 新增 session clean 清理空会话 |
| 20 | `b0468cd57` | fix | refine 的 passthrough 头改为合并而非整表覆盖 |
| 21 | `355495387` | fix | 多行粘贴在写边界被拆开时仍按粘贴处理 |
| 22 | `ebffeccdb` | feat | agents view 的 Model 列显示 provider / model（列宽规则已由上游 #4036 接管，本行只剩内容格式） |
| 23 | `dd11d85de` | fix | 用户消息回显改为纯文本，不再吞字符 |
| 24 | `c01de6e06` | fix | 清掉 fork 的 CI 门（fmt / clippy / 乱码注释） |
| 25 | `b24ebfd03` | fix | 单独的 `>` 保持字面显示（APPEND 续跑命令） |
| 26 | `c32dd7b18` | fix | RPC 的 lease 探测移出 tokio 运行时 |
| 27 | `3c024c5fd` | fix | RPC 会话接入心跳与阈值自动压缩 |
| 28 | `f6df9050c` | fix | daemon 把 provider 头贴到 `Model` 上给直连接缝用 |
| 29 | `b31cc0973` | fix | refine 请求保持机器可解析，失败时透出 provider 原话 |
| 30 | `ef888fd12` | fix | refine 的空回复在解析错误里点名 |
| 31 | `bb696a5d2` | fix | provider 报错不再被当成空回复重试；refine 不再压低输出预算 |
| 32 | `332cb5d75` | fix | Windows 括号粘贴从按键流重组为一次粘贴 |
| 33 | `7e4d501ba` | feat | ACP：Python cell 按代码首行命名，并上报上下文用量 |
| 34 | `c74a8b559` | feat | TUI：`PASTE_TRACE` 打印粘贴路径上的原始终端事件 |
| 35 | `325b00402` | feat | 按受众（人 / agent）限定可见模型与推理档位：`visibleModels`、`thinkingLevels` |
| 36 | `8fd1dc051` | fix | `/model` picker 接上 `visibleModels.human`（收尾：`f5c092286`、`1843dff47`） |
| 37 | `9ff2d7643` | feat | MCP 调用默认超时：全局设置 `mcpCallTimeoutMs`，server 自带的 `callTimeoutMs` 优先 |
| 38 | `500d29140` | feat | MCP stdio 的 `env` 支持字面值（`{"value": ...}` 与 `--env-value CHILD=VALUE`） |
| 39 | `1cfe1c2ec` | fix | `python_repl` 的 per-call 超时上限跟随会话的 `pythonCellTimeoutMs` |
| 40 | `3296e31c1` | feat | 恢复 Hindsight 记忆（隐藏 hook，见 `docs/hindsight-hook.md`） |
| 41 | `9cb66c286` | feat | TUI transcript 按条显示本地时间戳，`/settings → Display` 可开关（`terminal.showTimestamps`，默认关） |
| 42 | `67b2f1481` | feat | TUI 时间戳格式可调：日期（none / MM/DD / YY/MM/DD / YYYY/MM/DD）与毫秒位数（0–3），前缀宽度随格式 11–26 列；新增 `terminal.timestampDate`、`terminal.timestampMillis` |
| 43 | `43bc2f376` | feat | Ctrl+O 循环新增 minimal 阅读级别：隐藏模型工具卡、后台 shell 完成行与 inbox 到达/提醒通知；对话、`!`/`!!` bash 运行、状态通知、agent 消息、注入 prompt、压缩行照常显示，thinking 仍随 overview 隐藏；级别持久化为 `chatDetail: "minimal"`（收尾 `7c46a84d3`、`1ab285fc9`） |
| 44 | `ad44d8714` | feat | 终端窗口标题：命名会话为 `会话名 · working/done/error/idle`，未命名时为 `prime-agent · 状态`；仅变化时写 OSC 0；名称清洗控制字符与空白，并按显示宽度截到 32 列（收尾 `04a1e81b0`） |
| 45 | `3fca25f6e` | feat | tray 的上下文占用按阈值着色：<75% dim、≥75% warning、≥90% error，自动压缩临近时可见 |
| 46 | `5e6d6000a` | feat | 聊天顶栏四行：会话名 + 花费右对齐 / `project` 项目目录 / `cwd` 终端工作目录（偏离项目目录时标红）/ `branch` 分支；纯读 `.git/HEAD`（支持 worktree/submodule 的 `gitdir:`），无仓库显示 `no git repository`；路径按列宽中截、分支按列宽截断；attach、改名、回合结束刷新，不做 dirty，不启动 `git`（重打 `32b109ed5`，原单行 `⑂branch` 显示已由本补丁取代） |
| 47 | `392e139df` | feat | 输入框 Ctrl+R 反向历史搜索：头部显示 `(reverse-i-search)`query':`，Ctrl+R 循环更旧命中、Backspace 改查询、可打印字符扩展查询、Enter 接受但不提交、Esc/Ctrl+C 恢复草稿；无命中显示 `[no match]` 并保留草稿 |
| 48 | `3da416e04` | feat | `Alt+PageUp` / `Alt+PageDown`（别名 `Alt+[` / `Alt+]`）在用户消息（含 slash 命令回显）间跳转：跟随时先停在最新用户消息，暂停后按当前位置前后走 |
| 49 | `0cc18d466` | feat | `/find <text>` 在已加载转写里大小写不敏感搜索（用户、助手正文、slash 回显、状态行、压缩摘要），跳首个命中并用 toast 显示位置；`Alt+N` / `Alt+P` 循环、`/find` 清空；命中在命令执行时快照 |
| 50 | `24512f1ac` | feat | 完成响铃：`terminal.bellOnTurnEnd`（默认关）开启后，回合以 Done/Error 结束时响一次 BEL；aborted/idle 不响、重复状态不重响；`/settings → Display` 的 “Turn-end bell” 可切换 |

表格按**行为补丁**计数，所以行数与提交数**不相等**：同一个补丁的后续收尾提交
（对已入表补丁的修正、重构、补测试）并入该行的摘要，不另开一行——表末那条就是
这样收了 `8fd1dc051` 之后的两个（`f5c092286` 重构、`1843dff47` 补测试）。想核对
总数时用 `git log --oneline v0.10.3..HEAD` 与表格、`docs:`、`chore: stamp`、
rebase 修复、收尾提交、行尾修复逐项对照；任何一侧新增提交都会让固定算术式过期，
所以这里不写死数字。

补丁之外还有五类提交不进表格：文档提交、版本戳提交、rebase 修复提交
（`da5e0a653`、`dd5d002f8` 改 `pa-tui` 的测试调用点；`e13ef7f54` 修上游 #4044 删掉
的 private-auth 调用、`[profile.dev]` 合并与 agents view 列宽测试；`ef15b4d05` 修
v0.10.5 同步的冲突收尾、Hindsight 的 TLS provider、`assert_is_empty` 新 lint）、
已入表补丁的收尾提交、以及行尾修复提交（`12889e140`）。文档与版本戳提交随 rebase
改写 hash，按标题找（`docs:` / `chore: stamp` 开头）；修复提交是重放破损的修补，
不是本地补丁。

补丁 26–31 是补录条目：它们一直在 `custom` 上（均为 HEAD 的祖先），此前没进清单，
导致 rebase 时没人知道有这几处需要重新应用或验证。其中 `3c024c5fd`（补丁 27）
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
| `custom.33` | 含 `8fd1dc051` 及收尾 |
| `custom.34` | 本次同步到 `c4d420183`。二进制内容变了，所以升号 |
| `custom.35` | 在 `custom.34` 之上加了 MCP 调用超时（`mcpCallTimeoutMs`）、MCP `env` 字面值，以及 `python_repl` 的 per-call 超时上限（补丁 37–39） |
| `custom.36` | 同步上游到 `6aaa3a255`（25 个提交），并把空回复自动重试交还上游 #3968（收尾 `5de1cc0d5`）。二进制内容变了，所以升号 |
| `custom.37` | 顶栏改成「会话名 (会话目录, 终端 cwd)」，无名会话显示 `unnamed`，两个目录不一致时 cwd 标红（补丁 1 收尾 `23827d358`） |
| `custom.38` | 同步上游到 `v0.10.3`（`d649b93cf`，8 个提交）。基线版本从 `0.10.3-beta.5` 变成 `0.10.3`，所以后缀直接是 `0.10.3+custom.38`；二进制内容变了，所以升号 |
| `custom.39` | 同步上游到 `upstream/main` `0bce1417b`（43 个提交）：private-auth 路径被上游 #4044 删除、agents view 列宽交给 #4036、`[profile.dev]` 与上游 #3873 合并。二进制内容变了，所以升号 |
| `custom.40` | 恢复 Hindsight 记忆（隐藏 hook）。二进制内容变了，所以升号 |
| `custom.41` | TUI transcript 按条显示本地时间戳（`terminal.showTimestamps`，默认关）。二进制内容变了，所以升号 |
| `custom.41`（重放） | 同步上游到 `upstream/main` `61d4b4262`（12 个提交，含稳定 tag `v0.10.4`）。基线版本从 `0.10.3` 变成 `0.10.4`，后缀随之变成 `0.10.4+custom.41`；`custom.41` 此前从未构建，所以号不复用冲突 |
| `custom.42` | TUI 时间戳格式可调（日期 4 档、毫秒 0–3 位），前缀宽度随格式变化。二进制内容变了，所以升号 |
| `custom.43` | 同步上游到 `v0.10.5`（`783acb5d7`，40 个提交），并把 Ctrl+O 的 minimal 阅读级别并入。上游依赖大升级（`base64` 0.23、`fancy-regex` 0.20、`rsa` 0.10、`sha2` 0.11、`getrandom` 0.4、`reqwest` 0.13），二进制内容变了，所以升号 |
| `custom.44` | 同步上游到 `upstream/main` `bb21eae7b`（1 个提交，url 2.5.9 / RUSTSEC-2026-0592），131 个本地提交重放无冲突。二进制内容随依赖变化，所以升号 |
| `custom.45` | TUI 体验四连：终端标题、上下文占用按阈值着色、顶栏 git 分支、Ctrl+R 反向历史搜索（补丁 44–47）。二进制内容变了，所以升号 |
| `custom.46` | TUI 体验两连：用户消息跳转、`/find` 转写搜索（补丁 48–49）。二进制内容变了，所以升号 |
| `custom.47` | 终端标题压缩为「会话名 · 状态」，长名称截到 32 列（补丁 44 收尾）。二进制内容变了，所以升号 |
| `custom.48` | 完成响铃开关（补丁 50）。二进制内容变了，所以升号 |

stamp 提交写于哪个基线，rebase 后正文里的版本号可能与实际写入的不一致；那种提交
正文不追改（改写历史只为一个字符串不值当）。唯一例外是**标题行**：`v0.10.3` 那次
基线版本变了（`0.10.3-beta.5` → `0.10.3`），把 `chore: stamp this fork build as
0.10.3-beta.5+custom.N` 的标题统一改成 `0.10.3+custom.N`，否则标题与实际写入的号
对不上；更早的 `0.10.2+custom.N` 标题属于它当时写下的号，保持原样。

## 下次同步注意

**同步记录（`783acb5d7` → `bb21eae7b` 那次，0 个冲突）**

上游只有一个依赖提交（url 2.5.9，RUSTSEC-2026-0592，仅 `Cargo.lock`）。131 个本地
提交全部重放成功，无冲突；表内 hash 与收尾 hash 已按标题回填，行 43 的摘要改为
「只隐藏模型工具卡与后台 shell 完成行」。二进制内容随依赖变化，stamp 升到
`0.10.5+custom.44`。

**冲突化解（`61d4b4262` → `v0.10.5`／`783acb5d7` 那次，40 个提交）**

1. 127 个提交里 12 处冲突，处理原则同上（保留上游新结构、本地补丁作增量重接）：
   - `session_engine/admission.rs`：上游把 busy 分支收进 `if busy { ... return Queued }`
     + `} else { ... }` 的显式分叉（并在 idle 分支加了 race 回退）。turnHook 的
     `note_turn_hook` / `discard_turn_hook_note` 必须接进 idle 分支；**不要**保留旧的
     平铺写法，否则会重复出一段 `else if let Err(error)`，编译器报 `unexpected closing
     delimiter`。
   - `session_engine/engine.rs`：上游把 `mcp_generic_servers` 改名成
     `generic_mcp_servers`；turnHook 的 `turn_hook_settings` 与 stopHook 的
     `stop_hook_settings` 两个捕获都要留住。
   - `kernel/bootstrap/venv/tests.rs`、`pa-daemon/.../model_resolution.rs`、
     `pa-core/src/tools/ipython.rs`、`skills/mcp/SKILL.md`：两侧各加了一个测试/条目，
     取并集（把两个函数都留着，别只取一侧）。
   - `pa-tui/src/chat.rs`：本分支的 `wrap_user_text`（用户消息按字面渲染）赢；上游的
     `user_block_preserves_markdown_styles_on_its_background` 测试随之删除，换成
     `user_block_never_interprets_markdown`。
2. **`Cargo.toml` / `Cargo.lock` 会整文件冲突**（`Cargo.toml` 甚至报 `161/161` 行全冲
   突）。原因是历史上某个 stamp 提交用 PowerShell `WriteAllLines` 写成了 CRLF，之后
   每个提交都带着 CRLF 版本，与本分支的 LF 侧整文件不匹配。解法：`git checkout
   --theirs -- Cargo.toml Cargo.lock`（取上游的 LF 内容），再把本地的 `uuid` v5 feature
   加回去，然后用 `sed -i "s/\r$//"` 之类的 LF 归一化写回。**不要**用 PowerShell 的
   `Set-Content`/`WriteAllLines` 改这两个文件。
3. 收尾时用 `git filter-branch --tree-filter` 把 `783acb5d7..HEAD` 的
   `Cargo.toml`/`Cargo.lock` 行尾统一成 LF（否则 83 个提交带着 CRLF 噪音）。跑完之后
   `git diff-tree 原HEAD 新HEAD` 必须为空。
4. `reqwest` 升到 0.13 且 workspace 统一用 `rustls-no-provider`：任何**直接**
   `reqwest::Client::builder()` 都会在首次请求时 panic（"No rustls crypto provider is
   configured"）。新增 HTTP 客户端一律走 `pa_ai::utils::tls::http_client_builder()`。
   本次就是 `hindsight/client.rs` 踩了这个坑。
5. clippy 1.99 新增 `assert_is_empty`（pedantic），本分支补丁里新增的
   `assert!(x.is_empty());` 必须带消息或改 `assert_eq!`。上游已有 228 处同类告警，属既有
   噪音，不要顺手全改。
6. 版本号：基线从 `0.10.4` 变成 `0.10.5`，后缀随之是 `0.10.5+custom.43`。

**冲突化解（`0bce1417b` → `61d4b4262` 那次，12 个提交）**

1. 121 个提交里有 6 个冲突点，全部按「保留上游新结构、本地补丁作增量重接」处理：
   - `session_engine/mod.rs` 三处初始化块（turnHook、retry nudge、empty-reply 交还）：
     上游把 `compaction_flight` / `refinements_in_flight` / `refinement_cancel` 改成了
     `Arc<...>`，三处都取上游字段、只把本地的 `turn_hook: None` / `retry_nudge` 接回去。
   - `session_engine/tests.rs` 整文件冲突：两边函数集合完全一致，只有 digest 恢复测试
     是上游新语义（改成 append 而非 replace），取上游侧。
   - `Cargo.toml` / `Cargo.lock`（10 处）：`stamp this fork build as 0.10.2+custom.25`
     只冲突版本行，回填该历史 stamp 的字面版本。
2. 上游给 `AgentEvent::AgentEnd` 加了 `aborted` 字段：`session_engine/wiring.rs` 的
   stopHook 监听只读 `messages`，模式改成 `{ messages, .. }`，否则 E0027。
3. 对 `session_engine/tests.rs` 取 `--ours` 会连带丢掉
   `a_resolved_model_bill_survives_its_callers_cancellation` 的闭合大括号（与上一次
   rebase 同一个坑）。取完之后必须 `cargo check --all-targets`，那个提交正文里写明了
   这一处。
4. 版本号：基线从 `0.10.3` 变成 `0.10.4`，后缀随之是 `0.10.4+custom.41`（`custom.41`
   此前从未构建，号不复用冲突）。

**冲突化解（`v0.10.3` → `0bce1417b` 那次，43 个提交）**

1. 上游 #4044 删掉了 `models/private_auth.rs`、`is_private_prime_inference_model` 和
   `ModelRegistry::load_private_authorization_from_cache()`，而本分支有 8 处 registry
   构造在调它：`models/registry.rs`、`session_engine/runtime_wiring.rs`、
   `pa-cli/src/print_rlm.rs`、`agent_engine/turn/run_once.rs`、`image_route.rs`、
   `model_catalog.rs`、`rpc/model_commands.rs`、`state_getters/usage.rs`。解法是删调用、
   留 `apply_visibility(...)`（`visibleModels` 的执行点），并把被降级成 `let registry`
   的三处改回 `let mut registry`。
2. 上游 #4036 重写了 agents view 的列宽规则，本分支的 `.min(44)` 上限连同它的测试
   `the_model_column_fits_a_qualified_provider_model_pair` 一起删除——上游自己的
   `the_model_column_fits_the_widest_model_name_and_gives_way_first` 覆盖同一行为。
3. 上游 #3873 新增了自己的 `[profile.dev]`（`debug = "line-tables-only"`）与
   `[profile.dev.package."*"]`，本分支的 `incremental = false` 必须并进那张表：两份
   `[profile.dev]` 是 TOML duplicate key，cargo 会直接拒绝加载。
4. 上游 #3990 删掉 busy-kernel 的 Wait/Kill 对话框，连带删了
   `IpythonToolOptions::ui`：本分支 python_repl 超时上限的测试里那行 `ui: None,` 要去掉。
5. `Cargo.toml` / `Cargo.lock` 从这一轮起在索引里是 **LF**（之前是 CRLF，与上游合并时
   必然整文件冲突）。更早的提交里仍是 CRLF，但此后不再产生整文件冲突。

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
22/33/35，是 rebase 高发区，且历史上被误写成过 CRLF（`12889e140` 只转回 LF）；
同步前用 `git diff --ignore-cr-at-eol` 确认没有整文件噪声。

**两条不当门禁、也不当故障的现象**

- `cargo clippy -D warnings` 在基线上同样失败：本机工具链比上游 CI 新，新 lint 会
  命中上游代码（`vendor/crossterm`、`pa-telemetry`、`pa-ai` 生成文件）。用
  `cargo fmt --check` + `cargo check --all-targets`。
- `list-models` 打印完不退出是既有行为，与版本无关；`-p` 也观察到过无输出挂起
  （20 秒 stdout/stderr 全空，原因未查清）。别用它们做验收。
- 上游新加的三条 daemon 测试在本机 Windows 上会挂：
  `setting_switches::tests::set_auto_compaction_reaches_the_built_session` 与
  `session_navigation::tests::a_replaced_session_continues_its_goal` 单跑通过、全量并发
  时挂住；`worker::tests::goal_tests::goal_turn_end_loop_runs_to_completion` 单跑也挂。
  本分支没碰那些代码，是上游测试在本机的环境问题——跑 `cargo test -p pa-daemon --lib`
  时用 `--skip` 排除它们，别当成回归。

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
  3 秒等待、请求级 nudge 与可见警告行随之删除（收尾 `5de1cc0d5`）。留下的只是上游
  不覆盖的那半：子代理对**被注入**的消息沉默时（`owes_answer == false`）通知父代理
  重新派活，见补丁 13。
- **agents view 的 Model 列宽**：上游 `2af2ad0c2`（#4036）按最宽模型名计算列宽并给名称
  列保底，本分支原来的 44 格上限与对应测试随之删除（收尾 `e13ef7f54`）。补丁 22 只剩
  「列里显示 `provider / model`」这点内容格式。
- **私有模型鉴权**：上游 `fa6cda7f9`（#4044）删掉了重复的 private-auth 路径，私有模型
  改由 scoped Prime Inference snapshot 授权，`load_private_authorization_from_cache()`
  整个消失（收尾 `e13ef7f54`）。本分支的 `visibleModels`（`apply_visibility`）仍在，
  但它不再依赖那个缓存。
