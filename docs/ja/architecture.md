---
title: アーキテクチャとインターフェース
description: crate の分担、セッションの所有権、実行時の予算、キャンセル、パッケージとセーブデータの完全性。
sidebar: { order: 4 }
---

# アーキテクチャとインターフェース

## コードマップ

| ディレクトリ | 責務と主な crate |
| --- | --- |
| `Engine/Source/Runtime` | `roukaku-core` の基本型、`roukaku-runtime` のシーン・タスク・セッション、`roukaku-package` のコンテナ、`roukaku-asset` と `roukaku-byte-source` のリソースと有界読み取り、および文字・音声／動画・字幕・UI・Player の共通機構 |
| `Engine/Source/Platform` | `roukaku-platform` のホスト契約、`roukaku-platform-common` の共有ホスト実装、各プラットフォームのバックエンド、`roukaku-target` のプロジェクト／target 検証 |
| `Engine/Source/Developer` | `roukaku-cook` のインポートと Cook、`roukaku-release` のリリースデータ検証、`roukaku-observability` のログと診断、および Headless テストツール |
| `Engine/Source/Programs` | `roukaku-cli`（実行ファイル `roukaku`）、デスクトップ／Android／Web の Player、Headless、Crash Reporter、Player と VN の接続 |
| `Engine/Source/Modules/RoukakuVN` | スクリプトのコンパイル、シナリオのセマンティクス、ポリシー、演出、UI、パッケージ section、編集トランザクション、型付きの VN 製品エントリーポイント |
| `Editor/Source/roukaku-editor` | 独立した workspace にある GPUI アプリケーション、プロジェクトパネル、外部 Agent／MCP との接続、Player プレビュープロセスの管理 |

RoukakuVN の内部では、`roukaku-vn-script` が `.roukaku` をコンパイルして source map を生成します。`roukaku-vn-core` はシナリオのカーソル、変数、選択肢、待機、backlog、システム状態を所有します。`roukaku-vn-policy` は Luau をホストし、`roukaku-vn-presentation` はステージと演出を管理し、`roukaku-vn-package` は製品の section を管理し、`roukaku-vn-ui` は UI モデルを保持し、`roukaku-vn-editor` は GPUI に依存しない編集トランザクションを提供します。`roukaku-vn::VnSession` がこれらの製品機能を組み合わせます。

## セッションとプラットフォームの境界

`roukaku-runtime::EngineSession` は共有 World、タスクスコープ、論理ステップのライフサイクルを保持します。`VnSession` はさらに VN のシナリオとシステム状態を保持します。Player と Editor のプレビューは同じ製品セッションを使い、プレビュー専用のシナリオ実行器は作りません。デスクトップ版 Player は約 60 Hz で論理ステップをスケジュールし、表示とプラットフォームイベントは独立して処理します。

プラットフォームの host profile は、renderer、decode、audio mixer、audio output、save provider、パッケージの取得元を明示的に指定します。明示した provider のバインドに失敗した場合はエラーを報告し、別のバックエンドの出力を元のバインドの成功として扱うことはありません。文字、メディア、バイトソースなどの共有ライブラリは、実際の利用側が個別に組み合わせて使え、先に VN セッションを作る必要はありません。

## 実装済みの実行時保護

これらは製品の振る舞いであり、リファクタリング時にも維持する必要があります。

