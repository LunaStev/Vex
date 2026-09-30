# Vex

Vex is the package manager and build tool for the Wave programming language. It manages Wave project manifests, dependency resolution, lockfiles, and stable invocation of `wavec`.

Vex is designed to sit above `wavec` in the same way Cargo sits above `rustc`: Vex owns project structure and dependency orchestration, while `wavec` remains the compiler with detailed build flags.

## Requirements

- `wavec` compatible with the `build --dry-run --error-format=json` schema v1 contract
  and canonical package imports through Vex's `--dep` mappings
- `git` when using Git dependencies
- Rust 1.96.0 when building Vex from source (selected by `rust-toolchain.toml`)
- Python 3.11 or newer when using the release tooling

Vex runs `wavec` from `PATH` by default. Set `VEX_WAVEC=/path/to/wavec` to use a specific compiler binary.
The original Vex v0.0.1 smoke used `wavec 0.2.0-pre-beta`. Current Vex source
also relies on the newer canonical package import contract: the official
wavec v0.2.0-pre-beta archive runs Hello World but cannot compile those package
imports. Schema v1 alone is therefore not a complete compatibility guarantee.
Vex reports schema mismatches before the real build; package-language compatibility
is exercised separately by `tests/wave_compatibility.py` against a selected compiler.

## Platform validation

The targets below have published v0.0.1 release archives. Pull-request CI
validates the current source on these platforms; this is separate from the
package and clean-environment smoke tests required for each release.

| Platform | Rust target | CI validation | v0.0.1 status |
| --- | --- | --- | --- |
| Linux amd64 | `x86_64-unknown-linux-gnu` | native tests, build, package smoke | Released |
| Linux arm64 | `aarch64-unknown-linux-gnu` | native tests and build | Released |
| Windows x64 | `x86_64-pc-windows-msvc` | native tests and build | Released |
| macOS Intel | `x86_64-apple-darwin` | native tests and build | Released |
| macOS Apple Silicon | `aarch64-apple-darwin` | native tests and build | Released |
| Linux RISC-V | `riscv64gc-unknown-linux-gnu` | cross-build and QEMU version smoke | Experimental |

Windows release artifacts use the MSVC target. A Windows GNU artifact is not
part of the v0.0.1 scope. RISC-V remains experimental because its test coverage
is limited to cross-build and QEMU smoke rather than the complete integration
suite.

The v0.0.1 Linux GNU archives are built on Ubuntu 24.04 and require a glibc-based
system; Ubuntu 24.04 is the supported runtime baseline. Windows artifacts are
validated on the GitHub Windows Server 2025 runner, and macOS artifacts on
macOS 15. Older operating systems and other distributions are best effort for
this first release.

## Install

