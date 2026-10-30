# ADR-0010 — Release path: ad-hoc signed universal dmg, no Apple credentials

- Status: accepted
- Date: 2026-10-30
- Owner: R6 (DevOps)
- Reviewers: R2 (Backend) for DL-025; Project Administrator for scope
- Decided by: Hiep, D1–D7 (2026-10-30)
- Supersedes: nothing. Extends ADR-0008 §4 (tier 6, release) and replaces the
  Developer ID + notarisation assumption that ADR-0008 and MAC-006 carried.
- Backlog: REL-001 (release path), REL-003 (update channel, still open)

## Context

ADR-0008 designated tier 6 as "the release" and left it undesigned. The packaging that
exists is a CI artefact: `package-macos.yml` builds an ad-hoc signed dmg on every push to
`main`, asserts the bundle layout, and uploads it for 14 days. Nobody is meant to install
it. A release was to be the same bundle signed with a Developer ID certificate and
notarised, which is the only way a macOS download opens without ceremony.

That premise does not hold. On 2026-10-30 the sponsor decided (D1, D2) that the Apple team
`U77P695XNT` is **not** a paid Developer Program membership and that no submission will be
made to Apple. Apple issues Developer ID certificates and accepts notarisation submissions
only from paid members, so both steps are impossible, not merely deferred. What remains is
the ad-hoc signature the packaging job already produces.

Four other decisions shape this:

- **D3 — shared identity.** Every agent pushes as `centopw`, and `centopw` is the only
  collaborator and a repository admin. There is no second account with which to satisfy a
  required review, so no approval gate can exist.
- **D4 — rulesets approved.** Tag-creation and deletion protection can be registered
  without deadlocking a single-account repository; required code-owner review cannot, and
  stays off until a second identity exists.
- **D5 — universal binary.** One dmg for Apple silicon and Intel.
- **D6 — this ADR and `release.yml` approved.** **D7** — the GoKey name stays.

The consequence users see: the first launch is refused by Gatekeeper and needs a manual
bypass. That is a product cost of D1, not a defect of the build, and it belongs on the
download page rather than in a wiki.

## Decision

### 1. Tag-triggered build, no credentials anywhere in the path

`release.yml` runs on a `v*` tag push and on `workflow_dispatch` (which takes an existing
tag as its only input). The build job declares no `environment`, references no secret, and
a step fails the run if the string `secrets.` ever reappears in the file. The publish job
holds `contents: write` and nothing else; the build job holds `contents: read`. The only
credential in the entire path is the job-scoped `GITHUB_TOKEN`.

There is no Apple identity, no `.p12`, no API key, no keychain, and no `notarytool` or
`stapler` invocation. `build-dmg.sh` keeps `--sign -` and `--options runtime`.

### 2. Four assertions before anything is published

The workflow refuses to build unless all four hold:

1. the tag exists and `refs/tags/<tag>^{commit}` **is** the commit being built;
2. that commit is an ancestor of `origin/main` (checkout with `fetch-depth: 0`);
3. the tag's numeric base equals the resolved version of `crates/openkey-cli`
   (`cargo metadata --no-deps --locked`), so a dmg cannot claim a version the tree does
   not have. A semver pre-release suffix is allowed — `v0.1.0-alpha.1` is tagged from
   0.1.0 — but `v0.9.9` is not. The bundle itself carries the numeric crate version,
   because `CFBundleShortVersionString` must be period-separated integers; the tag is
   the release identity, the plist is the machine identity;
4. the tag matches `v<digits>...`.

`build-dmg.sh` enforces the same tag/version agreement locally (`--tag v0.9.9 --version
0.1.0` exits 2, `--tag v0.1.0-alpha.1 --version 0.1.0` passes), so a hand-run release
cannot produce a mismatched `SOURCE-OFFER.txt`.

### 3. The artefact is asserted to be ad-hoc, universal, and **rejected**

The verification step mounts the dmg and checks, on the bundle a user actually receives:

- `lipo -archs` on the app's executable reports both `arm64` and `x86_64`;
- `codesign -dv` reports `Signature=adhoc`;
- the signature flags contain `(adhoc,runtime)` — hardened runtime is still on, and this
  assertion is what stops a future signed build from losing it silently;
- `spctl -a -t exec -vv <app>` exits non-zero and says `rejected`;
- `spctl -a -t open --context context:primary-signature -v <dmg>` does the same.

The last two are **expected failures that the job asserts**. If `spctl` ever accepts the
build, the job fails with "Gatekeeper behaviour changed; ADR-0010 and the release notes are
stale". A green release therefore means precisely *"macOS refuses this build, as documented"*
and can never be read as approval. Skipping the check would have left the opposite
impression available: that a green release job means Gatekeeper is satisfied.

### 4. Corresponding source comes from `git archive` of the tag

The release uploads `source.tar.gz`, produced by `git archive --format=tar.gz --prefix=
gokey-<tag>/ refs/tags/<tag>`, and hashes it in `SHA256SUMS` next to the dmg. `SOURCE-OFFER.txt`
names the tag, the commit, the architectures, the toolchain, and both the release URL and
the `archive/<sha>` URL (the latter as a convenience, not as the offer).

**Why not GitHub's `archive/<sha>.tar.gz` endpoint:** it is a server-side regeneration with
no byte-stability guarantee, and a release is a permanent obligation. A tarball produced
from the tag we built is the tree that produced the dmg, and it is reproducible: two runs
here produced identical SHA-256 (`01b6323c…`), 96 entries, with the gzip header MTIME field
zeroed.

