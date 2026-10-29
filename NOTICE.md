# NOTICE

This project is a derivative work of **OpenKey**, an open-source Vietnamese
input method.

- Upstream: https://github.com/tuyenvm/OpenKey
- Author: Mai Vu Tuyen (mai vutuyen)
- License: GNU General Public License v3.0 (see `LICENSE`)

The keystroke engine in `crates/openkey-core` is a port of the C++ sources in
`Sources/OpenKey/engine` (upstream `Engine.cpp`, `Vietnamese.cpp`, `Macro.cpp`,
`ConvertTool.cpp`, `SmartSwitchKey.cpp`, `DataType.h`). Behaviour is intended to
match upstream, including documented quirks; the parity corpus in
`tests/corpus/` is derived from upstream documentation and observed behaviour.

Because OpenKey is GPL-3.0, this port is distributed under GPL-3.0 as a whole.
Platform front-ends are original Rust code but remain bound by the same license.

## Name and marks

The name "OpenKey" and its icon belong to the upstream project. GPL does not
grant trademark rights.

The public name of this port is **GoKey** (Vietnamese: "Gõ Key"), decided in
DL-010. "OpenKey" appears here and in the About dialog as attribution only —
never as the product name, package name or icon. Crate names keep the
`openkey-` prefix so the port stays traceable to its upstream sources.

Binaries **are** produced: every merge to `main` builds an ad-hoc signed macOS
`.dmg` as a CI artefact (K-007, ADR-0008). Those artefacts are public downloads on
a public repository, so each one carries this NOTICE, `LICENSE` and a
`SOURCE-OFFER.txt` naming the exact commit, and the source archive for that commit
is attached beside it. That is a GPL-3.0 distribution and is treated as one.

## Attribution obligations

- Keep this NOTICE and `LICENSE` in every distributed binary and source archive.
- Any modified version must remain GPL-3.0 and state that it is modified.
- Do not imply endorsement by the upstream author.
- Published artifacts, releases and package identifiers are owned by
  `centopw` (`hiep@hce.vn`), per DL-020.
