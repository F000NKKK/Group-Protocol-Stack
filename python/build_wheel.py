"""Build a platform-specific wheel for ``gbp-stack``.

Reads the target platform tag from the ``GBP_STACK_TARGET_PLATFORM`` env
var (set by the CI workflow) and retags the freshly built wheel with that
tag so PyPI serves the right wheel to each OS/arch.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def main() -> int:
    plat = os.environ.get("GBP_STACK_TARGET_PLATFORM")
    here = Path(__file__).parent.resolve()
    dist = here / "dist"
    if dist.exists():
        shutil.rmtree(dist)

    cmd = [sys.executable, "-m", "build", "--wheel", "--outdir", str(dist)]
    print(">", " ".join(cmd))
    rc = subprocess.call(cmd, cwd=here)
    if rc != 0:
        return rc

    if not plat:
        return 0

    for whl in dist.glob("*.whl"):
        retag = [
            sys.executable, "-m", "wheel", "tags",
            "--platform-tag", plat,
            "--remove",
            str(whl),
        ]
        print(">", " ".join(retag))
        rc = subprocess.call(retag, cwd=here)
        if rc != 0:
            return rc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
