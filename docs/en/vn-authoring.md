---
title: VN authoring
description: Author visual novels with project manifests, .roukaku, Luau, localization, and asset sidecars.
sidebar: { order: 5 }
---

# VN authoring

Use `Examples/NativeVN` as the public reference project. Story and UI sources use `.roukaku`, controllers use `.luau`, and themes use JSON/YAML. Cook compiles these inputs into a product package. Runtime executes compiled story data, not `.roukaku` source text.

## Project entry point

`project.yaml` uses `roukaku.target_manifest.v3`. It declares product targets, profiles, platform host profiles, NativeVN source directories, and additional package sections. The sample includes this NativeVN block:

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

This is an excerpt, not a standalone buildable project. The target `id` matches CLI `--target`; `--profile` selects eligible content. Platform profiles explicitly select renderer, decode, audio, and save providers. Changing the platform name does not fix incompatible bindings.

## Story and presentation

`.roukaku` uses indentation and named attributes. This fragment follows the sample's structure and localization keys:

```text
story main #@id story.main
state experience_read #@id state.experience_read
  scene reading #@id scene.experience.reading
    text key:experience.read.first window:main #@id experience.read.first
    text key:experience.read.second window:main #@id experience.read.second
```

Keep stable `#@id` annotations for diagnostics, source maps, Editor selection, and attribute editing. Organize content with `state` and `scene`, use `choice`/`option` and `jump` for control flow, and reference assets by `asset:/...` ID. Layers, cameras, show/background commands, and timelines define presentation. Timeline tracks declare target properties, keyframes, easing, and join policy. Consult `Scripts/experience.roukaku` for working syntax rather than treating unimplemented DSL designs as compiler input.

Graph and Timeline modify these same sources, not a second runtime graph. VN Core owns story variables, choices, backlog, read state, and save semantics.

## UI and Luau

In `UI/flagship.roukaku`, `ui_view` defines widget trees, while `ui_bind` binds a profile's message, choice, or system page to a view, controller, policy, and theme. Widgets read `$model` data and send typed actions such as `vn.advance` and `vn.choose`.

The product's Luau global is **`roukaku`**. The sample controller registers handlers with `roukaku.ui.controller.register(...)` and forwards UI actions with `roukaku.ui.effect.forward(action)`. `nativevn.ui_controllers` collects these files; controllers do not use the `.roukaku` extension. Policies likewise use the mechanisms exposed through `roukaku` for commands and mutations. Changing private Lua tables does not change authoritative VN state.

Use existing system pages and models for saves, settings, and history. Do not implement a second save container or story interpreter inside a controller.

## Localization

Story text refers to stable keys. For example, `Localization/en.json` has this shape:

```json
{
  "schema": "roukaku.vn.localization_table.v1",
  "locale": "en",
  "strings": {
    "experience.read.first": "The last train has left, but a reflection remains."
  }
}
```

Declare each file in `package_sections` as `vn.localization.<locale>` with schema `roukaku.vn.localization_table.v1` and the relevant target/profile selections. `nativevn.default_locale` selects the default. Cook rejects a project whose default locale lacks a profile-eligible localization section. Keep keys consistent across translations; UI labels can use `l10n:` references.

## Assets and sidecars

Place `.roukaku-asset.yaml` files inside declared `asset_roots`. Each records a stable asset ID, project-relative `source`, `source_hash`, type, license, importer, cook processor, and target profiles. `AssetSidecars/` describes assets; original images, audio, video, and fonts remain separate inputs, referenced by asset ID in authoring sources.

Use the existing importer to update hashes and metadata after changing an asset. Do not retain stale hashes or invent permissions. Editor's Import image supports PNG/JPEG/WebP and generates sidecars; see [Editor](editor.md). Preserve redistribution licenses, sources, and authors. Commercial game text, keys, and saves do not belong in the project repository.

## Check and play

```sh
cargo run --locked -p roukaku-cli -- ui check Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game
cargo xtask ui-check
```

A single-file syntax check does not replace cross-file project validation. After editing, follow the [Quick start](quick-start.md) Cook, package, and Player pipeline and check actual text, layout, media, and save/restore behavior.
