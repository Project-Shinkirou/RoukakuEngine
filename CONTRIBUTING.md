---
title: Contributing / 参与贡献
description: Branches, checks, behavior tests, and source attribution.
---

# 参与贡献 / Contributing

## 中文

从最新 `main` 创建主题分支，提交范围清楚的修改并发起 PR。说明行为变化、相关问题、实际运行的检查和未覆盖的环境。接口或格式修改同步更新调用方、行为回归和中英文文档。

提交 PR 前，在仓库根目录运行受影响 workspace 的检查；同时涉及两者时运行 `--workspace all`：

```sh
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

开发中可定向运行，例如 `cargo xtask test --workspace engine -p roukaku-runtime`。纯文档修改运行 `cargo xtask docs`。原生依赖和平台要求见[构建](docs/zh-cn/build.md)。

测试应证明可观察行为：格式往返、损坏或超限输入拒绝、取消和关闭、旧结果失效、读档失败保留当前状态、文件不覆盖，以及真实消费者的输出。不要添加只固定文案、源码字符串、路径拼写或常量值的测试，也不要用 mock 调用次数代替真实行为。不能通过删除失败回归或关闭真实后端让检查通过。

引入代码、字体、图片、音视频或其他素材时，保留许可证，注明来源、作者及必要的 revision/hash，确认分发授权。不要将根许可证误当作第三方素材许可。来源记录遵循 [ORIGINS.md](ORIGINS.md) 和相关素材的归属文件。

**禁止提交商业游戏数据、正文、密钥、认证信息或存档。** 使用公开、获授权的 fixture；本地私有输入和构建产物放在 ignored 目录。不要覆盖用户数据，也不要在日志或 PR 中暴露私有路径和内容。

## English

Create a topic branch from current `main` and open a focused PR. Describe behavior changes, related issues, checks actually run, and environments not covered. Update callers, behavior regressions, and both documentation languages when changing an interface or format.

Before opening a PR, run `cargo xtask check --workspace engine` or `cargo xtask check --workspace editor` for the affected workspace; use `--workspace all` when both change. During development, target a package with, for example, `cargo xtask test --workspace engine -p roukaku-runtime`. For documentation-only changes, run `cargo xtask docs`. See [Build](docs/en/build.md) for prerequisites.

Test observable behavior: round-trips, corrupt and oversized input rejection, cancellation and shutdown, stale-result rejection, failed restores preserving live state, no-overwrite guarantees, and real consumer output. Avoid wording, source-string, spelling, constant-value, and mock-forwarding tests. Do not remove a failing behavior regression or disable a real backend to pass checks.

Preserve licenses and attribution for code and assets, including fonts, images, audio, and video. Record sources, authors, and relevant revisions/hashes; confirm redistribution rights. The root code license does not relicense third-party assets. Follow [ORIGINS.md](ORIGINS.md) and asset-specific attribution records.

**Never commit commercial game data, game text, keys, credentials, or saves.** Use public, authorized fixtures. Keep private inputs and generated output in ignored directories, preserve user files, and omit private paths and content from logs and PRs.
