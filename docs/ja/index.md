---
title: RoukakuEngine ドキュメント
description: RoukakuEngine、RoukakuVN、Player、Editor、CLI の役割分担。
sidebar: { order: 1 }
---

# RoukakuEngine

RoukakuEngine は、ネイティブ 2D 作品とビジュアルノベルの制作・実行に使います。共有ランタイム層がシーン、タスク、クロック、入力、文字、メディア、リソース管理を提供し、RoukakuVN はその上でシナリオ、選択肢、演出、システムページ、セーブ／ロードを実装します。

- **RoukakuPlayer** は作品の実行ホストです。パッケージ化されたデータを読み込み、ウィンドウ、デバイス、プラットフォームイベントを管理します。
- **RoukakuVN** はビジュアルノベル向けの製品モジュールです。作者が書いた `.roukaku` ソースを Cook でコンパイルし、Player とプレビューが共通で使う `VnSession` が実行します。
- **RoukakuEditor** はテキスト、ストーリーグラフ、タイムライン、Details、独立した Player プレビューを提供します。パネルでの編集は同じソースに書き戻されます。
- **`roukaku`** は CLI です。スクリプトと UI のチェック、Cook、パッケージ作成、プラットフォーム別 bundle を担当します。

ShinkiEMU は、ネイティブの旧エンジンを動かすための独立した製品です。固定した Git revision の RoukakuEngine 共有ライブラリを使います。RoukakuEngine は ShinkiEMU に依存しません。

## はじめに

1. [クイックスタート](quick-start.md)：ツールをビルドし、公開サンプル `Examples/NativeVN` をパッケージ化して Player で起動します。
2. [ビルド](build.md)：Rust とネイティブ依存関係、workspace チェック、Android と macOS のクロスコンパイル。
3. [アーキテクチャとインターフェース](architecture.md)：crate の責務、所有権、予算、キャンセル、データ完全性。
4. [VN 制作](vn-authoring.md)：プロジェクト、シナリオ、UI、Luau、ローカライズ、アセット。
5. [Editor](editor.md)：編集、保存、インポート、製品プレビュー。

設定でプラットフォームを宣言しても、そのプラットフォームでウィンドウ、入力、音声、セーブ／ロードが実機検証済みであることにはなりません。具体的なビルド手順は[ビルドドキュメント](build.md)と現在の manifest に従ってください。

[中文](../zh-cn/index.md) · [English](../en/index.md)
