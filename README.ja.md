<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/readme-banner-dark.png">
  <img src="docs/assets/readme-banner-light.png" alt="RoukakuEngine — Rust 製ネイティブ 2D ビジュアルノベルエンジン・プレイヤー・エディター">
</picture>

<div align="center">

# RoukakuEngine

**Rust 製のネイティブ 2D ビジュアルノベルエンジン・プレイヤー・エディター・共有ランタイム。**

[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-f59e0b)](LICENSE-MIT)
[![Status](https://img.shields.io/badge/status-ソース公開中%20%C2%B7%20開発一時停止-64748b)](https://project-shinkirou.github.io/ja/status/)

[English](README.md) · [简体中文](README.zh-CN.md) · 日本語

[ポータル](https://project-shinkirou.github.io/ja/roukaku/) · [ドキュメント](https://project-shinkirou.github.io/ja/roukaku/docs/) · [クイックスタート](https://project-shinkirou.github.io/ja/roukaku/docs/quick-start/) · [Project-Shinkirou](https://github.com/Project-Shinkirou)

</div>

---

> [!NOTE]
> **プロジェクト状況：機能開発は一時停止中です。** バイナリ配布はありませんが、ソースコードとドキュメントは継続公開されておりソースからビルド可能です。共有ランタイムライブラリは姉妹プロジェクト [ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) で活用されています。詳細は[プロジェクト状況](https://project-shinkirou.github.io/ja/status/)をご覧ください。

## 構成要素

| コンポーネント | 役割 |
|---|---|
| **共有ランタイム** | シーン、タスク、クロック、入力、テキスト描画、メディア、リソース管理 |
| **RoukakuVN** | `.roukaku` スクリプトをクックし `VnSession` で実行するビジュアルノベルモジュール |
| **RoukakuPlayer** | パッケージ済み作品を読み込みウィンドウやデバイスイベントを管理するホスト |
| **RoukakuEditor** | スクリプト・ストーリグラフ・タイムライン・詳細パネル・独立プレビューを統合 |
| **`roukaku` CLI** | スクリプト／UI 検証、クック、パッケージ構築、プラットフォームバンドル生成 |

## ソースからのビルド

`rust-toolchain.toml` の Rust とネイティブ C/C++ ツールチェーンを用意し、[ビルドガイド](docs/ja/build.md)に沿って依存関係を準備したあとリポジトリルートで実行します：

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

CLI をインストールする場合：

```sh
cargo install --locked --path Engine/Source/Programs/roukaku-cli --bin roukaku
roukaku --help
```

公開サンプルのクック・パッケージ化・実行手順は[クイックスタート](docs/ja/quick-start.md)をご覧ください。

## ドキュメント

| トピック | 日本語 | English | 简体中文 |
|---|---|---|---|
| 概要 | [概要](docs/ja/index.md) | [index](docs/en/index.md) | [概览](docs/zh-cn/index.md) |
| クイックスタート | [クイックスタート](docs/ja/quick-start.md) | [quick-start](docs/en/quick-start.md) | [快速上手](docs/zh-cn/quick-start.md) |
| ビルド | [ビルド](docs/ja/build.md) | [build](docs/en/build.md) | [构建](docs/zh-cn/build.md) |
| アーキテクチャ | [アーキテクチャ](docs/ja/architecture.md) | [architecture](docs/en/architecture.md) | [架构](docs/zh-cn/architecture.md) |
| エディター | [エディター](docs/ja/editor.md) | [editor](docs/en/editor.md) | [编辑器](docs/zh-cn/editor.md) |
| VN 制作 | [VN 制作](docs/ja/vn-authoring.md) | [vn-authoring](docs/en/vn-authoring.md) | [VN 创作](docs/zh-cn/vn-authoring.md) |

[コントリビュート](CONTRIBUTING.md) · [出典と帰属](ORIGINS.md) · [Issues](https://github.com/Project-Shinkirou/RoukakuEngine/issues)

## ShinkiEMU との関係

[ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) は既存のビジュアルノベルエンジンを実行する姉妹製品であり、固定コミットの RoukakuEngine 共有ライブラリを利用しています。RoukakuEngine 自体は ShinkiEMU に依存しません。

## ライセンス

プロジェクト独自コードは [MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) のデュアルライセンスです。サードパーティコード・素材は元のライセンスと帰属を維持します。
