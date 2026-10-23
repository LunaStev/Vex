# Command reference

Generated from Vex command help. Regenerate with `python3 tools/cli_reference.py --write --vex target/debug/vex`.

See [manifest and operational contracts](reference.md) for field types, environment, streams and exit codes.

## Global options

```text
Vex - Wave package manager

Usage:
  vex [-q|--quiet] [-v|--verbose] [--message-file <new-path>] [--manifest-path <vex.ws>] <command> [options]
  vex init [--lib]
  vex build [--lib] [--bin <name>] [--target <triple>] [--profile <name>|--release] [--dry-run] [--locked] [--offline]
  vex run [--bin <name>] [--target <triple>] [--profile <name>|--release] [--dry-run] [--locked] [--offline] [-- <args...>]
  vex check [--lib] [--bin <name>] [--target <triple>] [--profile <name>|--release] [--dry-run] [--locked] [--offline]
  vex fetch [--locked] [--offline]
  vex update [<package>...]
  vex info
  vex metadata [--format=json] [--format-version <1|2>] [--locked] [--offline]
  vex tree [--invert <name>] [--depth <n>] [--locked] [--offline]
  vex clean [--dry-run]
  vex prune [--apply] [--dry-run]
  vex doctor [--format=json] [--target <triple>]
  vex setup wavec [--version <version>] [--script-fallback]
  vex --version

Output:
  -q, --quiet    Hide Vex progress; keep results, warnings, errors, and program output
  -v, --verbose  Show decisions; repeat (-vv) for redacted diagnostic detail
```

## init

```text
usage: vex init [--lib]
```

## build

```text
usage: vex build [--lib] [--bin <name>] [--target <triple>] [--profile <name> | --release] [--dry-run] [--locked] [--offline]
```

## check

```text
usage: vex check [--lib] [--bin <name>] [--target <triple>] [--profile <name> | --release] [--dry-run] [--locked] [--offline]
```

## run

```text
usage: vex run [--bin <name>] [--target <triple>] [--profile <name> | --release] [--dry-run] [--locked] [--offline] [-- <args...>]
```

## fetch

```text
usage: vex fetch [--locked] [--offline]
```

## update

```text
usage: vex update [<package>...]
```

## info

```text
usage: vex info
```

## tree

```text
usage: vex tree [--invert <name>] [--depth <n>] [--locked] [--offline]
  --invert <name>  Show the packages that depend on this package
  --depth <n>      Show at most n edges from the displayed root (0: root only)
```

## metadata

```text
usage: vex metadata [--format=json] [--format-version <1|2>] [--locked] [--offline]
```

## doctor

```text
usage: vex doctor [--format=json] [--target <triple>]
```

## clean

```text
usage: vex clean [--dry-run]
```

## prune

```text
usage: vex prune [--apply] [--dry-run]
  Preview unused, unchanged checkouts created by Vex
  --apply    Delete only the verified unused checkouts
  --dry-run  Preview without changing project state (also overrides --apply)
```

## setup

```text
usage: vex setup wavec [--version <version>] [--script-fallback]
```

`vex cache prune [--apply] [--dry-run]` previews or explicitly removes shared Git object caches. See [Git cache](git-cache.md).
