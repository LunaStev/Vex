#!/bin/sh
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0
set -eu
export CARGO_BUILD_JOBS=2
export RUST_TEST_THREADS=2
case "$(uname -s)" in
  Linux)
    echo 'QEMU Linux acceptance uses tools/ci_cross.py on amd64' >&2
    exit 2
    ;;
  FreeBSD)
    vex_guest_python=python3.11
    pkg install -y ca_root_nss curl git python311 ninja gmake pkgconf gh
    ;;
  *) echo 'unsupported guest OS' >&2; exit 1;;
esac
# python311 ships ensurepip and its bundled wheels; venv bootstraps pip
# without requiring a separately packaged Python-flavor pip.
"$vex_guest_python" -m venv /tmp/vex-meson
/tmp/vex-meson/bin/python -m pip install -r tools/requirements-build.txt
export PATH="/tmp/vex-meson/bin:$PATH"
curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/vex-rustup.sh
sh /tmp/vex-rustup.sh -y --profile minimal --default-toolchain 1.96.0
export PATH="$HOME/.cargo/bin:/tmp/vex-meson/bin:/usr/local/bin:$PATH"
# The whole process tree (Git, fake compiler, real compiler, user programs)
# executes in this target environment through binfmt or the FreeBSD kernel.
git config --global --add safe.directory "$(pwd)"
meson setup build -Drelease_targets=x86_64-unknown-freebsd -Dacceptance_platform="$1"
meson compile -C build acceptance
