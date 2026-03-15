# Release 日志

这个目录用于存放每个发布版本对应的 GitHub Release 正文。

命名规则：

- 使用 `v<version>.md`
- 例如 `v0.1.5.md`
- 也支持预发布版本，例如 `v0.1.5-alpha.1.md`、`v0.1.5-beta.1.md`

发布流程建议：

1. 先升级版本号。
2. 运行 `bun run release:collect -- v<version>`，生成 `.release-context/v<version>.md`。
3. 这个上下文文件会自动收集从上一个 tag 到当前 HEAD 的全部提交，并包含标题、正文、作者、涉及文件和变更统计。
4. 参考 `docs/releases/TEMPLATE.md`，由当前会话中的 AI 生成正式版本日志文件 `docs/releases/v<version>.md`。
5. 将正式版本日志文件与版本升级一起提交。
6. 打 tag 并推送。
7. GitHub Actions 在发版时直接读取该文件内容作为 Release 正文。

推荐采集方式：

- 默认使用 `bun run release:collect -- v<version>`。
- 如果需要手动指定上一个 tag，可使用 `bun run release:collect -- v<version> --previous-tag v<old-version>`。
- `<version>` 支持标准 semver 预发布后缀，例如 `0.1.5-alpha.1`、`0.1.5-beta.1`。
- 如果提交正文里已经写了变更原因、细节或兼容性说明，AI 总结时应一并纳入，而不是只看标题。
- `.release-context/` 是临时目录，不应提交到仓库。
- 如果采集结果显示 0 条提交，默认不建议继续发版；除非你明确就是要发布空版本。