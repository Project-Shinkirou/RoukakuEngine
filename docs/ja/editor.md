---
title: Editor
description: プロジェクトを開き、同じソースを編集し、安全に保存し、アセットをインポートして実際の Player でプレビューします。
sidebar: { order: 6 }
---

# RoukakuEditor

Editor は独立した workspace にある GPUI デスクトップアプリケーションです。手作業での編集に Agent やモデルサービスは必要ありません。

## プロジェクトを開く

[ビルド](build.md)に従って依存関係を用意し、リポジトリのルートで次を実行します。

```sh
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
```

単一の `.roukaku` ファイルを渡すこともできます。プロジェクトモードでは、プロジェクトが宣言したシナリオ、UI、テーマ、Luau controller を読み込みます。プロジェクトの切り替えには Open project を使います。切り替え前に未保存の変更を処理し、開くのに失敗した場合は現在のプロジェクトがそのまま残ります。

## 編集と保存

Content Browser でソースファイルと宣言済みのアセットを探し、Outliner でコマンドを選択し、Details で選択したコマンドの属性を編集します。Text、Graph、Timeline は同じバージョン管理されたソースを操作します。安定した source ID によって、パネルでの選択と診断が作者の入力に結び付きます。

- **Graph** は実際の state とルートを表示します。state の接続ポートを対象の state にドラッグすると jump を追加でき、Delete route で対応する制御フローのコマンドを削除します。ノードは現在固定のグリッドに配置され、自由なレイアウトのドラッグには対応していません。
- **Timeline** は実際のキーフレームを表示し、時刻／値の変更、挿入、削除、同じトラック内でのドラッグができます。ドラッグを離すと、取り消し可能なバッチが 1 つコミットされます。古いバージョンや重複する時刻は拒否されます。既存の linear/ease_in/ease_out/ease_in_out に対応しており、自由曲線の編集と時間軸のズームは未実装です。
- **Save / Ctrl+S / Cmd+S** はすべてのソースドキュメントを保存します。各ファイルは一時ファイルに書き込んでから置き換えます。ディスク上の内容が外部で変更されていた場合は、上書きせずに競合として扱い、開き直しを求めます。複数のファイルは順に保存され、IO の失敗は報告されますが、ファイルをまたいだアトミックなコミットは保証しません。
- **Undo batch / Redo batch** は共有の編集履歴を使い、取り消しでも新しいバージョンが作られます。テキスト、属性、外部からの編集は、いずれも期待バージョンと UTF-8 範囲の検証を受けます。テキストが元と同じ内容に戻っても、古い結果が再び有効になることはありません。

コンパイル診断は既存のプロジェクトコンパイラから得られ、ソースの位置を表示してジャンプできます。プロジェクトの切り替えやソースの更新の後は、古いコンパイル結果が現在の診断を上書きすることはありません。未保存の変更があるプロジェクトを閉じるときは、キャンセル、保存、破棄を選べます。

## 画像のインポート

Content Browser の Import image は PNG、JPEG、WebP に対応しています。プロジェクト相対のインポート先パス、asset ID、ライセンス、宣言済みの sidecar ルートを選択します。バックグラウンドで `roukaku-cook` の importer が画像をデコードし、hash を計算して、画像と `.roukaku-asset.yaml` を書き込みます。

重複するパスや ID はインポートを拒否し、インポート先のディレクトリはプロジェクト内でなければなりません。インポートはすぐにアセットを書き込み、`.roukaku` の Undo 履歴には含まれません。単一ファイルモードでは、先に完全なプロジェクトを開く必要があります。音声、動画、フォントのインポートフォームと、アセットのサムネイルは現在ありません。

## Player プレビューの設定

先に CLI、Player、Windows で必要な Crash Reporter をビルドします。手順は[クイックスタート](quick-start.md)を参照してください。プレビュー設定はローカルの JSON で、項目は次のとおりです。

| 項目 | 意味 |
| --- | --- |
| `project` | 現在の `project.yaml` のパス |
| `cli` | ビルド済みの `roukaku` 実行ファイル |
| `player` | ビルド済みの `roukaku-player` 実行ファイル |
| `profile` | プレビューに使う profile（例：`advanced-vn`） |
| `target` | プロジェクトの target ID（例：`nativevn-flagship-game`） |
| `windows_runtime` | Windows で必須。一致する再頒布可能な Microsoft VC x64 CRT ディレクトリ |
| `crash_reporter` | Windows で必須。ビルド済みの `RoukakuCrashReporter.exe` |

現在の checkout を指す絶対パスを使い、設定ファイルは ignored のディレクトリに置いてください。以下の PowerShell は現在のリポジトリから設定を生成します。先に `$env:VC_CRT_REDIST` を設定してください。

```powershell
New-Item -ItemType Directory -Force .work | Out-Null
@{
  project = (Resolve-Path Examples/NativeVN/project.yaml).Path
  cli = (Resolve-Path target/debug/roukaku.exe).Path
  player = (Resolve-Path target/debug/roukaku-player.exe).Path
  profile = 'advanced-vn'
  target = 'nativevn-flagship-game'
  windows_runtime = (Resolve-Path $env:VC_CRT_REDIST).Path
  crash_reporter = (Resolve-Path target/debug/RoukakuCrashReporter.exe).Path
} | ConvertTo-Json | Set-Content -Encoding utf8NoBOM .work/editor-preview.json
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml --preview-config .work/editor-preview.json
```

この例で BOM なしの UTF-8 JSON を書き出すには PowerShell 7 が必要です。Linux/macOS では同じ項目でネイティブの実行ファイルを指し、Windows 用の 2 項目は省略します。すでに開いている Editor でも、Preview setup から設定を選択できます。

Save & Preview は現在のソースをコンパイルして保存し、続けて `cook → package build → package bundle → Player` を実行して、独立した GPU ウィンドウを起動します。一時的な中間パッケージはプロジェクトの `.roukaku-cache/preview` の下に置かれます。Pause/Resume は Player が Ready を返した後にだけ表示されます。一時停止中のシークは、現在のフラグメントが実際に保持しているチェックポイントに限られます。Stop preview はまず実際のセッションに終了を要求し、タイムアウトした場合に子プロセスを回収します。

ソースの変更、プロジェクトの切り替え、新しいプレビュー、Editor の終了によって、古いプレビューは無効になり、そのプロセスは回収されます。失敗時は終了ステータスと上限付きのログが表示されます。コンパイルの成功を、画面表示や実際に聞こえる音声の検証とみなさないでください。

## オプションの外部 Agent

`--agent` は外部の ACP Agent コマンドを受け取ります。モデルと認証はその Agent 側で設定します。`--mcp` は stdio の MCP を起動し、`read_document` と `apply_batch` を提供して、同じ未保存のドキュメントにアクセスします。デフォルトではバッチごとに確認し、ユーザーが Apply batch か Reject batch を選びます。自律モードでも、同じバージョン、範囲、キャンセルの検証が行われます。キャンセルや手作業の編集が入ると、古い世代の結果は拒否されます。

Editor は独自のモデルリクエストループを持たず、外部プロセスに汎用的なサンドボックスの保証も与えません。手作業の編集と Player プレビューは、それぞれ単独で使えます。

続けて [VN 制作](vn-authoring.md)と[アーキテクチャ](architecture.md)を参照してください。
