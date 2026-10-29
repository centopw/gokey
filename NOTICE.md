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

The name "OpenKey" and its icon are used by the upstream project. GPL does not
grant trademark rights. The public name of this port is **not yet decided**
(decision DL-010); until the upstream author is contacted and a name is agreed,
this repository uses `openkey-rs` as an internal working name only and publishes
no binaries.

## Attribution obligations

- Keep this NOTICE and `LICENSE` in every distributed binary and source archive.
- Any modified version must remain GPL-3.0 and state that it is modified.
- Do not imply endorsement by the upstream author.
