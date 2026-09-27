# Current stabilization sequence

The base for this work is master `0f29f5e` (merged #145). Its eight required CI jobs
passed, including Windows. The old #99 text describing a red post-#113 master is
historical and is not the current baseline. Issue checkboxes are acceptance
tracking, not proof that an implementation is absent or complete.

1. Review the production-hardening branch as one coordinated PR: initialization,
   checkout encoding/reuse, lock portability, process supervision, credential
   handling, artifact installation, dependency auditing and release provenance.
   See [the implementation/acceptance notes](production-hardening.md).
2. Run native CI and review outstanding acceptance for #75/#79 and the earlier
   #145 work. Preserve the PR requirement, eight checks, latest-base requirement,
   and prohibition on force/delete/bypass.
3. When a compatible official Wave release exists, pin/validate that artifact and
   activate required real-compiler CI (#66/#131). Do not substitute a pinned source
   build or infer compatibility from version/schema alone.
4. Continue feature work in order: project discovery (#25), compiler capabilities
   (#27), then JSON metadata (#26). The separate #90 diagnostic JSONL and exit-code
   contract is implemented in this branch; native PR CI remains acceptance work.
5. Package targets/artifacts/profiles (#46/#47/#48), user workspaces (#30), package
   version policy (#132), and other product-design work remain separate decisions.

Explicit cleanup/GC, submodule policy, runtime OS baseline and repository-wide
normalization are separate follow-ups. Git/path sources remain the active model;
central registry/publish remain out of scope. This document does not publish any
issue edits or close issues automatically.
