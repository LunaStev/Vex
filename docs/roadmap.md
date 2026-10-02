# Current stabilization sequence

Project discovery (#25), compiler capabilities (#27), and read-only JSON metadata
(#26) were merged in #153 at master `d6a0bd0`. State coordination and production
hardening were merged earlier in #145 and #149. Open issue status alone does not
mean that the corresponding implementation is absent.

1. Finish Wave release consumption (#154/#96): select all eight Wave host
   artifacts, preserve the existing checksum/provenance and transactional
   installation boundaries, and test ZIP/TAR payload preservation. Review Wave's
   additive metadata schema without assuming a new requirement for older releases.
2. Make missing package execution verification explicit (#71), and handle closed
   CLI output pipes without panic (#91). These changes require the existing eight
   PR checks, including native Windows and macOS runs.
3. Review lockfile compatibility (#79/#138) and the implemented #110/#124/#146/
   #147/#148 acceptance against regression tests. Do not reimplement completed
   work or close broad issues solely because a related PR merged.
4. Keep release support documentation and this roadmap current (#75/#99).
5. When the official compatible Wave v0.2.1-pre-beta artifact exists, verify its
   checksum/provenance, run package-import and public-reexport smoke tests, then
   pin the verified artifact in required compiler CI (#66/#131). No source-build
   gate substitutes for the agreed official-release acceptance.

The package/CLI changes above form one reviewable PR. The compiler CI activation
remains conditional on the official Wave artifact; a local development compiler
or mock release is not a substitute. See [release readiness](release-readiness.md)
for the compatibility window and remaining release evidence.

Vex host distribution expansion (#72/#73), minimum runtime baselines (#85),
explicit cleanup/GC (#88), submodules (#95), and package targets/artifacts/profiles
(#46/#47/#48) remain separate. User workspaces (#30), package version policy
(#132), registry and publish are outside this stabilization bundle.
