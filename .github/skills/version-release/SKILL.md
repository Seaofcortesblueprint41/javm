---
name: version-release
description: "用于版本升级与发布流程。当用户提到：升级小版本、升级中版本、升级大版本、升级补丁版本、升级次版本、升级主版本、升级 alpha 版本、升级 beta 版本、升级预发布版本、升级发布、发版、发布版本、打 tag、推送远程、生成 release 日志、发布日志时使用。对小/中/大版本分别映射到 patch/minor/major。若用户提到 alpha、beta、预发布版本，则允许直接指定完整版本号，例如 0.1.5-alpha.1、0.1.5-beta.1。若用户只说升级发布或发版，在支持多项选择时先让用户选择 patch、minor、major 或直接输入完整预发布版本号，然后完成版本升级、生成发布日志、git commit、git tag、push 分支和 push tag。"
---

# 版本升级与发布

这个 skill 用于在当前仓库中完成“升级版本 -> 生成发布日志 -> 提交 -> 打 tag -> 推送远程”的完整流程。

## 触发规则

- 用户说“升级小版本”：等价于执行 patch 升级。
- 用户说“升级中版本”或“升级 minor 版本”：等价于执行 minor 升级。
- 用户说“升级大版本”或“升级 major 版本”：等价于执行 major 升级。
- 用户说“升级 alpha 版本”“升级 beta 版本”“升级预发布版本”：优先让用户给出完整版本号，或直接按用户给出的完整版本号执行，例如 `0.1.5-alpha.1`、`0.1.5-beta.1`。
- 用户只说“升级发布”“发版”“发布版本”：如果可以交互提问，先让用户在 patch、minor、major 中选择一个，再继续。

## 仓库规则

- 前端生态必须使用 bun，不要使用 npm、yarn 或 pnpm。
- 版本升级必须使用 `bun run vb -- <patch|minor|major>` 或 `bun run version:bump -- <patch|minor|major>`。
- 如果需要预发布版本，允许直接指定完整版本号，例如：`bun run vb -- 0.1.5-alpha.1`、`bun run vb -- 0.1.5-beta.1`。
- 提交范围采集脚本统一使用 `bun run release:collect -- v<version>`。
- 禁止手动编辑 package.json、src-tauri/tauri.conf.json、src-tauri/Cargo.toml 中的版本号。
- 发布日志文件使用 `docs/releases/v<version>.md` 命名，并与版本升级提交一起进入仓库。
- 发布日志模板统一参考 `docs/releases/TEMPLATE.md`。
- 发布日志的总结范围必须是“上一个已发布 tag”到“即将发布的 tag”之间的全部提交，不是只看本次版本升级提交。
- 版本号支持 semver 预发布后缀，例如：`0.1.5-alpha`、`0.1.5-alpha.1`、`0.1.5-beta`、`0.1.5-beta.1`；对应 tag 形如 `v0.1.5-beta.1`。
- 提交、tag、推送前先检查工作区状态，避免把用户未确认的改动带上去。

## 执行流程

1. 运行 `git status --short --branch` 检查当前分支与未提交改动。
2. 运行 `git remote -v`、`git fetch --tags origin`、`git status --short --branch`，确认：
   当前仓库存在 origin。
   本地 tag 已和远端同步，避免用旧 tag 生成日志。
   当前分支没有处于冲突态。
3. 如果 `git status --short --branch` 显示分支落后远端，或存在未解决冲突：
   停止发布流程，先告知用户同步或解决冲突；不要继续 bump/tag/push。
4. 如果用户没有明确升级级别，并且请求是“升级发布/发版/发布版本”这类泛化表述：
   使用提问工具让用户从 patch、minor、major 中选择一个，或直接输入完整预发布版本号，例如 `0.1.5-beta.1`。
5. 如果工作区存在未提交改动：
   先展示将被提交的范围，并询问是否一起提交这些改动；不要擅自提交不属于本次发布的改动。
6. 如果本次发布应包含的功能改动仍停留在未提交状态，先明确提示用户：未提交内容不会出现在“上一个 tag 到当前提交”的历史范围里；若要纳入发布日志，必须先提交这些改动。
7. 运行 `bun run vb -- patch|minor|major` 完成版本升级。
8. 从 package.json 读取新版本号，并确定新 tag：`v<version>`。
9. 在创建新 tag 前，先查找上一个已发布 tag，优先使用语义化版本 tag，例如：
   `git tag --sort=-version:refname`
   过滤出符合 semver 的 tag，例如 `v0.1.5`、`v0.1.5-alpha.1`、`v0.1.5-beta.1`，并排除即将发布的 `v<version>`。
