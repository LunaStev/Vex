#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

"""Execute one Meson step; record success only after the command succeeds."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def environment(arguments):
    env = dict(os.environ)
    with (ROOT / 'rust-toolchain.toml').open('rb') as source:
        env['RUSTUP_TOOLCHAIN'] = tomllib.load(source)['toolchain']['channel']
    targets = [arg.split('=', 1)[1] for arg in arguments if arg.startswith('--target=')]
    targets += [arguments[i+1] for i, arg in enumerate(arguments[:-1]) if arg == '--target']
    if 'riscv64gc-unknown-linux-gnu' in targets:
        key = 'CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_LINKER'
        if key not in env:
            linker = shutil.which('riscv64-linux-gnu-gcc')
            if linker is None:
                raise ValueError(f'RISC-V builds require riscv64-linux-gnu-gcc or {key}')
            env[key] = linker
    return env


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stamp', type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command:
        parser.error('a command is required')
    try:
        if args.stamp:
            args.stamp.unlink(missing_ok=True)
        result = subprocess.run(command, cwd=ROOT, env=environment(command), check=False)
        if result.returncode == 0 and args.stamp:
            args.stamp.touch()
        return result.returncode
    except (OSError, ValueError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