- **予算**：host profile は、キュー、フレーム、音声、PCM キャッシュ、パッケージ読み取りの上限がゼロでないことを検証します。表示、デコード要求、セーブデータの読み書き、パッケージの範囲読み取りは、入口でそれぞれのサイズを確認します。コンテナは section 数、テーブルサイズ、デコード後の長さを別途制限し、Cook のバッチもノード数と入力バイト数の予算を確認します。これらはそれぞれ別の障害境界を守るもので、汎用的な「無制限のメモリ保護」ではありません。
- **パス**：プロジェクトのリソースと section は検証済みの相対パスを使います。bundle は書き込み先のパスを検証してから書き込み、シナリオの参照は不正な相対パスを拒否します。Editor のリソースインポートは、実際のディレクトリがプロジェクト内に収まっていることを確認し、重複するパスと asset ID を拒否します。パスの検証は任意のディスクアクセスを許可するものではありません。
- **キャンセル**：タスクは owner、スコープ、ハンドルに結び付けられます。キャンセル済み、完了済み、または別の World 世代に属する completion は拒否されます。セッションを閉じると所有するタスクはキャンセルされ、復元時は新しいタスクコンテキストが古いものを置き換えるため、遅れて届いた結果はコミットされません。Cook は共有のキャンセルトークンを使います。Editor でプロジェクトを切り替えると、古いコンパイル、インポート、Agent、プレビューの処理はキャンセルされます。
- **セーブデータの完全性**：Luau の段落セーブは、入口時の変数とシーン状態、段落 ID、ソース revision、リプレイログを保存します。段落途中のカーソルやコルーチンスタックは保存しません。ロード時は段落を入口から再実行し、記録済み入力をリプレイします。ソース revision が一致しない場合は入口から再開し、`ROUKAKU_VN_SOURCE_REVISION_MISMATCH` を報告します。ロールバック checkpoint は台詞と選択ノードを識別し、task scope が古い非同期完了を無効にします。

## パッケージとフォーマット

`.roukakupkg` と `.roukakupak` は、製品パッケージとプレビュー用の中間パッケージに使います。`roukaku-package` の v2 コンテナは 8 バイトのヘッダー magic `ROUKACT2` と、8 バイトのフッター magic `ROUKAF2\0` を持ちます。リーダーはコンテナの種類、バージョン、長さ、section の範囲、およびヘッダー、テーブル、payload の hash を検証します。section を遅延読み込みする場合も、格納データは検証されます。

名称変更前の製品コンテナ、schema、プロジェクト独自のセーブデータは互換入力として扱いません。古い magic は入口で拒否され、拡張子を変えても中身は変換されません。現在の作者ソースから Cook とパッケージ化をやり直し、古いファイルとユーザーデータはそのまま残してください。

## Luau 制作機構の契約（R1/R3 の基盤）

`roukaku-vn-policy::authoring::LuauAuthoringRuntime` は、既存の `luau-runtime` feature と mlua を通じて、公式の Luau VM でプロジェクトのソースを実行します。機構 API はデフォルトの VN UI から独立しています。この統合ブランチでは既存の DSL を維持し、Player／Cook のエントリーポイントも変更しません。今後の製品の切り替えでは、この API を直接使います。

### 段落、モジュール、入力

ホストは `PolicyExecutionBudget`、owner の `TaskScope`、`Rc<dyn AuthoringHost>` でランタイムを作成します。`replace_module(module, source_name, source)` はソースチャンクを実行し、返されたエクスポートテーブルを取得します。Luau のエラーやコールスタックが作者のソースを指すよう、`source_name` には `@Story/chapter1.luau` のような実際のファイル名を使ってください。モジュールのグローバル環境は読み取り専用です。ローカル変数と明示的に登録した状態は通常どおり使えます。

```lua
return {
    paragraphs = {
        intro = function()
            local line = rk.line("greeting", { text = "Hello" })
            rk.wait_input("advance")
            local choice = rk.choice("route", {
                { id = "left", text = "Left" },
                { id = "right", text = "Right" },
            })
            local selected = rk.wait_input("choose")
            rk.choice_result(choice, selected)
        end,
        -- エントリーテーブルにも対応：intro = { run = function() ... end }
    },
}
```

`start_paragraph(ParagraphId { module, entry })` は `exports.paragraphs[entry]` を関数または `{ run = function }` として解決します。この組が段落の再開とロードの安定した ID です。再開すると古いセグメントと入力をキャンセルし、新しい子スコープとコルーチンを作成して、ノードの訪問回数をリセットします。段落セーブは入口 checkpoint とソース revision を記録します。ロード時は入口から再実行し、入力ログをリプレイして保存時の表示ノードに戻ります。ソース revision が一致しない場合は入口から再開し、診断を返します。

`rk.wait_input(action: string): any` はシナリオのコルーチンだけを中断します。ホストは `StoryStatus::Waiting(InputWait { token: u64, action: String })` を受け取り、`input(token, action, json_payload)` で一致する live wait を再開します。無関係な入力や遅れて届いた入力は `false` を返します。段落の再開やランタイムの置き換えを経ても、live token が重複することはありません。待機を開始できるのは、現在のシナリオのコルーチンだけです。待機中もホストは `advance_presentation(seconds)` を呼んで演出を進めるため、ホストスレッド、オーディオクロック、レンダラーはブロックされません。

