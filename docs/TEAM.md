# Team

Roles, not named people. Staffing decisions belong to the sponsor; the PM
maintains this roster and flags gaps.

## 1. Composition

| Code | Role | FTE | Joins | Must be able to | Owns |
| --- | --- | --- | --- | --- | --- |
| R1 | Program manager (this Waker role) | 0.5 | M0 | Plan, risk, dependency tracking, stakeholder comms | Plan, risks, decisions, status |
| R2 | Technical lead / Rust architect | 1.0 | M0 | Rust systems design, API ergonomics, code review authority | ADRs, core API, crate graph |
| R3 | Core engineer — engine | 1.0 | M0 | State machines, Unicode, careful transliteration of C++ | Telex/VNI/Simple Telex, spelling, options logic |
| R4 | Core engineer — data | 1.0 | M0 | Tables, encoding, serde, data-driven tests | Charsets, macros, convert tool, settings/migration |
| R5 | QA / parity engineer | 1.0 | M0 | Test harness engineering, fuzzing, differential testing | Reference harness, corpus, compat matrices |
| R6 | Build and release engineer | 0.75 | M0 | CI, cross-compilation, signing, notarisation, installers | Pipelines, artifacts, signing keys process |
| R7 | macOS platform engineer | 1.0 | M0 (spike) | Objective-C/Swift + Rust FFI, CGEventTap or InputMethodKit | macOS adapter, menu bar, packaging |
| R8 | Windows platform engineer | 1.0 | M2 | Win32, `windows-rs`, hooks, SendInput, MSI | Windows adapter, tray, installer |
| R9 | Linux platform engineer | 1.0 | M3 | C ABI, IBus or Fcitx5, Wayland behaviour, distro packaging | Linux IBus engine, packaging |
| R10 | Product/UI designer | 0.5 | M1 | Desktop utility UX, Vietnamese and English copy | Settings UI, tray/menu, onboarding, docs copy |
| R11 | Vietnamese linguistics reviewer | 0.2 | M0 | Telex/VNI rules, orthography, spelling conventions | Corpus review, rule sign-off, edge-case adjudication |
| R12 | Security and privacy reviewer | 0.1 | Gates | Threat modelling for input methods, permission UX | Permission model, telemetry stance, release sign-off |
| R13 | OSS / community liaison | 0.2 | M0 | GPL compliance, upstream relations, community process | Naming, NOTICE, contribution guide, upstream contact |

## 2. Phase 0 kickoff roster (must start now)

Five people plus two part-time advisors. This is the minimum to make G0 in two
weeks.

| Code | Week-1 assignment | Deliverable by end of W2 |
| --- | --- | --- |
| R1 | Kick off: confirm assumptions, book sponsor review, set up tracker and cadence | Charter approved, DL-003 decided, backlog imported |
| R2 | Ratify crate graph and core API; write ADR-0002; review CI | ADR accepted, key normalisation table drafted |
| R3 | Read `Engine.cpp`; produce function-to-module porting map; design word model | Porting map, C-001 started |
| R4 | Extract charset tables and option defaults from upstream sources | Table extraction plan, defaults audit |
| R5 | Build headless reference engine (K-003), start corpus v0 (K-004) | Ref binary replays a script; >=100 cases |
| R6 | Make CI green on three OSes, repo settings, start packaging spike | Green pipeline, first unsigned artifacts |
| R7 | macOS adapter spike (timebox 5 days): CGEventTap prototype | ADR-0004 recommendation |
| R11 | Review corpus format; supply 50 spelling/orthography cases | Reviewed corpus rules |
| R13 | Contact upstream author about name and attribution; draft NOTICE | NOTICE in repo, contact logged |

## 3. RACI

R = responsible, A = accountable, C = consulted, I = informed.

| Workstream | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 | R9 | R10 | R11 | R12 | R13 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Scope and schedule | A | C | I | I | I | I | I | I | I | I | I | I | I |
| Core API and architecture | I | A/R | C | C | C | I | C | C | C | I | I | C | I |
| Typing engine | I | A | R | C | C | I | I | I | I | I | C | I | I |
| Charsets, macros, settings | I | A | C | R | C | I | I | I | I | I | C | I | I |
| Parity and test strategy | I | A | C | C | R | I | I | I | I | I | C | I | I |
| macOS adapter | I | A | I | I | C | C | R | I | I | C | I | C | I |
| Windows adapter | I | A | I | I | C | C | I | R | I | C | I | C | I |
| Linux adapter | I | A | I | I | C | C | I | I | R | C | I | C | I |
| Packaging and release | C | C | I | I | C | A/R | C | C | C | I | I | C | I |
| UX and docs | C | C | I | I | I | I | C | C | C | A/R | C | I | I |
| Licensing and naming | C | I | I | I | I | I | I | I | I | I | I | C | A/R |
| Security and privacy | C | C | C | C | C | C | C | C | C | C | I | A/R | I |

## 4. Hiring and sourcing notes

- R3/R4 are the highest-leverage hires: they must read C++ fluently and write
  idiomatic, test-driven Rust. Prefer people who have ported code before.
- R5 is not a "tester" role; it is a tooling engineer who owns the differential
  harness. Without it, parity claims are unverifiable.
- R7/R8/R9 can be contractors if staff engineers are unavailable, but each
  adapter needs a named internal reviewer (R2 or the other platform engineer)
  so knowledge does not leave with the contractor.
- R11 can be a paid advisor on retainer; 4 hours per week is enough during M1,
  more during corpus review weeks.

## 5. Bus factor mitigation

- Every crate has an owner and a named secondary reviewer.
- Adapter design decisions must be written in ADRs, not just in code review.
- No single person holds signing keys or release steps; R6 documents and R2
  holds the backup role.
- Pair on the two riskiest items: reference harness (R5 + R3) and macOS event
  tap (R7 + R2).

## 6. Onboarding checklist (first day per engineer)

1. Read `docs/CHARTER.md`, `docs/ARCHITECTURE.md`, `docs/WORKFLOW.md`.
2. Install the toolchain from `rust-toolchain.toml`; run `cargo test --workspace`.
3. Run `cargo run -p openkey-cli` and the parity script once it exists.
4. Read the upstream C++ file(s) for your area; note surprises in the issue tracker.
5. Claim one task from `backlog.csv` in your milestone; open a draft PR early.
6. For platform roles: get a reference machine image with the OS versions in
   the charter, plus a signing/notarisation account request into R6.

## 7. Decision rights

| Decision | Decides | Consulted |
| --- | --- | --- |
| Scope, dates, staffing | Sponsor | R1, R2 |
| Architecture, crate boundaries, API | R2 | R3, R4, R7/R8/R9 |
| Parity exception (deliberate divergence) | R2 + R11 | R1, sponsor if user-visible |
| Public name, license, upstream relations | Sponsor + R13 | R2, R12 |
| Release go/no-go | R1 + R12 | R2, R5, R6 |
| Adding a platform or a v2 feature | Sponsor | R1, R2 |
