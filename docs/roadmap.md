# Vex stabilization roadmap

Vex `v0.0.2-beta` is published. Its release acceptance covers all nine hosts in
[platforms.json](../platforms.json), public Wave `0.2.1-pre-beta` artifacts,
transaction recovery, installation verification and release provenance.
See [platform validation](../README.md#platform-validation) for the tested
baselines and the limits of emulated runtime evidence. Every subsequent release
must pass those same platform gates for its own source commit.

## Current operational follow-up

This source branch adds explicit, conservative build cleanup (#49) and unused
checkout pruning (#88), offline environment diagnosis (#53), quiet/verbose
output policy (#62), inverse/depth tree queries (#64), and reuse of refs already
obtained by an initial clone (#43). These changes still require the normal PR
review and nine-platform CI before being included in a release.

Cleanup preserves untracked/legacy state and edited files. Live run generations
have independent OS leases; there is no automatic GC. Prune defaults to preview
and requires `--apply` for deletion. Recovery transactions, transaction backups
and prior compiler installations remain outside deletion scope.

## Separate designs

User workspaces (#30), shared caches (#29), multiple package targets and profiles
(#46/#47/#48), manifest editing (#28), build fingerprints (#59), package version
ranges (#132/#51), optional dependencies, registry and publish remain separate
product work. Implemented project discovery, metadata, lockfile v3, source
credential handling, project leases, isolated run generations and atomic init
are maintained rather than reimplemented.

Issue closure follows acceptance evidence, not roadmap checkboxes. Build-only
cross checks do not establish native runtime support, and a compiler schema
match alone does not establish package-language compatibility.
