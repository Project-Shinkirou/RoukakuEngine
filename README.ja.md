# RoukakuEngine

ネイティブ 2D 作品とビジュアルノベルを制作・実行するエンジンです。

[English](README.md) · [简体中文](README.zh-CN.md) · 日本語

> **状況：開発休止中。** RoukakuEngine にはバイナリのリリースがありません。ソースとドキュメントは公開したままで、下の手順でソースからビルドできます。[プロジェクトの状況](https://project-shinkirou.github.io/ja/status/)のページも参照してください。

**ポータル：**[RoukakuEngine](https://project-shinkirou.github.io/ja/roukaku/) · [ドキュメント](https://project-shinkirou.github.io/ja/roukaku/docs/) · [クイックスタート](https://project-shinkirou.github.io/ja/roukaku/docs/quick-start/)

## 構成

- **共通ランタイム層**：シーン、タスク、クロック、入力、テキスト、メディア、リソース管理。
- **RoukakuVN**：ビジュアルノベル用のモジュールです。作者が書いた `.roukaku` ソースを Cook でコンパイルし、共通の `VnSession` で実行します。
- **RoukakuPlayer**：作品の実行ホストです。パッケージ化されたデータを読み込み、ウィンドウ、デバイス、プラットフォームイベントを管理します。
- **RoukakuEditor**：テキスト、シナリオグラフ、タイムライン、Details、単独の Player プレビューを備えています。パネルでの編集は同じソースに書き戻されます。
- **`roukaku` CLI**：スクリプトと UI のチェック、Cook、パッケージのビルド、プラットフォーム向けバンドルを担当します。

設定でプラットフォームが宣言されていても、そのプラットフォームでのウィンドウ、入力、音声、セーブ／ロードが実機で検証済みとは限りません。

## はじめる

Rust（ツールチェーンは `rust-toolchain.toml` で選択されます）とネイティブの C/C++ ツールチェーンをインストールし、[ビルド手順](docs/ja/build.md)に従ってネイティブ依存関係を準備してから、リポジトリのルートで次を実行します。

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

CLI を Cargo の bin ディレクトリにインストールします。

```sh
cargo install --locked --path Engine/Source/Programs/roukaku-cli --bin roukaku
roukaku --help
```

公開サンプルの Cook、パッケージ化、Player での起動手順は[クイックスタート](docs/ja/quick-start.md)を参照してください。

## ドキュメント

| | 日本語 | English | 简体中文 |
|---|---|---|---|
| 概要 | [index](docs/ja/index.md) | [index](docs/en/index.md) | [index](docs/zh-cn/index.md) |
| クイックスタート | [quick-start](docs/ja/quick-start.md) | [quick-start](docs/en/quick-start.md) | [quick-start](docs/zh-cn/quick-start.md) |
| ビルド | [build](docs/ja/build.md) | [build](docs/en/build.md) | [build](docs/zh-cn/build.md) |
| アーキテクチャ | [architecture](docs/ja/architecture.md) | [architecture](docs/en/architecture.md) | [architecture](docs/zh-cn/architecture.md) |
| Editor | [editor](docs/ja/editor.md) | [editor](docs/en/editor.md) | [editor](docs/zh-cn/editor.md) |
| VN 制作 | [vn-authoring](docs/ja/vn-authoring.md) | [vn-authoring](docs/en/vn-authoring.md) | [vn-authoring](docs/zh-cn/vn-authoring.md) |

[コントリビュート](CONTRIBUTING.md) · [由来と帰属](ORIGINS.md) · [問題の報告](https://github.com/Project-Shinkirou/RoukakuEngine/issues)

## ShinkiEMU との関係

[ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) は既存のビジュアルノベルエンジンをネイティブに動かすための独立した製品で、RoukakuEngine の共有ライブラリを固定した Git リビジョンで利用しています。RoukakuEngine は ShinkiEMU に依存しません。

## ライセンス

独自のコードは [MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) のいずれかを選んで利用できます。サードパーティのコードと素材は、それぞれのライセンスと帰属表示に従います。
