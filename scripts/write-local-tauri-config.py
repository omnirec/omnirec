#!/usr/bin/env python3

import json
import os
import platform
import shlex
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: write-local-tauri-config.py <output-path>")

    output_path = Path(sys.argv[1])
    base = json.loads(Path("src-tauri/tauri.conf.json").read_text())

    macos_overlay_path = Path("src-tauri/tauri.macos.conf.json")
    macos_overlay = (
        json.loads(macos_overlay_path.read_text())
        if macos_overlay_path.exists()
        else {}
    )

    app_windows = base.setdefault("app", {}).setdefault("windows", [{}])
    bundle = base.setdefault("bundle", {})
    resources = bundle.setdefault("resources", {})
    build = base.setdefault("build", {})
    macos_bundle = bundle.setdefault("macOS", {})

    bundle["createUpdaterArtifacts"] = False
    app_windows[0].update(macos_overlay.get("app", {}).get("windows", [{}])[0])
    resources.update(macos_overlay.get("bundle", {}).get("resources", {}))

    signing_identity = os.environ.get("APPLE_SIGNING_IDENTITY") or macos_bundle.get(
        "signingIdentity"
    )

    if platform.system() == "Darwin" and signing_identity:
        macos_bundle["signingIdentity"] = signing_identity
        build["beforeBundleCommand"] = {
            "script": f"bash scripts/sign-macos-whisper.sh {shlex.quote(signing_identity)}"
        }
    else:
        macos_bundle.pop("signingIdentity", None)
        build.pop("beforeBundleCommand", None)

    output_path.write_text(json.dumps(base))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
