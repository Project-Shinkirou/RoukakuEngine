---
title: ビルド
description: Rust ツールチェーン、ネイティブ依存関係、xtask チェック、Android と macOS のビルド手順。
sidebar: { order: 3 }
---

# ビルド

## ツールチェーンと workspace

`rust-toolchain.toml` は Rust `1.99.1` を選択し、`rustfmt`、`clippy`、ターゲット一覧を宣言します。Engine の crate は Rust 2024 edition を使い、Rust 1.99 以降が必要です。ターゲット一覧は Rust 標準ライブラリのインストール用で、プラットフォームの実行サポート表ではありません。ネイティブの C/C++ コンパイラ、リンカー、CMake、および選択したネイティブ依存関係に必要なツールをインストールしてください。Windows では MSVC ツールチェーンを使います。

ルート workspace には Engine、RoukakuVN、Player、CLI、共有ライブラリが含まれます。Editor は独自の `Editor/Cargo.toml` と lockfile を持ちます。リポジトリのルートで `cargo xtask` を実行し、`engine`、`editor`、`all` のいずれかを選択します。内部で呼び出す Cargo コマンドは `--locked` を使います。

依存関係のベースラインは vcpkg `0c2a9ace1964b818b15f7ae50ba1c42404918a7b`、libass 0.17.5、オプションの FFmpeg 9.0.2 port revision 2 です。`mlua` は最新リリースの 0.12.2 を維持しています。リリース版の `mlua-sys` 0.13.0 は VM 初期化時に Luau 744 が生成したバイトコードを拒否するため、workspace は[上流の loader 修正](https://github.com/mlua-rs/mlua/blob/ff96ed786295cae96f43ef1372bd22be0fbf7866/mlua-sys/src/luau/compat.rs)に固定しています。Luau のダウングレードやランタイムテストの省略は行いません。

Engine のレンダリングは wgpu 30.0.x を使い、ShinkiEMU および Slint 1.18.1 の `unstable-wgpu-30` バックエンドと揃えています。Windows では `static-dxc` feature を維持します。表示はコマンドを送信した queue で行い、surface はデフォルトの `Auto` 色空間を使って SDR 出力を保ちます。Editor は現在 Engine の crate 経由で wgpu を使っておらず、GPUI レンダラーと独立した lockfile は変わりません。

```sh
cargo xtask docs
cargo xtask check --workspace engine
cargo xtask check --workspace editor
cargo xtask build --workspace engine -p roukaku-cli -p roukaku-player
cargo xtask test --workspace engine -p roukaku-runtime
cargo xtask fmt --workspace all
cargo xtask lint --workspace editor
cargo xtask ui-check
```

`check` はドキュメント、フォーマット、Clippy、ビルド、テストを順に実行します。`fmt` はフォーマットのチェックのみで、ファイルを書き換えません。`lint` は Clippy を実行し、warning を拒否します。package を指定しない `test` は、実際の CLI／ホストテスト用に先に製品をビルドします。`-p` で対象を絞った場合、この事前ビルドは行いません。`-p` には明示的な workspace の指定が必要です。

実際のウィンドウや GPU surface、物理的な音声出力デバイスを使うテストはデフォルトで無視され、ホスト型 CI でも実行されません。テストのアサーションとネイティブバックエンドはそのままです。必要なデバイスがある開発マシンでは、`test` または `check` に `--hardware` を付けて有効にします。

```sh
cargo xtask test --workspace engine --hardware -p roukaku-platform-windows
cargo xtask check --workspace engine --hardware
```

このフラグはテストハーネスに `--include-ignored` を渡すため、これまで無視されていたテストも実行されます。各テストが必要とするプラットフォームのコーデックと fixture を用意してください。`platform-test-driver` で制御されるテストには、引き続き `--features platform-test-driver` が必要です。入力ドライバーのテストは実際のウィンドウにフォーカスを移し、入力を送信します。CI では `--hardware` を渡しません。

次のテストはウィンドウを開かずに、ネイティブ GPU 上でシーン描画、WGSL フィルター、テクスチャのアップロードとリードバックを実際に行います。

```sh
cargo test --locked -p roukaku-platform-common --test offscreen native_offscreen_gpu_renders_scene_filter_and_readback -- --ignored --nocapture
```

`check-fast` はドキュメントのリンクとテキストの衛生チェックだけを行い、コードのコンパイルやテストは行いません。`docs` も同じドキュメントチェックを実行します。`ui-check` は固定版の Luau analyzer を明示的にダウンロードして NativeVN の controller をチェックするため、ネットワーク接続が必要です。

## コンパイル統合型の作品拡張

作品拡張は `roukaku_plugin::WorkExtension` を実装し、プロジェクトの `player` crate で `WorkExtensionRegistry::register` により Rust 型を登録します。`roukaku_player::run()` を呼ぶ前にレジストリをインストールし、セッションごとに拡張インスタンスを作成します。Player は起動時に provider、コマンド、schema を検査し、未登録の provider は `ROUKAKU_EXTENSION_PROVIDER_UNREGISTERED` を報告します。コマンドは `ExtensionInvocation` で引数を受け取り、実際の `SceneCommand` を出力します。出力は provider ごとに後続フレームまで保持されます。スカラー引数は JSON 値に対応し、固定小数点引数は 100 万分の 1 単位の整数を使います。

`Examples/ExtensionPlayer/extension` は公開して再利用できるプログレスバー拡張です。隣の `player` crate がこれを通常の Player ホストにリンクします。テストでは作品パッケージ内のコマンドを起動し、最終的な表示出力を検査します。

```sh
cargo xtask project-player Examples/ExtensionPlayer --output ./project-player.exe
# --target で Cargo ターゲットを指定できます。ビルドマシンには対応するネイティブ SDK が必要です。
cargo xtask project-player ./ContentProject --prebuilt ./roukaku-player.exe --output ./content-player.exe
```

`project-player` は `--locked --release` で `player/Cargo.toml` をビルドし、Cargo の成果物メッセージから唯一の実行ファイルを読み取って出力パスにコピーします。コンテンツのみのプロジェクトには `player/Cargo.toml` がないため、`--prebuilt` をそのまま再利用します。出力ファイルを既存の package bundle コマンドで対応プラットフォームの Player オプションに渡せば、パッケージ化できます。拡張プロジェクトでは、専用 Player の代わりにビルド済みバイナリを使うことはできません。

`WorkExtensionDeclaration` と `ExtensionCommandDeclaration` は provider ID、コマンド schema、引数 schema というデータのみを持ちます。コンパイル統合された Editor パネルはこれらの型を共有でき、Player は Editor UI に依存せずに済みます。ランタイムの動的ライブラリローダー、バイナリ記述子、プラグイン ABI ゲートは削除されました。`roukaku-engine` と `roukaku-vn` は通常の `rlib` 依存としてビルドされ、不要な Rust `dylib` は生成されなくなりました。Android と Web ホストの `cdylib` エントリーポイントはプラットフォーム成果物として残ります。拡張は `restoration()` で `Transient` か `Restorable` を明示的に選ぶ必要があります。`ExtensionState` は業務状態の取得、候補の検証、復元のフックを提供し、チェックポイント／復元を担う部分がこれを組み込みます。

## ネイティブ依存関係

依存関係の準備は明示的な操作で、通常のビルドがシステムパッケージをインストールすることはありません。以下のコマンドは固定した vcpkg revision を使い、必要に応じて FFmpeg と libass 字幕の依存関係を用意します。

```sh
cargo xtask deps --target x86_64-pc-windows-msvc --subtitle
cargo xtask deps --target x86_64-pc-windows-msvc --ffmpeg --subtitle
```

`--ffmpeg` と `--subtitle` のどちらも指定しなければ、何もダウンロードしません。FFmpeg が必要かどうかは、実際にビルドする crate／features とデコード provider によって決まります。プロジェクトが明示的にバインドした WMF、GStreamer、AVFoundation などには引き続きそれぞれのネイティブ環境が必要で、FFmpeg を用意してもこれらのバインディングが置き換わることはありません。

ツールは vcpkg を `.work/vcpkg` に、字幕用は独立した `.work/vcpkg-subtitle` に配置します。環境設定を出力し、GitHub Actions では環境ファイルに書き込みます。ローカルで続けてコマンドを実行する場合は、出力された設定を自分で適用してください。Windows PowerShell での字幕の設定は次のとおりです。

```powershell
$env:ROUKAKU_SUBTITLE_VCPKG_ROOT = (Resolve-Path .work/vcpkg-subtitle).Path
$env:PATH = "$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/bin;$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/debug/bin;$env:PATH"
```

FFmpeg を用意した後は、出力された `VCPKG_ROOT`、`VCPKG_DEFAULT_TRIPLET`、ランタイム DLL の PATH も設定します。Windows 以外のターゲットでは、出力に従って `PKG_CONFIG_PATH` と `LD_LIBRARY_PATH` または `DYLD_LIBRARY_PATH` を設定します。製品を配布する際は、ネイティブ依存関係のライセンスと動的ライブラリの扱いにも対応してください。

## 動画再生と音声同期

プラットフォームのデコーダーが引き続き主経路です。Windows は WMF、Apple プラットフォームは AVFoundation、Linux は GStreamer、Android は MediaCodec、実験的な Web ターゲットは WebCodecs を使います。FFmpeg はオプションのままです。AVFoundation の動画は `AVAssetReader` で PTS 付きのフレームを必要に応じて読み取り、先読みサンプル 1 つと Player が持つ表示待ちフレーム 1 つだけを保持します。動画全体を `CpuBuffer` にまとめることはありません。ストリームを閉じると reader はキャンセルされ、seek ではデコーダーを作り直して古い表示処理を無効にします。

Player は同じ動画コンテナ内の音声トラックを、選択したプラットフォームの音声デコーダーに渡し、既存の Kira mixer に送ります。公開されたメディア scheduler は、レンダリング済みで未消費のサンプルとデバイスの出力レイテンシを PCM voice cursor から差し引き、音声マスタークロックを求めます。CPAL の出力は callback／playback のタイムスタンプを読み取り、Android は Oboe のレイテンシを読み取ります。音声トラックのない動画だけがホスト時刻を使います。一時停止は transport を止め、seek は古い出力キューを破棄して目標位置で両トラックを再構築します。デバイス復帰時は、聞こえていた位置から PCM voice を再構築します。動画の音声は既存の PCM 予算に従います。音声トラックの終了後は、デバイスが実際に消費した mixer の無音が残りの動画を駆動します。完了時刻は両トラックの実際の EOS で決まり、コンテナの長さの推定値には依存しません。

Windows のリグレッションテストは WMF で両トラックをデコードし、実際の Kira mixer を通して有界のネイティブ出力キューに書き込み、制御されたソフトウェア callback でサンプルを消費します。

```sh
cargo test --locked -p roukaku-audio-kira --test native_movie_sync -- --nocapture
cargo test --locked -p roukaku-media --test container_tracks
```

テストは一時停止、seek、出力の再構築をカバーし、動画の PTS と音声の進行のずれが scheduler の閾値（遅れ 100 ms／先行 20 ms）を超えないことを確認します。物理的な音声デバイスは開きません。Apple の reader、PTS、seek、cancel のリグレッションには Apple SDK が必要で、CI `products` の `macos-15` と `macos-15-intel` の行で実行されます。

`roukaku-media/tests/fixtures` 内の `av-clock.mp4`、`silent.mp4`、`silent.webm` はテストパターンと正弦波から生成したもので、CC0 で提供しています。ダウンロードしたメディアは含みません。音声付き動画サンプルの生成コマンドは次のとおりです。

```sh
ffmpeg -f lavfi -i testsrc2=size=64x64:rate=25:duration=3 -f lavfi -i sine=frequency=440:sample_rate=48000:duration=1.8 -c:v libx264 -pix_fmt yuv420p -bf 0 -g 25 -c:a aac -ac 2 -b:a 32k -movflags +faststart av-clock.mp4
```


## Android Player

エントリーポイントは `cargo xtask android` です。JDK 27、`cargo-ndk`、Rust の `aarch64-linux-android` target が必要で、`ANDROID_SDK_ROOT` または `ANDROID_HOME` を設定します。SDK プラットフォーム `platforms;android-37.2`、Build Tools `37.0.0`、安定版の [NDK r30](https://developer.android.com/ndk/downloads) `30.0.16248370` をインストールしてください。プロジェクトは AGP 9.4.1 と Gradle 9.8.2 を使います。JNI ライブラリは引き続き 16 KiB の ELF ページアラインメントを使い、APK 内の非圧縮ネイティブライブラリは最新の zip アラインメントを使います。

ビルド用 JVM は引き続き JDK 27 です。[Gradle 9.8.2 は Java 27 を明示的にサポート](https://docs.gradle.org/9.8.2/userguide/compatibility.html)しており、[Android の JDK ガイド](https://developer.android.com/build/jdks)はビルド用 JDK が AGP の最低要件（AGP 9.4 では JDK 17）以上であることを求めています。ここで選んでいるのはビルドツールを動かす JVM であり、Android アプリの Java API やバイトコードのバージョンではありません。

まず[クイックスタート](quick-start.md)に従って Android host profile を含む `.roukakupkg` を作成し、次を実行します。

```sh
cargo xtask android --package .work/nativevn.roukakupkg --target nativevn-flagship-game --application-id org.shinkirou.roukakuplayer --output .work/nativevn-android
```

ここでの `--target` は製品の target ID で、Rust triple ではありません。現在のコマンドは arm64-v8a の JNI ライブラリをビルドし、プロジェクト内の Gradle wrapper を実行して、実際の APK／AAB、JNI ライブラリ、hash マニフェストを収集します。出力ディレクトリは新規である必要があります。`--profile` は Cargo profile を選択し、デフォルトは `release` です。`--signing-properties` でローカルの署名設定を渡せます。指定しない場合、debug APK はデバッグ署名され、release 成果物は未署名として扱われるため、正式なインストールパッケージとしては使えません。署名ファイルはコミットしないでください。

Android パッケージには対応するプラットフォーム profile が必要です。デスクトップ用パッケージの名前を変えても Android 対応にはなりません。タッチ操作、戻るキー、フォアグラウンド／バックグラウンドの切り替え、音声、セーブ／ロードは実機で確認してください。

## Linux からの Apple ターゲットビルド

ネイティブの macOS CI は引き続き `macos-15` / `macos-15-intel` 上で Cargo と Xcode を使います。Linux 上での Apple ターゲットのビルドには cross 0.2.5 を使い、`Cross.toml` に記載したプライベートイメージを使用します。最低バージョンは macOS 13.0、iOS 16.0 です。ターゲットは `x86_64-apple-darwin`、`aarch64-apple-darwin`、デバイス用の `aarch64-apple-ios`、シミュレーター用の `aarch64-apple-ios-sim` と `x86_64-apple-ios` です。

### 正規の SDK を用意する

[Mac App Store](https://apps.apple.com/app/xcode/id497799835) または [Apple Developer Downloads](https://developer.apple.com/download/all/) から Apple のライセンスに従って Xcode を入手します。Mac 上で `xip --expand` を使って公式の `.xip` を展開し、その Xcode を選択して、同じ Mac でライセンスに同意します。Command Line Tools 単体には iOS SDK が含まれません。続いて、内部のシンボリックリンクを保ったまま SDK の内容をエクスポートします。

```sh
for sdk in macosx iphoneos iphonesimulator; do
  tar -C "$(xcrun --sdk "$sdk" --show-sdk-path)" -cJf "$sdk.sdk.tar.xz" .
done
```

アーカイブは非公開の手段で Linux に転送し、SDK ごとに別のディレクトリへ展開します。`SHINKI_APPLE_SDK_DIR` は `SDKSettings.json`、`usr/`、`System/` を含む SDK の内容ディレクトリを直接指します。Darwin には `macosx`、デバイスには `iphoneos`、2 つのシミュレーターターゲットには `iphonesimulator` を使います。xtask も Dockerfile も Apple SDK をダウンロードしません。SDK のアーカイブや SDK を含むイメージを Git や公開レジストリに置かないでください。

### ツールの準備とビルド

ソース、キャッシュ、ツールは十分な空き容量のある専用ディレクトリに置きます。イメージ作成時に `/` の空き容量が 8 GiB 未満の場合はそのまま拒否されます。無関係なキャッシュや Docker イメージは削除しないでください。Linux での設定例：

Docker には Buildx/BuildKit が必要です（`docker buildx version` で確認します）。タスク専用のプラグインは、`DOCKER_CONFIG=~/shinkirou-apple/docker` の `cli-plugins/docker-buildx` パスに置けます。[Buildx v0.38.1 公式の Linux x86_64 アセット](https://github.com/docker/buildx/releases/download/v0.38.1/buildx-v0.38.1.linux-amd64)の SHA256 は `e25d5cd675092bb5d1d03632fdc46cdf3fede7b514dc5f023a105e00d3ce5ad4` です。照合してから実行権限を付与してください。sudo は不要で、既存の Docker 設定も変更しません。

```sh
export CARGO_HOME="$HOME/shinkirou-apple/cargo"
export RUSTUP_HOME="$HOME/shinkirou-apple/rustup"
export CARGO_TARGET_DIR="$HOME/shinkirou-apple/target"
export PATH="$HOME/shinkirou-apple/tools/bin:$PATH"
export DOCKER_CONFIG="$HOME/shinkirou-apple/docker"
rustup toolchain install 1.99.1 --profile minimal
rustup target add --toolchain 1.99.1 x86_64-apple-darwin aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cargo +1.99.1 install --locked --root "$HOME/shinkirou-apple/tools" cross --version 0.2.5
cargo +1.99.1 install --locked --root "$HOME/shinkirou-apple/tools" apple-codesign --version 0.29.0
export SHINKI_APPLE_SDK_DIR="$HOME/shinkirou-apple/sdks/macosx.sdk"
cargo xtask apple-image --target aarch64-apple-darwin
cargo xtask build --workspace engine --target aarch64-apple-darwin -p roukaku-player --release
cargo xtask cross --target aarch64-apple-darwin check -- -p roukaku-core
```

イメージはターゲットごとに作成し、iOS をビルドするときは SDK ディレクトリを切り替えます。イメージは Rust 1.99.1 に固定され、macOS には固定 revision の [osxcross](https://github.com/tpoechtrager/osxcross)、iOS には LLVM/LLD を使います。ターゲット専用の C/C++、CMake ツールと、`build/cross/` の固定 vcpkg 入力から作る静的 libass も設定されます。ホスト用ツールは引き続き Linux のコンパイラを使います。ネイティブ依存関係のライセンスは `.work/apple-native/<target>` にエクスポートされ、イメージの識別情報が同じ場合にのみ再利用されます。ビルド JSON には Cargo が実際に返した `compiler-artifact.executable` が含まれ、パッケージ化ではこのパスを使います。target ディレクトリを推測することはありません。

### Player のリソースとテスト用パッケージ

共有の Apple factory は winit/Metal、AVFoundation によるデコード、CoreAudio による再生、有効化した iOS オーディオセッションを使い、AccessKit のセマンティクス、タッチ／IME 入力、サンドボックス内のアトミックなセーブ、永続的な許可付きのネイティブ Files ピッカーを組み込みます。iOS のメインスレッドは UIKit が保持し、VM の future は owner のワーカースレッドで実行されます。デスクトップのイベントポンプは使いません。iOS ではデスクトップ用の gilrs ゲームパッドバックエンドを有効にしません。

まず `ios-release` profile（`PlatformHostProfile::ios_release` に対応）を含むパッケージを Cook し、`roukaku package bundle --platform ios --ios-player <built-player>` でフラットなリソースを生成します。通常の package、target、profile、out の引数も指定してください。最後に、Cargo が返した実際の実行ファイルとリソースディレクトリをパッケージ化コマンドに渡します。

```sh
cargo xtask package --target aarch64-apple-ios --binary <compiler-artifact-executable> --resources .work/ios-resources --version 0.1.0-beta.1 --out .work/ios-player-test
```

macOS では ad-hoc 署名された `.app` と ZIP、シミュレーターではテスト用と明示した `.app` と ZIP、デバイスでは `Payload/*.app` を含む `adhoc-device-test.ipa` を出力します。パッケージ化では Mach-O のアーキテクチャ、デバイス／シミュレーターのプラットフォーム、依存関係の閉包を先に検査してから、コピーしたファイルを書き換えて署名します。Linux では `rcodesign` で署名し、Mach-O ごとに検証します。ネイティブの macOS では `codesign` を使います。ad-hoc 署名には provisioning profile、公証、App Store の識別情報は含まれません。デバイスへのインストールには有効な Apple の署名と provisioning が必要で、正式なインストールパッケージとしては扱えません。デバイス／シミュレーターのライフサイクルと実行時の適合性は別途検証します。

実際の成果物については、パスとサイズに加えて、`file`、`llvm-objdump --macho --dylibs-used --private-headers <executable>`、`rcodesign verify <executable>` の出力を残してください。正規の SDK がない場合、SDK を使うビルドと実際の署名の検証は行えません。


[ドキュメントのトップ](index.md)に戻るか、[クイックスタート](quick-start.md)に進んでください。
