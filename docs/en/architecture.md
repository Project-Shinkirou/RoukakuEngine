---
title: Architecture and interfaces
description: Crate ownership, sessions, resource budgets, cancellation, packages, and save integrity.
sidebar: { order: 4 }
---

# Architecture and interfaces

## Crate map

| Directory | Responsibilities and key crates |
| --- | --- |
| `Engine/Source/Runtime` | Foundation types in `roukaku-core`; scenes, tasks, and sessions in `roukaku-runtime`; containers in `roukaku-package`; assets and bounded reads in `roukaku-asset` and `roukaku-byte-source`; shared text, media, subtitle, UI, and Player mechanisms |
| `Engine/Source/Platform` | Host contracts in `roukaku-platform`, shared host implementation in `roukaku-platform-common`, platform backends, and project/target validation in `roukaku-target` |
| `Engine/Source/Developer` | Import and Cook in `roukaku-cook`, release-data validation in `roukaku-release`, diagnostics in `roukaku-observability`, and Headless testing tools |
| `Engine/Source/Programs` | `roukaku-cli` (executable `roukaku`), desktop/Android/Web Player, Headless, Crash Reporter, and Player VN integration |
| `Engine/Source/Modules/RoukakuVN` | Script compilation, story semantics, policy, presentation, UI, package sections, editing transactions, and the typed VN product entry point |
| `Editor/Source/roukaku-editor` | The separate GPUI application workspace, project panels, external Agent/MCP integration, and Player preview process ownership |

Within RoukakuVN, `roukaku-vn-script` compiles `.roukaku` and produces source maps. `roukaku-vn-core` owns the story cursor, variables, choices, waits, backlog, and system state. `roukaku-vn-policy` hosts Luau; `roukaku-vn-presentation` owns staging and presentation; `roukaku-vn-package` owns product sections; `roukaku-vn-ui` owns UI models; `roukaku-vn-editor` provides GPUI-independent editing transactions. `roukaku-vn::VnSession` composes the product.

## Sessions and platform ownership

`roukaku-runtime::EngineSession` owns the shared World, task scopes, and logical-step lifecycle. `VnSession` additionally owns VN story and system state. Player and Editor preview use the same product session, not separate story interpreters. Desktop Player schedules logical steps at approximately 60 Hz while handling presentation and platform events independently.

A platform host profile explicitly selects renderer, decode, audio mixer, audio output, save providers, and package sources. Failed explicit bindings report errors rather than presenting a different backend as successful. Consumers can compose shared text, media, and byte-source libraries without first creating a VN session.

## Enforced runtime protections

These are product behavior and must survive refactoring:

- **Budgets:** host profiles validate nonzero queue, frame, audio, PCM-cache, and package-read limits. Presentation, decode requests, saves, and package range reads check applicable sizes at entry. Containers separately bound section counts, table size, and decoded lengths. Cook batches bound nodes and input bytes. These protect distinct failure boundaries; they are not a universal memory quota.
- **Paths:** project assets and sections use validated relative paths. Bundling validates destination paths, and scenario references reject invalid relative paths. Editor asset import checks that resolved directories remain inside the project and rejects duplicate paths and asset IDs. Path validation does not grant arbitrary disk access.
- **Cancellation:** tasks carry an owner, scope, and handle. Completion from cancelled, finished, or different-World contexts is rejected. Session shutdown cancels owned work; restoring state replaces the task context so late results cannot commit. Cook uses a shared cancellation token. Switching Editor projects invalidates old compiler, import, Agent, and preview work.
- **Save integrity:** saves contain explicit runtime and VN state, not native handles or execution stacks. Loading decodes and validates a candidate before replacing the live World. Invalid containers or state leave live state unchanged. Successful restoration also rebuilds task context. Ignoring errors would break this boundary.

## Packages and formats

`.roukakupkg` and `.roukakupak` are used for product packages and preview intermediates. The `roukaku-package` v2 container has the eight-byte header magic `ROUKACT2` and eight-byte footer magic `ROUKAF2\0`. Readers validate kind, version, lengths, section ranges, and header, table, and payload hashes. Lazy section reads still verify stored data.