`rk.call(module: string, name: string, ...): ...` は呼び出しのたびにエクスポート関数を解決し、プロジェクト関数内での待機に対応します。タイムラインのコールバックは `{ module: string, name: string }` を使い、ディスパッチ時に同様に解決されます。モジュールを置き換えると以後の呼び出しに反映されますが、すでに中断している呼び出しフレームは古い関数を使い続けます。ソースチャンクの実行に失敗した場合、エクスポートテーブルは置き換えられませんが、エラー前に明示的に実行した機構の副作用は残ります。構造に互換性がない場合は、段落をキャンセルして再開してください。

### ノード、テキスト、復元可能な状態

| 機構 | 型とセマンティクス |
| --- | --- |
| `rk.line(node, data)` | 台詞とプロジェクトの純データを記録し、`NodeOccurrence = { paragraph = { module, entry }, node, visit }` を返します。UI の表示や待機は自動では行いません。 |
| `rk.choice(node, options)` | `{ id: string, text: string }[]` を記録します。選択肢 ID は空でなく重複しないことが必要です。同じ種類の occurrence を返します。 |
| `rk.choice_result(occurrence, selected)` | その選択の記録に有効な選択肢 ID を 1 つコミットします。コミットは 1 回だけです。 |
| `rk.measure_text(request)` | シリアライズされた完全な `roukaku_text::TextLayoutRequest` を受け取り、方向、ルビ、フォントチェーン、制約を保持したまま `{ width, height, clipped, ellipsized, revision }` を返します。 |
| `rk.display_text(id, request)` | 同じレイアウト provider を使い、同じ形式の計測値を返します。`AuthoringHost::display_text` はグリフリソースと source/cluster の対応を含む `Arc<TextLayoutResult>` を受け取り、そのまま描画に使います。 |
| `rk.register_state(scope, key, initial)` | 空でない状態キーを登録します。既存の値があればそれを返し、初回登録時にだけ初期値を書き込みます。登録を繰り返しても復元済みのデータは上書きされません。 |
| `rk.get_state(scope, key)` / `rk.set_state(scope, key, value)` | 登録済みのキーを読み書きします。未登録のキーはエラーになります。scope は `"variables"`、`"scene"`、`"extensions"` のいずれかです。 |
| `rk.random_u64()` | 決定的な 53 bit の乱数を返し、値を段落リプレイログに記録します。 |
| `rk.once(id)` | 通常実行では `true`、リプレイ時に同じ一回限りの effect ID を消費すると `false` を返します。音声やボイス再生など、外部の一回限りの effect のガードに使います。 |

visit は、**段落の 1 回の実行の中で、ソースノードごとに 1 から数える訪問回数**です。ループ内の複数回の訪問はそれぞれ別の識別子を持ち、同じエントリーから同じ経路をリプレイすると同じ識別子が再現されます。再開後は論理ノードの識別子が重複することがありますが、live wait／segment の token は新たに割り当てられます。ノードの記録は機構が保持するため、会話 UI を置き換えても失われません。履歴やリプレイの層は `records()` で読み取ります。

復元可能な値は、有限の数値を含む JSON 風の純データで、配列と null に対応し、深さとホストメモリの予算による制限を受けます。関数、スレッド、ネイティブの userdata ハンドル、任意の metatable、循環テーブルは拒否されます。`recoverable_state()` は明示的な変数、シーン、拡張の状態を返します。`restore_registered_state(candidate)` は検証してからインストールし、live なシナリオのコルーチンや実行中のセグメントがないことを要求します。ネイティブオブジェクトの作成や実行スタックの復元は行いません。ステージのプロパティは `scene[target][property]` にも記録されるため、target に対応するシーン状態はオブジェクトでなければならず、プロジェクトは同じキーにオブジェクト以外の値を保存してはいけません。任意の Luau スタックのシリアライズは禁止です。

### 宣言的な演出と終端状態の所有権

```lua
type FunctionRef = { module: string, name: string }
type Keyframe = { time: number, value: number }
type Track = { target: string, property: string, keys: {Keyframe} }
type Callback = { time: number, call: FunctionRef, complete: FunctionRef }
type Timeline = {
    duration: number, tracks: {Track}, callbacks: {Callback}?,
    on_complete: FunctionRef?, on_cancel: FunctionRef?,
}
```