10. 使用 `bun run release:collect -- v<version>` 生成发布上下文采集文件，默认输出到 `.release-context/v<version>.md`。
11. 采集发布日志素材时，范围必须覆盖：`<previous-tag>..HEAD`，如果没有上一个 tag，则回退为当前仓库全部历史或用户指定范围。
12. 采集提交信息时不要只取标题，必须拿到每条提交的完整信息，至少包含：hash、标题、正文、作者、涉及文件、变更统计。
13. 如果采集脚本提示“当前范围内没有提交记录”：
   默认停止发布流程，并明确向用户确认是否仍要继续发布这个版本。
14. 如果提交正文为空、标题过于笼统或存在大量 `chore` / `fix` / `update` 这类低信息提交：
   应继续结合 `git show --stat --summary <commit>`、相关 diff、关键文件变更说明补足上下文后再总结，避免日志失真。
15. 发布日志内容使用当前对话中的 AI 基于 `.release-context/v<version>.md` 与 `docs/releases/TEMPLATE.md` 进行中文归纳，要求：
   标题简洁，适合直接放进 GitHub Release。
   不要只根据提交标题总结，要综合标题和正文。
   优先总结用户可感知变化，其次再写构建、工程、脚本调整。
   使用 Markdown，建议包含：`## 版本概览`、`## 主要更新`、`## 使用说明` 或 `## 风险与说明`。
   不要编造不存在的功能。
16. 将归纳后的内容写入发布日志文件：`docs/releases/v<version>.md`。
17. 写入后再次检查文件内容，确认不是空文件，且标题中的版本号与 `v<version>` 一致。
18. `.release-context/v<version>.md` 只作为临时上下文文件使用，不要加入本次提交。
19. 运行 `git status --short --branch` 与必要的 `git diff`，确认升级涉及的文件、发布日志文件与其他准备提交的改动。
20. 在 `git commit` 和 `git tag` 之前，向用户展示：
   发布日志文件的摘要或全文预览。
   本次将被提交的文件列表。
   上一个 tag 与本次统计范围。
   如果采集结果为 0 条提交，必须再次提示用户这是空版本发布。
   如果用户发现总结范围或内容不对，先修正日志，再继续。
21. 从 package.json 读取新版本号，生成：
   提交信息：`chore: 发布 v<version>`
   标签名：`v<version>`
22. 检查 tag 是否已存在：`git tag -l v<version>`。
23. 执行提交与打 tag：
   `git add <确认要提交的文件>`
   `git commit -m "chore: 发布 v<version>"`
   `git tag -a v<version> -m "v<version>"`
24. 推送到远端：
   先推送当前分支到 origin，再推送对应 tag 到 origin。
25. 最后再次运行 `git status --short --branch` 确认工作区状态。

## 安全要求

- 不要使用交互式 git 命令。
- 不要执行破坏性命令，例如 `git reset --hard`。
- 如果远端推送失败、tag 已存在、工作区有冲突，停止并把阻塞点明确告诉用户。
- 如果用户已经明确要求“上传代码”，默认理解为 push 到已配置的 origin。
- 如果发布日志文件缺失，不要假装已经生成；应明确创建并让其进入本次提交。
- 如果找不到上一个 tag，需要明确告知当前使用的回退范围，不要默默缩小总结范围。
- 如果 `git fetch --tags origin` 失败、origin 不存在、当前分支落后远端，停止流程并解释原因。
- 如果生成的发布日志与实际版本号不一致，先修正日志文件，再继续提交和打 tag。
- 不要把 `.release-context/` 目录中的临时采集文件提交到仓库。
- 如果发布范围内没有提交记录，默认不要继续发布；除非用户明确确认要发布空版本。

## 回复要求

- 用中文汇报结果。
- 结果中应包含：
   上一个 tag
   提交统计范围
  最终版本号
  commit hash
  tag 名称
   发布日志文件路径
  是否已成功 push 分支与 tag
- 如果推送输出里出现额外警告，例如远端安全告警，可以简短附带说明，但不要喧宾夺主。