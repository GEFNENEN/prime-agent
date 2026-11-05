# 补丁清单（实时）

本文件只描述 **当前 `custom` 分支相对上游基线的补丁**，以 HEAD 为准。补丁被移除
或合并进上游后，对应条目直接删除，不保留历史（历史见 `git log main..custom`）。

每条的原理、改动文件与验证记录都在提交正文里（`git log -1 <hash>`），这里只留
一行说清它做了什么。

- 基线：`v0.10.3-beta.5`（tag-only；同步点 `c4d420183`）
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
| 1 | `92f030245` | fix | 顶栏始终显示会话工作目录 |
| 2 | `af188f6ba` | fix | 内核忙时等待更久（5s → 30s） |
| 3 | `c77b469d5` | fix | 从 agent 主题目录解析自定义主题 |
| 4 | `a538bfb99` | fix | 分离进程不再弹出控制台窗口 |
| 5 | `4345c753c` | feat | 每个被接纳的 turn 前运行 turnHook |
| 6 | `dc5ab4c53` | feat | 运行结束后通过 stopHook 写回 |
| 7 | `f225341d7` | fix | 在运行结束前同步 spawn stopHook |
| 8 | `898017e1b` | feat | 用 updateGuard 拦截自更新 |
| 9 | `d8d183a9f` | fix | Windows 剪贴板走 UTF-8 控制台 |
| 10 | `e0056aed5` | fix | 所有多行粘贴折叠为标记 |
| 11 | `f9fb71b2c` | feat | 新增 clean-kernel-state 命令 |
| 12 | `5d45e9818` | fix | 空回复自动重试 + 流空闲超时 |
| 13 | `3a0db13da` | fix | 停滞的子代理重新派活 + 重试提示按回合隔离 |
| 14 | `08fe52593` | fix | 停滞通知只发给父代理一次 |
| 15 | `12f0c1163` | fix | 收养无 bootstrap 记录的 kernel venv |
| 16 | `5a295d376` | fix | 空回复重试前固定等 3 秒 |
| 17 | `390a76795` | fix | 托盘显示 provider 与配置的模型名 |
| 18 | `b560a81ed` | fix | 推理档位一路带到线上 |
| 19 | `5072cbc9f` | fix | `/speed` 开关持久化 |
| 20 | `7d95060b2` | feat | 新增 session clean 清理空会话 |
| 21 | `c3c9270a9` | fix | refine 的 passthrough 头改为合并而非整表覆盖 |
| 22 | `f5a50b0a6` | fix | 多行粘贴在写边界被拆开时仍按粘贴处理 |
| 23 | `6bf409822` | feat | agents view 的 Model 列显示 provider / model |
| 24 | `d54c4a017` | fix | 用户消息回显改为纯文本，不再吞字符 |
| 25 | `2eaf63735` | fix | 清掉 fork 的 CI 门（fmt / clippy / 乱码注释） |
| 26 | `43c980002` | fix | 单独的 `>` 保持字面显示（APPEND 续跑命令） |
| 27 | `9ffd5839b` | fix | RPC 的 lease 探测移出 tokio 运行时 |
| 28 | `144a81bf2` | fix | RPC 会话接入心跳与阈值自动压缩 |
| 29 | `ac5e84ff8` | fix | daemon 把 provider 头贴到 `Model` 上给直连接缝用 |
| 30 | `f397ea635` | fix | refine 请求保持机器可解析，失败时透出 provider 原话 |
| 31 | `a773d788e` | fix | refine 的空回复在解析错误里点名 |
| 32 | `e5af49a7d` | fix | provider 报错不再被当成空回复重试；refine 不再压低输出预算 |
| 33 | `fba3b07e7` | fix | Windows 括号粘贴从按键流重组为一次粘贴 |
| 34 | `4080ecb48` | feat | ACP：Python cell 按代码首行命名，并上报上下文用量 |
| 35 | `d724f0d93` | feat | TUI：`PASTE_TRACE` 打印粘贴路径上的原始终端事件 |
| 36 | `7fbbf4003` | feat | 按受众（人 / agent）限定可见模型与推理档位：`visibleModels`、`thinkingLevels` |
| 37 | `9df0655b4` | fix | `/model` picker 接上 `visibleModels.human`（收尾：`e3a34b38b`、`0855cfc01`） |

表格按**行为补丁**计数，所以行数与提交数**不相等**：同一个补丁的后续收尾提交
（对已入表补丁的修正、重构、补测试）并入该行的摘要，不另开一行——表末那条就是
这样收了 `9df0655b4` 之后的两个（`e3a34b38b` 重构、`0855cfc01` 补测试）。想核对
总数时用 `git log --oneline v0.10.3-beta.5..HEAD` 与表格、`docs:`、`chore: stamp`、
rebase 修复、收尾提交、行尾修复逐项对照；任何一侧新增提交都会让固定算术式过期，
所以这里不写死数字。

补丁之外还有五类提交不进表格：文档提交、版本戳提交、2 个 rebase 修复提交
（`3d8daa6b7`、`50d5c969a`，改 `pa-tui` 的测试调用点）、已入表补丁的收尾提交、
以及行尾修复提交（`db92dabc0`）。文档与版本戳提交随 rebase 改写 hash，按标题找
（`docs:` / `chore: stamp` 开头）；修复提交是重放破损的修补，不是本地补丁。

补丁 26–31 是补录条目：它们一直在 `custom` 上（均为 HEAD 的祖先），此前没进清单，
导致 rebase 时没人知道有这几处需要重新应用或验证。其中 `144a81bf2`（补丁 28）
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
| `custom.33` | 含 `9df0655b4` 及收尾 |
| `custom.34` | 本次同步到 `c4d420183`。二进制内容变了，所以升号 |

stamp 提交写于哪个基线，rebase 后正文里的版本号可能与实际写入的不一致；那种提交
正文不追改（改写历史只为一个字符串不值当）。

## 下次同步注意

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
22/33/35，是 rebase 高发区，且历史上被误写成过 CRLF（`db92dabc0` 只转回 LF）；
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
