---
title: VN 制作
description: プロジェクト manifest、.roukaku、Luau、ローカライズ、アセット sidecar でビジュアルノベルを制作します。
sidebar: { order: 5 }
---

# VN 制作

公開リファレンスは `Examples/NativeVN` です。UI レイアウトは `*.screen.jsonc`、スタイルは `*.style.jsonc`、controller は `.luau` を使います。Cook は JSONC を検証し、既存のランタイム blueprint とテーマ manifest を生成します。シナリオの `.roukaku` は UI の形式とは別です。

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

`UI/flagship.screen.jsonc` は `views`、`bindings`、`policies` を含みます。各コントロールには `id` と `kind` が必須で、子は `children` 配列に置きます。`bindings` は profile ごとに message、choice、system page を view、controller、policy、theme に結び付けます。`policies` はセーブスロット、許可するページ、読み進め方、カスタム action を宣言します。ウィジェットは `$model`、`$item`、`$event`、`$state` を読み、`vn.advance` や `vn.choose` などの型付き action を送ります。

両形式のルートには `$schema` が必要です。エンジンは `Engine/Schemas/ui-screen.v1.json` と `Engine/Schemas/ui-style.v1.json` を公開します。JSONC では行コメント、ブロックコメント、末尾カンマを使えます。コメント、空白、プロパティの順序は実行結果に影響しません。schema と意味検証の診断にはファイル名・行・列が付きます。`anchor` と `position_*` は併用できず、競合時には `ROUKAKU_UI_POSITION_CONFLICT` を報告します。

```jsonc
{
  "$schema": "https://project-shinkirou.github.io/schemas/ui-screen.v1.json",
  "views": [{
    "id": "ui.title",
    "model": "roukaku.vn.ui_model.title.v1",
    "theme": "theme.classic",
    "root": {
      "id": "root", "kind": "screen",
      "children": [{
        "id": "start", "kind": "button", "style": "primary",
        "text": "l10n:system.new_game",
        "events": [{"event": "activate", "action": "vn.invoke_system_action",
                    "arguments": {"action_id": "nativevn.story.start"}}],
      }],
    },
  }],
}
```

```jsonc
{
  "$schema": "https://project-shinkirou.github.io/schemas/ui-style.v1.json",
  "$id": "theme.classic",
  ":root": {"text.body": {"color": [245, 245, 248, 255]}},
  "button": {"min_height": 48},
  ".primary": {"min_height": 56},
  "#start": {"font_size": 24},
}
```

スタイルは「セレクター → プロパティオブジェクト」のマップです。`$id` はランタイムのテーマ ID、`$parent` は任意の親テーマ、`:root` は型付き token、`:high-contrast` はハイコントラスト用 token を表します。優先順位はウィジェットの kind、`style` に並べたクラス（左から右）、`#id`、インライン指定の順です。`theme:token.name`、`l10n:key`、`asset:asset:/id` でテーマ token、翻訳、アセットを参照します。上の例はレイアウトとスタイルの断片です。実行可能なプロジェクトでは、リポジトリ内のサンプルの bindings と policies も参照してください。

`id` は view 内の編集上の識別子です。任意の `source_id` は不透明なランタイム識別子で、変換したサンプルは元の値を保ち、blueprint hash を維持します。ファイルの参照先ではありません。診断には JSONC ファイルと token の範囲を使います。

Cook は `nativevn.ui_sources` から `.screen.jsonc`、`nativevn.ui_themes` から `.style.jsonc` だけを収集します。既存の Cook artifact キャッシュを再利用し、ソースのパスと内容、sidecar、schema バージョン、controller/policy 入力、target/profile をキーに含めます。UI `.roukaku` と JSON/YAML テーマの読み込み経路は削除されました。

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

## チェックと再生

```sh
cargo run --locked -p roukaku-cli -- ui check Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game
cargo xtask ui-check
```

単一ファイルの構文チェックは、ファイルをまたぐプロジェクト全体の検証の代わりにはなりません。編集が終わったら[クイックスタート](quick-start.md)の Cook、package、Player の手順をたどり、実際のテキスト、レイアウト、メディア、セーブと復元の動作を確認してください。
