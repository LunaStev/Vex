#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0
"""Real-compiler smoke, deliberately separate from the network-free Rust suite."""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def run(command, cwd, env, *, succeeds=True):
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    print(f"$ {' '.join(map(str, command))}", flush=True)
    print(result.stdout, end="", flush=True)
    print(result.stderr, end="", flush=True)
    if (result.returncode == 0) != succeeds:
        raise RuntimeError(f"unexpected exit {result.returncode}: {command}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vex", type=Path, required=True)
    parser.add_argument("--wavec-bin", type=Path, required=True)
    parser.add_argument("--reexports", action="store_true")
    args = parser.parse_args()
    vex = str(args.vex.resolve())
    env = os.environ.copy()
    env.pop("VEX_WAVEC", None)
    env["PATH"] = str(args.wavec_bin.resolve()) + os.pathsep + env.get("PATH", "")
    with tempfile.TemporaryDirectory(prefix="vex-wave-compat-") as temporary:
        root = Path(temporary)
        run(["wavec", "--version"], root, env)
        app, middle, leaf = [root / name for name in ("app", "middle", "leaf")]
        for package in (app, middle, leaf):
            package.mkdir()
            run([vex, "init", *(["--lib"] if package != app else [])], package, env)
        for command in ("build", "check", "run"):
            result = run([vex, command, "--locked", "--offline"], app, env)
            if command == "run" and "Hello World" not in result.stdout:
                raise RuntimeError("Hello World was not printed through PATH wavec")
        rejected = run([vex, "build", "--emit=obj"], app, env, succeeds=False)
        if "unknown Vex option" not in rejected.stderr:
            raise RuntimeError("raw compiler option was not rejected by Vex")

        (leaf / "src/lib.wave").write_text(
            "pub fun value() -> i32 { return 42; }\nfun hidden() -> i32 { return 9; }\n",
            encoding="utf-8",
        )
        run(["git", "init", "-q", "-b", "master"], leaf, env)
        run(["git", "add", "."], leaf, env)
        run(["git", "-c", "user.name=Vex Test", "-c", "user.email=vex@example.invalid", "commit", "-qm", "fixture"], leaf, env)
        (middle / "vex.ws").write_text(
            '{ name = "middle", version = 0.1.0, lib = true, dependencies = ['
            f'{{ name = "leaf", git = "{leaf.as_uri()}" }}] }}\n', encoding="utf-8",
        )
        (middle / "src/lib.wave").write_text(
            'pub import("leaf")::{value};\n' if args.reexports else
            'import("leaf");\npub fun forwarded() -> i32 { return value(); }\n',
            encoding="utf-8",
        )
        symbol = "value" if args.reexports else "forwarded"
        (app / "vex.ws").write_text(
            '{ name = "app", version = 0.1.0, dependencies = [{ name = "middle", path = "../middle" }] }\n',
            encoding="utf-8",
        )
        import_line = f'import("middle")::{{{symbol}}};' if args.reexports else 'import("middle");'
        (app / "src/main.wave").write_text(
            f'{import_line}\nfun main() {{ var result: i32 = {symbol}(); println("{{}}", result); }}\n',
            encoding="utf-8",
        )
        run([vex, "fetch"], app, env)
        locked = (app / "vex.lock").read_bytes()
        # Take the original source offline: successful reuse cannot clone/fetch it.
        leaf.rename(root / "offline_leaf")
        for command in ("fetch", "build", "check", "run"):
            result = run([vex, command, "--locked", "--offline"], app, env)
            if command == "run" and "42" not in result.stdout.splitlines():
                raise RuntimeError("dependency graph did not produce 42")
            if (app / "vex.lock").read_bytes() != locked:
                raise RuntimeError("locked/offline command changed vex.lock")
        if args.reexports:
            (middle / "src/lib.wave").write_text('pub import("leaf")::{hidden};\n', encoding="utf-8")
            (app / "src/main.wave").write_text('import("middle")::{hidden};\nfun main() { var result: i32 = hidden(); println("{}", result); }\n', encoding="utf-8")
            rejected = run([vex, "check", "--locked", "--offline"], app, env, succeeds=False)
            if "hidden" not in rejected.stderr or "private" not in rejected.stderr:
                raise RuntimeError("private import failed for an unexpected reason")
            if (app / "vex.lock").read_bytes() != locked:
                raise RuntimeError("private-symbol failure changed vex.lock")


if __name__ == "__main__":
    main()
