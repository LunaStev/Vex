# Environment diagnostics

`vex doctor [--format=json] [--target <triple>]` inspects the local environment.
It works outside a project. `--manifest-path <vex.ws>` selects an existing project
using the same physical-path rules as build; otherwise doctor searches the current
directory and its ancestors. An explicitly missing or invalid manifest is an error.

Doctor reports:

- Vex version, host OS/architecture, executable path and invocation directory.
- The selected compiler path/version, supported targets and effective target
  (`--target`, then project `build.target`, then the compiler default).
  `VEX_WAVEC` is authoritative, including when its value is invalid. Relative
  compiler paths and PATH entries are interpreted from the invocation directory.
- Git's executable path and version.
- An available project's manifest, declared target entries and exact compiler requirement.
- Whether the existing dependency graph permits checking every root target
  with the root's effective `dev` optimization level.
- Each planned linker's path and execute permission when its plan requires linking.

Doctor does not install tools, fetch dependencies, create temporary projects,
create coordination files, update lockfiles or checkouts, recover transactions,
compile, link, or run a user program. Tool queries are local version/capability
requests and, when possible, `wavec build --dry-run`. They use the existing bounded,
noninteractive subprocess supervision. No target linker is executed.

Project planning uses the existing graph under a read-only shared project lease.
The lease remains held while the compiler reads dependency sources for every
declared target. Entry paths for all targets are checked before planning begins;
a missing secondary binary cannot be hidden by a valid library. Binary plans
include the package's own library import when present. Planner calls use the same
output layout and platform settings as a dev build, without executing compilation
or modifying artifact ownership. Missing
coordination or dependencies, an inconsistent local graph, or pending recovery
produces a warning and an explicit `not_checked` dry-run result. Doctor preserves
that state; run `vex fetch` explicitly to prepare or recover the project. Outside a
project, the project and dry-run checks are `not_checked`; tool checks still run.
An explicitly requested `--message-file` is a separate user-requested JSONL output
with the normal no-overwrite contract.

Missing Git/compiler, incompatible capabilities or dry-run schema, an invalid
selected manifest, a missing canonical input, an unsatisfied compiler requirement,
and unavailable host linker tools are required failures. An unsupported explicit
`--target` is an error. For a supported explicit cross target, missing linker tools
or a compiler-reported backend environment failure are warnings. Doctor does not
prove that a target sysroot, libraries or linker can produce a working executable;
a valid plan and an existing executable are only local preflight checks.

Doctor prints the complete report before returning a failure. Required failures
use the existing exit categories: project/target resolution `3`, compiler protocol
`4`, environment `5`, timeout `124`, cancellation `130`. If independent checks fail,
the first required failure in report order determines the category; supervised
timeout/cancellation retains its normal precedence. Warnings and unchecked
optional checks alone return `0`, so callers must inspect individual results when
they require a completed project or cross-target check. Invalid CLI arguments
return usage code `2` before invoking tools.

## JSON schema v1

`--format=json` (or `--format json`) writes exactly one UTF-8 JSON document on
stdout. Human diagnostics use the default format; command errors use stderr.
Quiet mode does not hide the report. Source authentication and known credentials
are redacted in human and JSON reports. No full environment or raw PATH is dumped.
Non-UTF-8 paths produce an explicit error instead of lossy JSON path strings.

The machine-readable contract is [doctor-v1.json](schemas/doctor-v1.json).

| Field | Meaning |
| --- | --- |
| `schema_version` | Integer `1`; consumers must check it |
| `success` | No required check failed; not a claim that unchecked checks passed |
| `checks` | Ordered diagnostic records; consumers should select by `id` and target details |
| `checks[].id` | Check name, such as `wavec`, `capabilities`, `local_graph` or `dry_run` |
| `checks[].status` | `ok`, `warning`, `error` or `not_checked` |
| `checks[].required` | Whether this result is part of required environment validation |
| `checks[].message` | Human-readable explanation, not a stable classifier |
| `checks[].details` | Check-specific object or null, including paths, versions and category where available |

Check records are conditionally included: a failed compiler capability check cannot
produce a meaningful target result, for example. Check IDs and details may be added
within schema v1. Consumers should ignore unknown fields and check IDs; changes to
existing field types or meanings require a new schema version. Object-key order is
not a contract. Paths are absolute native paths where they identify tools or the
selected manifest. Target strings are the compiler's capability names.


For mixed/multiple-target projects, `dry_run` and `linker` IDs repeat once per
planned target. Successful `dry_run` details add `package_target: {name, kind}`,
`profile: "dev"`, and effective `opt_level`. Linker details identify the package
target too. These are additive schema-1 fields. The report does not select a run
default or execute its program. CLI/compiler target selection remains separate
from a package's binary/library target name.
