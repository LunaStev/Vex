#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

"""Read build configuration from the existing Cargo/platform sources of truth."""
import argparse
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools.release_artifacts import (PLATFORMS, SUPPORTED_TARGETS, ReleaseError,
                                     detect_host_target, load_version)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('query', choices=['version', 'host', 'targets', 'platforms', 'list-targets'])
    args = parser.parse_args()
    try:
        if args.query == 'version':
            print(load_version())
        elif args.query == 'host':
            print(detect_host_target())
        elif args.query == 'targets':
            print('\n'.join(SUPPORTED_TARGETS))
        elif args.query == 'list-targets':
            try:
                host = detect_host_target()
            except ReleaseError:
                host = '<unknown>'
            print(f'Host: {host}\nSupported release targets:')
            for target in SUPPORTED_TARGETS.values():
                marker = ' (native default)' if target.triple == host else ''
                print(f'  {target.triple:<36} {target.platform:<8} {target.architecture:<13} {target.archive}{marker}')
        else:
            print('\n'.join(row['id'] for row in PLATFORMS))
    except ReleaseError as error:
        print(f'error: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
