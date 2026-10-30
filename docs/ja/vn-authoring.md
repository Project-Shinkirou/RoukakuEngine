---
title: VN 制作
description: プロジェクト manifest、.roukaku、Luau、ローカライズ、アセット sidecar でビジュアルノベルを制作します。
sidebar: { order: 5 }
---

# VN 制作

公開リファレンスプロジェクトは `Examples/NativeVN` です。シナリオと UI の作者ソースは `.roukaku`、controller は `.luau`、テーマは JSON/YAML を使います。Cook はこれらのファイルを読み込んで製品パッケージを生成します。ランタイムが実行するのはコンパイル済みのシナリオで、`.roukaku` のテキストを直接解釈することはありません。

## プロジェクトのエントリーポイント

`project.yaml` は `roukaku.target_manifest.v3` を使います。製品の target、使用できる profile、各プラットフォームの host profile、NativeVN のソースディレクトリ、追加の package section を宣言します。サンプルの NativeVN 設定は次のとおりです。

```yaml
nativevn:
  default_locale: zh-Hans
  sources: [Scripts]
  ui_sources: [UI]
  ui_themes: [Themes]
  ui_controllers: [Controllers]
  profiles: [classic, advanced-vn]
  asset_roots: [AssetSidecars]
```

これは完全なプロジェクトの中の設定ブロックの一つで、単独でパッケージ化できるプロジェクトではありません。`targets` の `id` は CLI の `--target` に対応し、`--profile` は今回のビルドに含めるコンテンツを選択します。プラットフォームの profile は renderer、decode、audio、save の provider を明示的に指定します。プラットフォーム名だけを変えて、合わないバインドを使い回すことはできません。

## シナリオと演出

`.roukaku` はインデントによるブロックと名前付き属性を使います。以下の断片は、サンプルにある構造とローカライズキーに沿っています。

```text
story main #@id story.main
state experience_read #@id state.experience_read
  scene reading #@id scene.experience.reading
    text key:experience.read.first window:main #@id experience.read.first
    text key:experience.read.second window:main #@id experience.read.second
```

診断、source map、Editor での選択、属性の編集で位置を特定できるよう、安定した `#@id` を維持してください。コンテンツは `state` と `scene` で構成し、制御フローは `choice`／`option` と `jump` で表し、素材は `asset:/...` の ID で参照します。ステージには layer、camera、show/background などのコマンドを使い、タイムラインでは対象プロパティごとにキーフレーム、イージング、join ポリシーを宣言します。実際の構文は `Scripts/experience.roukaku` で確認し、未実装の DSL 設計をコンパイラへの入力として扱わないでください。

Graph と Timeline は同じソースを編集するため、作者が別のランタイムグラフを管理する必要はありません。シナリオの変数、選択肢、backlog、既読状態、セーブ／ロードは VN Core が保持します。

## UI と Luau

`UI/flagship.roukaku` では、`ui_view` がウィジェットツリーを定義し、`ui_bind` が profile ごとに message、choice、system page を view、controller、policy、theme にバインドします。ウィジェットは `$model` のデータを読み取り、`vn.advance` や `vn.choose` などの型付き action で要求を送ります。

Luau における製品のグローバル名は **`roukaku`** です。サンプルの controller は `roukaku.ui.controller.register(...)` でハンドラーを登録し、`roukaku.ui.effect.forward(action)` で UI action を転送します。controller ファイルは `nativevn.ui_controllers` が収集し、`.roukaku` ファイルではありません。ポリシーも `roukaku` が公開する機構を通じてコマンドと状態変更を扱います。プライベートな Lua テーブルを変更しても、VN の正式な状態は変わりません。

セーブ／ロード、設定、履歴には既存のシステムページとモデルを使ってください。controller の中に別のセーブコンテナやシナリオ実行器を実装しないでください。

## ローカライズ

シナリオのテキストは安定したキーを参照します。たとえば `Localization/en.json` の形式は次のとおりです。

```json
{
  "schema": "roukaku.vn.localization_table.v1",
  "locale": "en",
  "strings": {
    "experience.read.first": "The last train has left, but a reflection remains."
  }
}
```

`package_sections` で各ファイルを `vn.localization.<locale>` として宣言し、schema には `roukaku.vn.localization_table.v1` を使い、該当する target／profile を選択します。デフォルト言語は `nativevn.default_locale` で指定します。現在の profile に合うデフォルト言語の section がないプロジェクトは、Cook が拒否します。翻訳間でキーを揃えてください。UI のラベルには `l10n:` 参照を使えます。

## アセットと sidecar

`.roukaku-asset.yaml` は宣言した `asset_roots` の中に置きます。各ファイルには、安定した asset ID、プロジェクト相対の `source`、`source_hash`、種類、ライセンス、importer、cook processor、target profiles を記録します。`AssetSidecars/` はアセットを記述するもので、元の画像、音声、動画、フォントとは役割が異なります。作者ソースは asset ID でそれらを参照します。

アセットを変更したら、既存の importer で hash とメタデータを更新してください。古い hash を残したり、許諾を偽ったりしないでください。Editor の Import image は PNG/JPEG/WebP をインポートして sidecar を生成できます。詳しくは [Editor](editor.md) を参照してください。再頒布ライセンス、出典、作者の記録はアセットとともに残してください。商業ゲームの本文、鍵、セーブデータはプロジェクトのリポジトリに含めません。

### オーバーレイのマウント

package の VFS manifest でベース package section とオーバーレイレイヤーの優先順位を指定します。ホストが明示的に許可したローカルディレクトリだけを追加でき、未許可のディレクトリは VFS の許可診断とともに Player が拒否します。既存 catalog の asset ID と URI を使い、画像、音声、動画、フォント、UI ドキュメント、ローカライズテーブル、スクリプトを同じ名前空間で解決します。同じ優先順位の競合はエラーになり、whiteout は低優先順位のリソースを明示的に隠します。ファイルを置き換えると content revision が変わり、そのアセットのキャッシュだけが無効になります。

## チェックと再生

```sh
cargo run --locked -p roukaku-cli -- ui check Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game
cargo xtask ui-check
```

単一ファイルの構文チェックは、ファイルをまたぐプロジェクト全体の検証の代わりにはなりません。編集が終わったら[クイックスタート](quick-start.md)の Cook、package、Player の手順をたどり、実際のテキスト、レイアウト、メディア、セーブと復元の動作を確認してください。
