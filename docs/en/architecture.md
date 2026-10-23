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

## One-way sharing with ShinkiEMU

ShinkiEMU consumes `roukaku-core`, `roukaku-byte-source`, `roukaku-media-core`, `roukaku-text`, `roukaku-media`, `roukaku-subtitle`, `roukaku-platform`, `roukaku-platform-common`, and `roukaku-observability`. Production dependencies are centralized in the ShinkiEMU workspace and pinned to one RoukakuEngine Git revision. They must not rely on adjacent developer directories or copied Rust implementations.

The dependency direction is **ShinkiEMU → RoukakuEngine** only. RoukakuEngine and its shared crates must not depend on ShinkiEMU, its Manager, or its core ABI. Shared mechanisms stay in their existing owner crates; story rules, native engine formats, and product behavior remain product-owned.

See [Build](build.md), [VN authoring](vn-authoring.md), and [Editor](editor.md).
