## What this PR does

<!-- One paragraph. Link the backlog/tracker ID, e.g. C-002. -->

Task:
Milestone:

## Type

- [ ] feat
- [ ] fix
- [ ] chore / refactor
- [ ] spike (throwaway, does not merge to main)
- [ ] adr / docs

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] Parity: corpus cases added/updated, or "no behaviour change"
- [ ] `cargo deny check` (if dependencies changed)

## Risk and rollback

<!-- Platform permissions, packaging, data migration, user-visible behaviour. "None" is a valid answer. -->

## Reviewer notes

- [ ] New `unsafe` blocks have a `// SAFETY:` comment and an owner check
- [ ] No telemetry, no typed content logged or persisted