Pre-rename product containers, schemas, and project-owned saves are not compatible inputs. Old magic is rejected at entry; changing an extension does not convert content. Re-cook and package current authoring sources, leaving old files and user data untouched.

### VFS overlays

Player resolves catalog asset IDs through the package VFS manifest. Package sections, overlay packages, and host-authorized local directories share the same priority rules; equal-priority matches fail as ambiguous, and an applicable whiteout hides lower-priority content. Local paths are read only through bounded catalog entries rooted in directories the host explicitly authorized. Asset cache identity includes the asset ID, selected source layer, and content hash, so a changed overlay revision replaces only that asset's decoded data.

## Luau authoring mechanism contract (R1/R3 foundation)

`roukaku-vn-policy::authoring::LuauAuthoringRuntime` runs project source on the official Luau VM through the existing `luau-runtime`/mlua feature. This integration API supplies mechanisms independently of the default VN UI. Player/Cook and the existing DSL remain unchanged in this branch; the product cutover consumes this API.

### Paragraphs, modules and input

The host constructs the runtime with `PolicyExecutionBudget`, an owner `TaskScope`, and `Rc<dyn AuthoringHost>`. `replace_module(module, source_name, source)` evaluates a source chunk returning an export table. Use a real source filename such as `@Story/chapter1.luau` for Luau errors and stack traces. Each module has a read-only global environment; use local variables and explicit registered state.

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
        -- Entry tables are also supported: intro = { run = function() ... end }
    },
}
```

`start_paragraph(ParagraphId { module, entry })` resolves `exports.paragraphs[entry]` as a function or `{ run = function }`. This pair is the stable restart/load identity. Restart cancels old owned segments and input, starts a fresh child scope/coroutine, and resets node visits. Installing entry-state and choosing when to replay are the save/replay consumer's responsibilities.

`rk.wait_input(action: string): any` suspends only the story coroutine. The host sees `StoryStatus::Waiting(InputWait { token: u64, action: String })`; `input(token, action, json_payload)` resumes only a matching live wait. Unrelated or late input returns `false`. Live tokens remain distinct across paragraph restarts and runtime replacement. A wait may originate only from the active story coroutine. Presentation keeps advancing through `advance_presentation(seconds)` while the story waits. No host thread, audio clock or renderer waits for input.

`rk.call(module: string, name: string, ...): ...` resolves an export at each call and supports yielding project functions. Timeline callback references use `{ module: string, name: string }` and also resolve at dispatch. Replacing a module changes subsequent calls; an already suspended frame retains its old function. A failed chunk does not replace the export table; any explicit mechanism side effects performed before that error remain. Incompatible changes require cancelling and restarting the paragraph.

### Nodes, text and registered state

| Mechanism | Types and semantics |
| --- | --- |
| `rk.line(node, data)` | Records a line with plain project data and returns `NodeOccurrence = { paragraph = { module, entry }, node, visit }`. It neither displays UI nor waits. |
| `rk.choice(node, options)` | Records `{ id: string, text: string }[]`, with nonempty distinct option IDs, and returns the same occurrence type. |
| `rk.choice_result(occurrence, selected)` | Commits one valid option ID to that choice occurrence exactly once. |
| `rk.measure_text(request)` | Accepts the complete serialized `roukaku_text::TextLayoutRequest`, including direction, Ruby, font chain and constraints; returns `{ width, height, clipped, ellipsized, revision }`. |
| `rk.display_text(id, request)` | Uses the same layout provider and returns the same measurement. `AuthoringHost::display_text` receives `Arc<TextLayoutResult>` with glyph resources and source/cluster mapping for drawing. |
| `rk.register_state(scope, key, initial)` | Registers a nonempty state key and returns its existing value, or the initial value on first registration. Repeating a registration preserves recovered data. |
| `rk.get_state(scope, key)` / `rk.set_state(scope, key, value)` | Read/write registered keys; unknown keys fail. Scopes are `"variables"`, `"scene"` and `"extensions"`. |

Visits are one-based counters **per source node within a paragraph invocation**. Loop visits have distinct identities, while replaying the same path from the same entry reproduces them. Restart creates fresh live wait/segment tokens even when logical node identities repeat. Replacing dialogue UI cannot remove these records; history/replay consumers read `records()`.

Recoverable values are finite JSON-like plain data (including arrays and null), subject to depth/host-memory limits. Functions, threads, userdata handles, arbitrary metatables and cyclic tables are rejected. `recoverable_state()` returns explicit variables/scene/extension data. `restore_registered_state(candidate)` validates before installation and requires no live story coroutine or running segment; it does not recreate native objects or resume a stack. Stage property values are also retained as `scene[target][property]`; targets must be scene objects, so project state must not use a non-object at that key. Never serialize arbitrary Luau stacks.

### Declarative presentation and terminal ownership

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

`rk.segment(declaration: Timeline): number` creates a scoped segment and evaluates time zero. Times/durations are finite seconds; values are finite numbers. Each target/property appears once, keys increase strictly inside `[0, duration]`, and values interpolate linearly with endpoint clamping. `TimelineDeclaration::evaluate(time, apply)` provides pure arbitrary-time sampling for editing/preview: no callbacks or host mutation. Project effects, media or free logic use referenced callbacks and host-owned properties rather than serializing closures into declarations.

`rk.set_property(target, property, value)` applies a finite numeric property through `AuthoringHost::set_property` and updates recoverable scene data. `advance_presentation(delta)` evaluates active tracks independently of story input. Timed callbacks run once when reached, in declaration order. Lifecycle callbacks are synchronous and must not yield.

`rk.complete(id)` (or host `complete_segment`) claims `Completed`, applies every track's end state, dispatches the `complete` reference for each timed callback not already dispatched, then `on_complete`, and releases segment resources. Normal playback dispatches reached `call` references and completes at duration. `rk.cancel(id)`/`cancel_segment` claims `Cancelled`, stops at the current state, invokes `on_cancel`, and releases resources without applying end values or required completion results. An already terminal segment returns `false`; the first transition returns `true`. Cancellation after owner-scope invalidation releases native resources without executing project code in that dead scope.

Terminal ownership is claimed before user callbacks. Callback failures propagate and cannot be retried to duplicate effects; release is attempted even on failure. An unhandled story or presentation-tick failure cancels the remaining owned work. Parent cancellation, paragraph restart, explicit cancellation and runtime drop all release owned segments. `AuthoringHost::release_segment(id)` is called once per terminal segment, for both completion and cancellation.

The VM retains the existing host-IO sandbox and Lua memory limit. Instruction and output allowances reset only at host-controlled dispatch boundaries. Host-owned data is conservatively charged for the runtime lifetime, including retained records, declarations and registered-state copies; budget exhaustion is sticky. Updating an existing numeric stage property does not charge another stored copy. Create a new runtime after exhausting its allowance. No script API resets budgets or detaches work from the owner scope.


## One-way sharing with ShinkiEMU

ShinkiEMU consumes `roukaku-core`, `roukaku-byte-source`, `roukaku-media-core`, `roukaku-text`, `roukaku-media`, `roukaku-subtitle`, `roukaku-platform`, `roukaku-platform-common`, and `roukaku-observability`. Production dependencies are centralized in the ShinkiEMU workspace and pinned to one RoukakuEngine Git revision. They must not rely on adjacent developer directories or copied Rust implementations.

The dependency direction is **ShinkiEMU → RoukakuEngine** only. RoukakuEngine and its shared crates must not depend on ShinkiEMU, its Manager, or its core ABI. Shared mechanisms stay in their existing owner crates; story rules, native engine formats, and product behavior remain product-owned.

See [Build](build.md), [VN authoring](vn-authoring.md), and [Editor](editor.md).
