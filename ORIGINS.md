# Origins

RoukakuEngine starts a new Git history. Its code and assets come from the AstraEngine monorepo by the same authors; the original history stays in that repository.

| Source | Revision | Used for |
|---|---|---|
| https://github.com/2018wzh/AstraEngine (`master`) | `1576884396ce7803c267dbc5119135cc7eb6a715` | `Engine/`, `Editor/`, `Examples/`, `scenarios/`, `Tools/LuauTypes`, `Tools/NativeVN`, `Tools/TsuiNoSora`; `Engine/Tools/xtask` became `xtask/` |
| same repository, branch `codex/cmvs-independent-host-20261011` | `8b6a31f6123ec14cfb9236ee3b7c9095b7c8ce66` | `Engine/Source/Runtime/roukaku-media` FFmpeg frame-rate fallback |

The emulator half of that monorepo (`Emulator/`, its tools and metadata) moved to [ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU).

All own identifiers were renamed (`astra-*` → `roukaku-*`, `AstraVN` → `RoukakuVN`, `.astra*` → `.roukaku*`, package magic `ASTRACT2`/`ASTRAF2` → `ROUKACT2`/`ROUKAF2`). Containers and files written by AstraEngine are not readable by RoukakuEngine.

Third-party assets keep their own licenses next to the files (for example the OFL texts under `Examples/NativeVN/Assets/Fonts` and `Engine/Fixtures/PublicDomainFonts`, the CC0 notice in `Engine/Fixtures/PublicDomainMedia`).
