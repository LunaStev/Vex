# Unreleased: state coordination and format migration

This describes the development branch, not an already published Vex release.
No release version or tag is assigned by this change.

## Compatibility window

| Input | Normal command | `--locked` | `--offline` |
| --- | --- | --- | --- |
| Manifest without `format`, or format 1 | Literal backslashes | Same | Same |
| Manifest format 2 | JSON string escapes | Same | Same |
| Lockfile v1 | Resolve and migrate to v3 | Reject unchanged | Migrate only from locally available sources |
| Lockfile v2 | Read legacy strings; successful resolution may migrate to v3 | Preserve bytes if graph matches | Never fetch; migration still allowed without `--locked` |
| Lockfile v3 | Read/write escaped strings | Preserve bytes if graph matches | Never fetch |
| Unknown format or malformed graph | Error; preserve file | Error; preserve file | Error; preserve file |

Older Vex binaries do not understand manifest format 2 or lockfile v3. Upgrade
collaborators before committing a migrated lockfile. Existing manifests are never
automatically rewritten. To opt into escaped strings, add `format = 2` and encode
each existing string's actual value, rather than guessing what backslashes meant.
For example, legacy `"C:\tmp\new"` becomes `"C:\\tmp\\new"`; its value stays the same.
Ambiguous legacy quoting and literal multiline strings require an explicit edit.

The new shared WSON boundary recognizes comments and delimiters only outside
strings. It preserves URL fragments and former sentinel text, rejects duplicate
keys, and reports parser source locations. Unsupported versioned escapes fail
before graph publication. Package version strings retain their existing meaning;
format versions do not introduce SemVer dependency requirements or a registry.

## Dependency transaction

1. Validate CLI options and the root manifest without creating state.
2. Acquire `.vex/state.lock` before reading the lockfile; recover any interrupted
   publication first. Dry-run takes a shared lock and rejects pending recovery.
3. For targeted update, discover the current graph using only available local
   manifests and pinned Git checkouts. If that cannot be established, request an
   explicit `vex fetch`; do not query remotes just to validate a name.
4. Stage candidate checkouts, including isolated copies of reused repositories.
   Resolve and validate the whole graph before moving live directories. Preserve
   declared origins, exact SHAs, dirty data, and unrelated remote-tracking refs.
5. Sync candidates and publish the journal. Move old checkouts into backups and
   rename complete candidates into the existing `.vex/deps/<name>` paths.
6. Atomically replace the lockfile when its graph/format changes. This is the
   commit point. When lockfile bytes must stay unchanged, use the journal's
   committed marker after all checkout transitions instead.
7. Clear the active journal after successful publication or completed recovery.
   Keep backups and abandoned candidates for inspection; do not delete user data.

An interruption before the commit point restores the old checkouts. After the
commit point recovery retains the new graph. The current lockfile must match an
expected transaction state; inconsistent or dirty recovery data stops automatic
recovery with a diagnostic. Recovery is idempotent across interruptions.
Multiple directory renames are not one atomic filesystem operation. Readers see
consistent state because cooperating Vex commands hold the project lock.

The guard lives through dependency resolution and compiler planning/compilation.
Git/compiler children inherit its lifetime. On Unix this uses an inherited flock
descriptor; Windows uses inherited file-sharing restrictions, which persist with
the open handle even after the parent exits. Vex never deletes the coordination
file to recover ownership. These guarantees apply to cooperating Vex processes,
not arbitrary editor/Git modifications or a child deliberately closing its lease.

Implementation references: [Rust File locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock)
and [Windows file-sharing lifetime](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).

## Run generations

Each real run allocates `target/.vex-run/<unique generation>/`. Vex validates a
single JSON plan, compiles without `--run`, checks the produced artifact, drops
the dependency/build guard, and only then spawns the program or compiler-selected
runner with structured arguments. Node/WASM and QEMU runners are not converted to
shell command strings. Runtime cwd, environment and stdio remain inherited; the
existing Vex failure-exit behavior is retained.

Run generations have no automatic GC. This also avoids assuming that all runtime
children have exited when the original program returns. Disk usage increases
until the user performs deliberate cleanup when no process needs those outputs.
The internal generation paths are not the complete public artifact layout from
issue #47, and this change does not introduce multiple package targets or profiles.

Dry-run creates no generation and invokes the compiler planner once. It may create
the coordination directory/file. The printed plan uses a placeholder generation;
it is diagnostic output, not a stable Vex metadata interface.

## Release and merge policy

Rust 1.96.0 is the initial supported toolchain/MSRV, including rustfmt and Clippy.
The checked-in toolchain, Cargo declarations and CI/release setup agree. x.py
selects that toolchain even when the caller's rustup default differs.

Release versions permit normal versions and prereleases, without `+build.metadata`.
The workflow and x.py call the same validation policy. No official release is
created by local build/package verification.

Archives include the license texts and notices from the complete locked Cargo
graph in `THIRD_PARTY_LICENSES`, including build-time and platform-specific
packages. Regenerate and review these when Cargo.lock changes; CI checks their
lockfile fingerprint with CRLF normalized to LF. Known-vulnerability scans are time-specific release
evidence and must be repeated for the eventual release commit.

Master requires PRs, all eight current CI checks and an up-to-date base. Force
pushes and branch deletion are prohibited. There are no bypass actors;
administrators must also meet the PR and check requirements.

Before release: run the new concurrency/recovery tests natively on Linux,
Windows and macOS, verify clean-environment packages, complete the dependency
audit, and validate/pin the next official Wave release. Cross compilation and a
local development wavec smoke do not substitute for those native release gates.
