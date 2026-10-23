---
title: Editor
description: Open projects, edit canonical sources, save safely, import assets, and preview in real Player.
sidebar: { order: 6 }
---

# RoukakuEditor

Editor is a GPUI desktop application in a separate workspace. Manual authoring requires neither an Agent nor a model service.

## Open a project

Prepare dependencies using [Build](build.md), then run from the repository root:

```sh
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
```

A single `.roukaku` file is also accepted. Project mode loads declared story, UI, theme, and Luau controller sources. Use Open project to switch projects. Unsaved changes are handled first; a failed open preserves the current project.

## Edit and save

Use Content Browser to find sources and declared assets, Outliner to select commands, and Details to edit attributes. Text, Graph, and Timeline operate on the same versioned sources. Stable source IDs connect selections and diagnostics to author input.

- **Graph** shows actual states and routes. Drag a state's connection port onto a target state to add a jump. Delete route removes its control-flow command. Nodes currently use a fixed grid, without free layout dragging.
- **Timeline** shows real keyframes and supports changing times/values, inserting, deleting, and dragging within the same track. Releasing a drag commits one undoable batch. Stale versions and duplicate times are rejected. Existing linear/ease_in/ease_out/ease_in_out modes are supported; free curve editing and time zoom are not implemented.
- **Save / Ctrl+S / Cmd+S** saves all source documents. Each file is written through a temporary file and replaced. External disk changes cause a conflict rather than being overwritten. Files are saved sequentially: IO failures are reported, without a cross-file atomicity guarantee.
- **Undo batch / Redo batch** uses shared editing history; undo also advances document versions. Text, attributes, and external edits undergo expected-version and UTF-8 range validation. Returning to identical text does not make an old result valid again.

Compiler diagnostics show source locations and support navigation. Results for an old project or document version cannot replace current diagnostics. Closing a dirty project offers cancel, save, or discard.

## Import images

Content Browser's Import image supports PNG, JPEG, and WebP. Select a project-relative destination, asset ID, license, and configured sidecar root. A background `roukaku-cook` importer decodes the image, computes its hash, and writes the image and `.roukaku-asset.yaml`.

Duplicate paths or IDs are rejected, and destination directories must remain inside the project. Import writes assets immediately and is not part of `.roukaku` undo history. Single-file mode requires opening a full project first. Audio, video, and font import forms and asset thumbnails are not available.

## Configure Player preview

Build CLI, Player, and the Windows Crash Reporter using [Quick start](quick-start.md). Preview configuration is local JSON:

| Field | Meaning |
| --- | --- |
| `project` | Current `project.yaml` path |
| `cli` | Built `roukaku` executable |
| `player` | Built `roukaku-player` executable |
| `profile` | Preview profile, such as `advanced-vn` |
| `target` | Product target ID, such as `nativevn-flagship-game` |
| `windows_runtime` | Required on Windows: matching redistributable Microsoft VC x64 CRT directory |
| `crash_reporter` | Required on Windows: built `RoukakuCrashReporter.exe` |

Use absolute paths into the current checkout and keep the file ignored. This PowerShell example generates configuration from the repository root after you set `$env:VC_CRT_REDIST`:

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

This example requires PowerShell 7 to write BOM-free UTF-8 JSON. On Linux/macOS, use the same fields pointing to native executables and omit the two Windows fields. Preview setup can load this configuration in an already-open Editor.

Save & Preview compiles and saves current sources, then runs `cook → package build → package bundle → Player` in a separate GPU window. Intermediates live under the project's `.roukaku-cache/preview`. Pause/Resume appears only after Player returns Ready. Paused seeking uses checkpoints actually retained in the current fragment. Stop preview requests real session shutdown before terminating an unresponsive child.

Source edits, project switches, a new preview, or Editor shutdown invalidate the old preview and reclaim its process. Failures show exit status and bounded logs. Compilation alone does not verify rendered output or audible audio.

## Optional external Agent

`--agent` accepts an external ACP Agent command. The Agent owns model configuration and authentication. `--mcp` starts stdio MCP with `read_document` and `apply_batch`, accessing the same unsaved document. Batch review is the default: choose Apply batch or Reject batch. Autonomous mode retains the same version, range, and cancellation checks. Cancellation or manual editing rejects old-generation results.

Editor does not implement a model request loop or provide a general sandbox for external processes. Manual editing and Player preview work independently.

Continue with [VN authoring](vn-authoring.md) and [Architecture](architecture.md).
