---
title: クイックスタート
description: CLI をビルドし、公開 NativeVN サンプルを Cook、パッケージ化して Player を起動します。
sidebar: { order: 2 }
---

# クイックスタート

以下のコマンドはリポジトリのルートで実行します。先に[ビルド](build.md)に従って Rust、C/C++ ツールチェーン、ネイティブ依存関係を用意し、リポジトリの Git LFS アセットを取得してください。サンプルはコミット済みの作者ソースをそのまま使うため、生成スクリプトは不要です。

## ツールのビルド

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
```

CLI の Cargo package 名は `roukaku-cli`、実行ファイル名は `roukaku` です。以下の例は `cargo run` を使うため、PATH へのインストールは不要です。

## Cook とパッケージ作成

公開サンプルは `Examples/NativeVN` にあり、エントリーポイントは `project.yaml` です。そこで実際に宣言されている target と profile を使います。

```sh
cargo run --locked -p roukaku-cli -- cook Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game --out .work/nativevn-cooked
cargo run --locked -p roukaku-cli -- package build .work/nativevn-cooked --target nativevn-flagship-game --out .work/nativevn.roukakupkg
```

Cook はシナリオと UI をコンパイルし、ローカライズ、アセット sidecar、プロジェクト section を収集します。`package build` は Cook 成果物の hash を検証してからコンテナを書き出します。既存の成果物を上書きしないよう、実行のたびに新しい出力パスを選んでください。

## Windows Player プレビュー

PowerShell で `$env:VC_CRT_REDIST` に、ビルドツールチェーンと一致する再頒布可能な Microsoft VC x64 CRT ディレクトリを設定します。このディレクトリには `msvcp140.dll`、`vcruntime140.dll`、`vcruntime140_1.dll` が必要です。デバッグ版 DLL ではなく、再頒布可能なリリース版 DLL を使ってください。

```powershell
cargo run --locked -p roukaku-cli -- package bundle .work/nativevn.roukakupkg --out .work/nativevn-preview --target nativevn-flagship-game --profile advanced-vn --platform windows --windows-player target/debug/roukaku-player.exe --windows-runtime "$env:VC_CRT_REDIST" --crash-reporter target/debug/RoukakuCrashReporter.exe
.work/nativevn-preview/RoukakuPlayer.exe
```

bundle 内の Player を起動すると、隣接する `RoukakuPlayer.config.json` から設定を読み込み、独立したウィンドウを開きます。素の `target/debug/roukaku-player.exe` には bundle の設定が必要で、プロジェクトのソースパスを直接受け取ることはできません。

物理的な入力とサンプルの UI で操作します。文字送り表示の途中では、最初の送り操作で行全体が表示され、次の操作でシナリオが進みます。画面表示、実際に聞こえる音声、選択肢、セーブ／ロードを確認してください。Cook やコンパイルが成功しても、実行時の動作が正しいことの確認にはなりません。

Linux では同じ Cook とパッケージのコマンドを使い、bundle 時に `--platform linux --linux-player target/debug/roukaku-player` を指定します。Windows CRT と crash reporter の引数は不要です。出力ディレクトリ内の `roukaku-player` を実行します。macOS では `--platform macos --macos-player target/debug/roukaku-player` を使い、エントリーポイントは出力ディレクトリ内の `Contents/MacOS/roukaku-player` です。いずれのプラットフォームでも、プロジェクトが選択したネイティブ provider の依存関係を満たす必要があります。

## Editor からのプレビュー

```sh
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
```

Preview setup でローカルのツール設定を読み込み、Save & Preview を選択します。同じ Cook、package、bundle、Player のパイプラインが実行され、実際の Player とのハンドシェイク後に一時停止、再開、停止が使えます。設定項目と現在の編集機能は [Editor](editor.md) を参照してください。

次は [VN 制作](vn-authoring.md)または[アーキテクチャ](architecture.md)へ進んでください。