Download the archive and `SHA256SUMS` for your platform from the
[GitHub release](https://github.com/wavefnd/Vex/releases/tag/v0.0.1). Verify the
download before extracting it:

```sh
sha256sum --check SHA256SUMS
tar -xzf vex-v0.0.1-x86_64-unknown-linux-gnu.tar.gz
install -m 0755 vex-v0.0.1-x86_64-unknown-linux-gnu/vex ~/.local/bin/vex
vex --version
```

On Windows, compare `Get-FileHash <archive> -Algorithm SHA256` with the matching
line in `SHA256SUMS`, extract the zip, and place `vex.exe` in a directory on
`PATH`.

To build from source instead:

```sh
git clone https://github.com/wavefnd/Vex.git
cd Vex
cargo build --locked --release
install -m 0755 target/release/vex ~/.local/bin/vex
```

Install `wavec` separately and make it available on `PATH`, or set
`VEX_WAVEC` to its full path. `vex setup wavec --version 0.2.0-pre-beta`
downloads the official host archive, verifies SHA256SUMS and available GitHub
provenance, extracts a new versioned directory, checks the executable version and
atomically switches a current-installation pointer. Previous installations remain
available. Downloads require `curl` (`curl.exe` on Windows), and `gh` when
published provenance must be verified. Missing host artifacts fail explicitly.

`VEX_TOOLCHAIN_HOME` selects the installation prefix (default:
`$HOME/.vex/toolchains`, or `%LOCALAPPDATA%/.vex/toolchains` on Windows).
Compiler selection prefers `VEX_WAVEC`, then `PATH`, then the managed installation.
`--script-fallback` explicitly permits downloading and running the official
`wave-lang.dev` installer if artifact installation fails; it is never automatic.
This fallback follows the external script's installation policy.

Installing an artifact does not prove package-language compatibility. Official
Wave `0.2.0-pre-beta` supports Hello World but not the canonical package imports
required here. Required real-compiler CI remains deferred until a compatible
official release is available.

## Commands

```sh
vex init [--lib]
vex build [--target <triple>] [--release] [--dry-run] [--locked] [--offline]
vex run [--target <triple>] [--release] [--dry-run] [--locked] [--offline] [-- <args...>]
vex check [--target <triple>] [--release] [--dry-run] [--locked] [--offline]
vex fetch [--locked] [--offline]
vex update [<package>...]
vex info
vex tree [--locked] [--offline]
vex metadata [--format=json] [--locked] [--offline]
vex setup wavec [--version <version>] [--script-fallback]
vex --version
```

Project commands search the current directory and its physical ancestors for the
nearest `vex.ws`. An invalid nearest manifest is an error, not a reason to select
another project. `--manifest-path <path/to/vex.ws>` takes precedence and accepts
relative or absolute paths before or after the command. Dependency paths, build
outputs and the program started by `vex run` are relative to that project root.
`init` always uses the invocation directory. Help needs no project.

```sh
vex --manifest-path ../app/vex.ws check --locked --offline
vex metadata --format=json
```

`metadata` writes one versioned JSON document to stdout, without invoking wavec,
fetching, migrating or repairing project state. It uses an existing shared state
lock; run `vex fetch` first when the local graph or coordination state is missing.
`--locked` also enforces lockfile compatibility; metadata preserves v2/v3 bytes
with or without that flag. `--offline` is accepted and is already the default.
See [the metadata v1 contract](docs/metadata.md) for fields and ordering.

## Project Layout

```text
my_project/
├── src/
│   └── main.wave
├── .gitignore
├── vex.ws
└── vex.lock
```

`vex init --lib` creates `src/lib.wave` with a public `greet` example instead
of a binary entry point. Initialization writes a project `.gitignore` for
`/target/` and `/.vex/` only when one does not already exist. The managed
`.vex/deps/` directory is created only when a Git dependency needs a checkout.

## Manifest

Vex uses `vex.ws` as the project manifest. The extension is `.ws`.

```wson
{
    format = 2,
    name = "my_project",
    version = 0.1.0,
    lib = false,
    description = "my_project Project",
    author = "unknown",
    license = "Unknown",
    dependencies = []
}
```

The current root manifest object accepts only `format`, `name`, `version`, `lib`, `description`,
`author`, `license`, and `dependencies`. Dependency objects accept only `name`,
`version`, `path`, `git`, `branch`, `tag`, and `rev`. Unknown fields are errors,
including names intended as private or experimental extensions; adding a field
requires an explicit Vex schema change.

New manifests use `format = 2`: quoted strings decode JSON escapes (`\\`,
`\"`, `\n`, `\r`, `\t`, and Unicode escapes). Strings can contain URLs, commas,
comment markers, quotes, and Unicode without becoming WSON syntax. Comments are
recognized only outside strings; duplicate fields are errors with source locations.
Omitting `format`, or using `format = 1`, preserves legacy literal backslashes.
Vex does not automatically rewrite old manifests or guess whether a backslash was
intended as an escape. Ambiguous legacy quoting/multiline values need an explicit
conversion to format 2. Older Vex releases cannot read the new format.

## Dependencies

Vex currently uses a Git-first package model and does not require a central package registry. Local path dependencies are also supported.

Path dependency:

```wson
{
    name = "my_project",
    version = 0.1.0,
    dependencies = [
        { name = "local_math", path = "../local_math" }
    ]
}
```

Git dependency:

```wson
{
    name = "my_project",
    version = 0.1.0,
    dependencies = [
        { name = "math", git = "https://github.com/example/wave-math.git", tag = "v0.1.0" }
    ]
}
```

A dependency entry must use exactly one of `path` or `git`. Git dependencies may specify at most one of `branch`, `tag`, or `rev`; those selectors are invalid on path dependencies. Source and selector values cannot be empty or whitespace-only. Dependency names must be unique within a manifest and cannot reuse the root package name.

Fetched Git dependencies are stored under `.vex/deps/pkg_<SHA-256 of package name>`. This stable encoding separates case-sensitive import names and avoids Windows device names. Existing `.vex/deps/<name>` checkouts remain usable with `--locked`; a normal fetch stages migration to the encoded path and updates the lockfile while retaining the old checkout. Dry-run does not migrate. Every fetched dependency must contain a `vex.ws` file at its root. Dependency manifests are resolved recursively, and a package name must identify one source and version requirement across the graph.

Vex refuses to use a managed Git checkout with tracked edits or untracked
files, even if its HEAD matches `vex.lock`. If this happens, preserve your
changes outside the managed checkout, restore that checkout yourself, and rerun
`vex fetch`. Vex will not discard local changes automatically.

Every dependency is a library package: its manifest must set `lib = true` and
its canonical entry is `src/lib.wave`. Wave source imports the package name,
not the entry filename:

```wave
import("local_math");
import("local_math::vector");
import("local_math")::{sum, Point};
```

The first form resolves `local_math/src/lib.wave`; the second resolves
`local_math/src/vector.wave`. Vex passes exact mappings for every direct and
transitive dependency to `wavec`, and only `pub` declarations are visible to
consumers.

On the first `vex fetch`, build, run, or check, Vex resolves each Git selector to an exact commit and records the complete transitive graph in `vex.lock`. Later commands reuse those commits without updating branches or tags. Run `vex update` explicitly to refresh every Git dependency and rewrite the lockfile.

Pass one or more package names to update only those packages, including transitive dependencies. Names are validated from the current locally available graph before fetching. If a missing lockfile, checkout, or changed source prevents complete local discovery, run `vex fetch` first; a stale lockfile name list is not authoritative. Unrelated packages keep their exact locked commits and are not fetched. If an updated package changes its dependencies, Vex recalculates that part of the graph while preserving unrelated locked packages.

```sh
# Refresh the complete Git dependency graph.
vex update

# Refresh only alpha and the transitive package shared_core.
vex update alpha shared_core
```

Commit `vex.lock` so the same manifest and lockfile select the same dependency graph. A dry run never fetches or rewrites dependencies; use `vex fetch` first when the locked checkout is not available locally.

Inspect the resolved graph, including path sources, Git selectors, and short
locked commit IDs. `vex tree` performs normal dependency resolution and may clone,
fetch, synchronize checkouts, and write the lockfile. Use `vex tree --locked
--offline` to prevent network access and lockfile changes; local checkout
synchronization can still occur.

```sh
vex tree
vex tree --locked --offline
```

Example output with a Git dependency, a path dependency, and a shared
transitive package:

```text
app v0.1.0
├── alpha v1.0.0 (git https://example.com/alpha.git branch main @ 0123456)
│   └── shared v0.2.0 (path ../shared)
│       └── leaf v0.1.0 (path ../leaf)
└── beta v2.0.0 (path ../beta)
    └── shared v0.2.0 (path ../shared) (*)

(*) package dependencies already shown
```

The `(*)` marker means that package was already expanded earlier in the tree;
its dependencies are not printed again.

### Reproducible and Offline Modes

Use `--locked` when Vex must not create or modify `vex.lock`. The command fails if the lockfile is missing, uses an older schema, or does not match the manifest graph. It may still download a commit already pinned by the lockfile when the managed checkout is missing.

Use `--offline` to prohibit all Git network operations. Vex may switch an existing managed checkout to a locally available locked commit, but it fails with instructions to run `vex fetch` when a checkout or commit is missing.

Combine both options for the strictest CI build:

```sh
vex fetch --locked
vex build --locked --offline
```

`vex update` intentionally accepts neither option because it refreshes Git refs and rewrites the lockfile.

Git commands use null stdin, suppress terminal credential prompts and have a
300-second per-command deadline. `VEX_GIT_TIMEOUT` accepts 1–86400 seconds.
Credential helpers, SSH agents and user Git rewrites remain available. Cancellation
supervises child process trees; user programs inherit stdio without an implicit
timeout. Successful commands/help/version return 0. Vex failures return 1 for
internal errors, 2 for CLI usage, 3 for project/dependency resolution, 4 for
compiler failures, 5 for environment, 124 for timeout, and 130 for cancellation.
`vex run` preserves program exit codes; Unix signal exits translate to `128 + signal`.

Use `vex --message-file build.jsonl build` to record schema-1 JSON Lines without
mixing JSON into compiler or program stdio. Each completed report ends with a
`finished` event containing `origin` (`vex` or `program`), `category`, `exit_code`,
and `success`. A program may return any of Vex's own codes; use `origin` to tell
which failed. Missing completion means an incomplete report. Existing report
files are never overwritten; choose a fresh path in an existing directory.
With `--dry-run`, Vex checks that destination but creates no report file; an
existing destination still errors. A reporting failure before program execution
stops Vex. After execution it warns on stderr and preserves the program exit code.
See [the CLI message contract](docs/cli-contract-design.md) for the full schema.

HTTP userinfo, SSH passwords and recognized authentication query fields are
excluded from rendered Git sources and new lockfiles. SSH routing usernames and
other query fields remain part of identity. A credential-bearing historical
lockfile requires ordinary fetch migration; `--locked` preserves it and errors.
Use Git credential helpers or SSH agents to avoid credentials in your manifest.

## Build Model

Vex uses `wavec` internally and validates the compiler dry-run plan before executing a real build. Vex commands stay manifest-based; raw compiler flags belong to `wavec`, not to Vex.

An explicit target is checked against the selected compiler's
`print supported-targets --format=json` response before dependency resolution.
Capabilities are cached only within the current Vex invocation. An empty or
whitespace-only `VEX_WAVEC` is an error; unset it to use automatic selection.
Relative compiler paths are anchored at the invocation directory.

Arguments after `vex run --` are passed as OS strings directly to the program or
compiler-selected runner, including empty arguments, whitespace and Unicode.
They are not interpreted as Vex options or sent through the compiler JSON plan.
Non-UTF-8 program arguments work on supported OS interfaces; a dry-run JSON plan
cannot represent them and fails explicitly. Compiler input/dependency paths
that the JSON/WSON protocols cannot represent also fail instead of being replaced.

Build progress is written to stderr with Cargo-style stages such as `Resolving`, `Fetching`, `Compiling`, `Checking`, `Running`, and `Finished`. Program output remains on stdout.

Examples:

```sh
vex build --target x86_64-unknown-linux-gnu
vex build --locked --offline
vex run -- arg1 arg2
vex check
VEX_WAVEC=/opt/wave/bin/wavec vex build --dry-run
```

## Development and release tooling

### Lockfile compatibility

Vex writes lockfile schema v3 with explicitly decoded JSON string escapes.
Schema v2 remains readable with literal backslashes. New resolved paths use `/` on every OS; legacy native Windows paths need migration on Windows. Manifest paths retain their declared meaning; use `/` for portable relative paths. `--locked` preserves a valid
v2 or v3 file byte-for-byte when its graph matches. A normal successful fetch can
migrate v2 to v3 without changing source identities, commits, versions, or edges.
Legacy v1 is unresolved: normal fetch may replace it after resolution, offline
only if all sources are local; `--locked` rejects v1.

Unknown future versions and malformed graphs are rejected without rewriting.
Every semantic format change requires versioned fixtures and migration release
notes. Old Vex releases cannot read v3. See [the migration notes](docs/release-readiness.md).

Names must match `[A-Za-z_][A-Za-z0-9_]*`; init rejects invalid directory names
before creating state. Init refuses existing destinations, publishes the manifest
last and recovers interrupted creation from its journal. Edited or replaced files
are preserved. Ordinary/offline fetch creates missing empty lockfiles; locked
mode always requires an existing lockfile.

WSON documents and captured compiler plans are bounded at 8 MiB; WSON nesting is
bounded at 128. Escaped strings and literal legacy backslashes retain their
version-specific behavior.

### Concurrent commands and recovery

State commands coordinate through the persistent `.vex/state.lock`. Build/check
hold exclusive protection from the first lockfile read until compilation ends;
fetch/update/tree and init also participate. `--locked` and `--offline` do not
make a command read-only. Help and info do not create coordination state.

Dry-run uses shared protection and may create only `.vex/` and its coordination
file. It never fetches, repairs dependencies, creates target output, or rewrites
the lockfile. Its single compiler planning call returns validated JSON on stdout;
this diagnostic compiler plan is separate from the versioned `vex metadata` API.

Git candidates are staged and fully validated before publication. Existing
checkouts and backup/recovery records remain under `.vex/`; lockfile replacement
is the commit point when the graph changes. General state commands recover an
interrupted publication before reading its graph. Dry-run reports pending recovery
and requires a normal command such as `vex fetch`. Dirty/conflicting recovery data
is preserved and reported instead of reset. Multiple directory renames are not
one filesystem-wide atomic operation: the project lock hides intermediate states
from cooperating Vex commands, and the journal handles interruption.

Never delete `state.lock` to unlock a project. Lock ownership belongs to OS handles,
including live compiler/Git children, and ends when their handles close. Editing
sources or running Git outside Vex does not participate in this coordination.

`vex run` compiles to a unique `target/.vex-run/<generation>/` directory, releases
project protection **before spawning** the program/runner, and preserves its
working directory, environment, stdio, and runtime arguments. Subsequent builds
and updates cannot overwrite that run's output. Initial policy does not perform
automatic run-generation GC. Transaction backups are also retained; preserve them
when diagnosing failed recovery. No automatic deletion based on PID or age occurs.

Vex is a Cargo workspace. The root package contains only the CLI surface;
manifest parsing, lockfile storage, dependency resolution, compiler invocation,
and toolchain installation live in focused library crates:

```text
Vex/
├── src/          # CLI parsing, commands, and terminal UI
├── manifest/     # vex.ws parsing and rendering
├── lockfile/     # vex.lock parsing, rendering, and storage
├── resolver/     # dependency graph, Git, and path resolution
├── compiler/     # wavec selection, capabilities, plans, and invocation
├── toolchain/    # platform-specific wavec installation
├── state/        # project locks, init recovery, and publication primitives
├── process/      # bounded child execution and cancellation
├── diagnostic/   # shared typed error categories
├── source/       # Git source identity and credential-safe rendering
└── wson/         # shared versioned string/parser boundary
```

The repository-level `x.py` script is the supported entry point for release
builds and packages. It reads the version from `Cargo.toml`, always builds with
the committed `Cargo.lock`, and writes archives plus `SHA256SUMS` to `dist/`.
Run it with Python 3.11 or newer:

```sh
# Show the host and every supported release target.
python3 x.py list-targets

# Run formatting, release-tool tests, Rust tests, Clippy, and a debug build.
python3 x.py check

# Build and package the native target.
python3 x.py build
python3 x.py package

# Build or package one or more explicit targets.
python3 x.py build x86_64-unknown-linux-gnu
python3 x.py package x86_64-unknown-linux-gnu

# Verify an assembled target set and regenerate its checksums.
python3 x.py checksum x86_64-unknown-linux-gnu
```

Archives contain the Vex executable together with `README.md`, `LICENSE`,
`NOTICE`, `COPYRIGHT`, and `THIRD_PARTY_LICENSES`. Their file order, permissions, owners, and timestamps
are normalized. Set
`SOURCE_DATE_EPOCH` to an explicit non-negative Unix timestamp when reproducing
an artifact outside the tagged source revision.

`python3 x.py release [<target>...]` is intentionally stricter than separate
build and package commands. It runs the complete validation suite and succeeds
only when the working tree is clean and `HEAD` has the exact `v<version>` tag.
Cross-target builds still require the corresponding Rust target and native
linker to be installed. `VEX_RELEASE_HOST` exists for release infrastructure
that must override host-target detection; normal development should not set it.

`python3 x.py verify-release` checks that the source tree is clean and `HEAD`
has the annotated `v<version>` tag for local release reproduction. For an
official release, a maintainer dispatches the Release workflow from
`wavefnd/Vex:master`; the workflow refuses forks and non-`master` refs. It
validates and packages the exact upstream commit and verifies the complete
archive set. Its final
`gh release create --target ... --generate-notes` call creates the tag in
`wavefnd/Vex` and generates the GitHub release notes from merged changes.
Publishing a reviewed draft remains a separate maintainer action. See
[RELEASING.md](RELEASING.md) for the complete procedure.

Verify downloaded archives from the directory containing `SHA256SUMS`:

```sh
sha256sum --check SHA256SUMS
```

## License

[MPL 2.0 LICENSE](LICENSE)

## Community and Project Policies

- [Contributing](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Maintainers](MAINTAINERS)
- [Security Policy](SECURITY.md)
- [Release Process](RELEASING.md)
- [Copyright](COPYRIGHT)
- [Notice](NOTICE)
- [Third-party Licenses](THIRD_PARTY_LICENSES)
- [AI Usage Policy](ai.txt)

## Stabilization follow-up

See [the current roadmap](docs/roadmap.md) and [production hardening notes](docs/production-hardening.md)
for implemented contracts, verification limits and remaining acceptance work.