Measured caveat, and the reason the ADR states a limit rather than a promise: byte-identity
holds across runs of the same git version (2.56.0 here). A git upgrade can change the
deflate output. The release notes record the git-independent facts (tag, commit, toolchain)
and nothing in this repository may claim cross-version reproducibility of the tarball.

The `source.tar.gz` is asserted to contain `LICENSE`, `NOTICE.md`, `Cargo.lock` and
`packaging/macos/build-dmg.sh`. There is no `.gitattributes`, so no `export-ignore` rule can
drop a file from the corresponding source without the assertion noticing.

### 5. Pre-releases until MAC-002

`gh release create --prerelease` is **hardcoded**. The payload is `openkey-cli`: a headless
engine harness, not an input method. A stable release would be a claim the artefact cannot
support, and the download page says so in its first sentence. Removing `--prerelease`
requires MAC-002 (the real InputMethodKit payload) plus an amendment to this ADR — it is a
product-scope change, not a workflow flag.

### 6. Release authority is the audit log

With a single shared identity there is no approval gate. `centopw` can create a tag, and a
tag runs the release. D4's rulesets restrict tag creation, but the same admin account can
adjust them, and there is no separate approver to require. Anything in this repository,
including a workflow comment, that implies a human approved a specific release is wrong.
The record is the GitHub audit log (tag creation, workflow run, release publication) and
the release page itself.

This is the honest version of D3. It is also why the ADR names its own supersession
conditions rather than pretending the gap does not exist.

## Consequences

- **Every user must clear Gatekeeper once.** Right-click → Open, or
  `xattr -dr com.apple.quarantine /Applications/GoKey.app`. The instructions are generated
  into the release notes from `.github/release-notes.md.in`, so they cannot drift from the
  build that shipped.
- **No revocation and no notarisation ticket.** The integrity anchor is the SHA-256 in the
  release plus TLS on the download. `SHA256SUMS` tells a user whether their download
  matches *this* release; it says nothing about whether the release is trustworthy, and it
  is not signed. An ad-hoc signature proves integrity of the sealed bundle, never identity:
  anyone can re-sign a modified bundle ad-hoc.
- **No stable identity to pin.** An ad-hoc cdhash changes on any repack, so nothing
  downstream can key off it.
- **Two packaging jobs now exist.** `package-macos.yml` stays the per-commit gate (bundle
  layout, time-to-artefact, 14-day artefact); `release.yml` is publication. Shared logic
  stays in `packaging/macos/build-dmg.sh`, which is the seam: a change there affects both
  and must be exercised by both.
- **Re-runs are manual.** `gh release create` fails if the release already exists.
  Re-publishing a failed run means deleting the release first, which is deliberately a
  human act. Release assets are mutable by admins; this ADR does not claim otherwise.
- **A version bump is now a coordinated act**: `Cargo.toml` and the tag must agree, and the
  build fails if they do not. That is the point.
- The universal build doubles the download (≈380 KB today) and needs both rust targets on
  the runner. Immaterial at this size.

## What would supersede this ADR

1. **A paid Apple Developer Program membership** — Developer ID + notarisation replace the
   ad-hoc signature. The `spctl` assertion inverts from "must be rejected" to "must be
   accepted", the release environment returns, and the install instructions lose their
   first section.
2. **A second GitHub identity** — an approval gate becomes satisfiable: the release
   environment with `prevent_self_review`, and D4's required code-owner review, can be
   turned on without deadlocking the repository.
3. **MAC-002 landing** — the payload stops being the stub, and §5's hardcoded
   `--prerelease` can be revisited.

## Alternatives considered

- **Notarise without a Developer ID certificate.** Rejected: Apple requires a Developer ID
  signature and a secure timestamp; `notarytool` rejects an ad-hoc build. Not a budget
  question, a validity question.
- **A self-signed certificate instead of ad-hoc.** Rejected: Gatekeeper treats it exactly
  like ad-hoc ("unidentified developer"), so it buys no user experience and adds a key that
  must be protected and rotated.
- **Ship a zip instead of a dmg.** Rejected: same Gatekeeper story, and it loses the
  mount-time content assertions and the `/Applications` symlink.
- **Publish only the GitHub `archive/` tarball as the source offer.** Rejected: no
  byte-stability guarantee; see §4.
- **Wait for MAC-002 before publishing anything.** Rejected: D2 asks for a downloadable
  artefact now, and a correctly labelled pre-release validates the path that the real
  payload will use. The label carries the honesty instead of the delay.
- **Keep an environment approval in front of `publish` even though it cannot be satisfied
  by anyone but the same identity.** Rejected: an approver and an author that are one
  account is theatre, and D4 already rejected a rule that would deadlock the repository.

## Verification

| what | how |
|---|---|
| tag/commit/version agreement | `release.yml` "Resolve the tag" step |
| universal binary, both slices, inside the dmg | `lipo -archs` on the mounted app |
| ad-hoc + hardened runtime | `codesign -dv` flags `(adhoc,runtime)` |
| Gatekeeper refuses the build | `spctl`, asserted to fail on dmg and app |
| corresponding source is complete | tarball contains LICENSE, NOTICE, Cargo.lock, build script |
| assets match their checksums | `shasum -a 256 -c SHA256SUMS` in the publish job |
| no credential crept in | `grep -n 'secrets\.'` fails the build job |