`rk.segment(declaration: Timeline): number` はスコープ内のセグメントを作成し、ただちに時刻 0 を評価します。時刻と長さは有限の秒数で表し、プロパティの値も有限でなければなりません。各 target/property は 1 回だけ現れ、キーフレームの時刻は `[0, duration]` の範囲で狭義単調増加し、トラックは線形補間と端点でのクランプを使います。`TimelineDeclaration::evaluate(time, apply)` は編集とプレビュー向けに任意時刻の純粋な評価を提供し、自由ロジックの呼び出しやホストの変更は行いません。プロジェクトの効果、メディア、自由ロジックはコールバック参照とホストのプロパティを通じて接続し、宣言にクロージャは保存しません。

`rk.set_property(target, property, value)` は `AuthoringHost::set_property` を通じて有限の数値プロパティを適用し、復元可能なシーンデータを更新します。`advance_presentation(delta)` はシナリオの入力とは独立して、実行中のトラックを評価します。時刻に達したコールバックは宣言順に 1 回ずつ実行されます。ライフサイクルコールバックは同期的に実行され、yield してはいけません。

`rk.complete(id)`（ホスト側では `complete_segment`）は `Completed` の所有権を取得し、すべてのトラックの終了値を適用し、まだディスパッチされていない時刻付きコールバックについてはその `complete` 参照を呼び、次に `on_complete` を呼んで、最後にリソースを解放します。通常の再生では、時刻に達した `call` 参照をディスパッチし、duration に達した時点で完了します。`rk.cancel(id)`／`cancel_segment` は `Cancelled` の所有権を取得し、現在の状態で停止して `on_cancel` を呼び、リソースを解放します。終了値や必須の完了結果は適用しません。最初の終端遷移は `true` を返し、すでに終端状態のセグメントでは `false` を返します。owner のスコープがすでに無効になっている場合、キャンセルはネイティブリソースを解放するだけで、そのスコープのプロジェクトコードは実行しません。

終端状態の所有権は、ユーザーのコールバックより先に取得されます。コールバックの失敗は上位に伝播し、再試行しても副作用が重複することはありません。失敗した場合もリソースの解放は試みられます。処理されなかったシナリオのエラーや演出の tick エラーは、所有する残りの処理をキャンセルします。親スコープのキャンセル、段落の再開、明示的なキャンセル、ランタイムの破棄は、いずれも所有するセグメントを解放します。終端に達した各セグメントについて `AuthoringHost::release_segment(id)` は 1 回だけ呼ばれ、完了とキャンセルのどちらもこの規則に従います。

VM は既存のホスト IO サンドボックスと Lua のメモリ上限を維持します。命令数と出力量の枠は、ホストが制御するディスパッチ境界でのみリセットされます。ホスト側の純データは、保持しているノード記録、演出の宣言、状態のコピーを含め、ランタイムの存続期間にわたって保守的に累積計上されます。既存の数値ステージプロパティを更新しても、保存コピーが追加で計上されることはありません。予算を使い切ると以後の操作は拒否され続けるため、新しいランタイムを作成する必要があります。プロジェクトの API から予算をリセットしたり、処理を owner のスコープから切り離したりすることはできません。


## ShinkiEMU との一方向の共有

ShinkiEMU は `roukaku-core`、`roukaku-byte-source`、`roukaku-media-core`、`roukaku-text`、`roukaku-media`、`roukaku-subtitle`、`roukaku-platform`、`roukaku-platform-common`、`roukaku-observability` を使います。本番の依存関係は ShinkiEMU の workspace で同じ RoukakuEngine の Git revision に一括で固定され、開発マシン上の隣接ディレクトリに依存したり、Rust の実装をコピーしたりすることはありません。

依存の方向は **ShinkiEMU → RoukakuEngine** のみです。RoukakuEngine とこれらの共有 crate は、ShinkiEMU、その Manager、コア ABI に依存してはいけません。共有の機構は既存の owner crate に置き、シナリオ、ネイティブエンジンのフォーマット、製品のルールはそれぞれの製品側に置きます。

関連する操作は[ビルド](build.md)、[VN 制作](vn-authoring.md)、[Editor](editor.md) を参照してください。
