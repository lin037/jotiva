"""Build the Tauri configuration and the static manifest for one signed release."""
import argparse
import base64
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import sys
from urllib.parse import quote


def config():
    key = os.environ.get("JOTIVA_UPDATER_PUBLIC_KEY", "").strip()
    if not key or not os.environ.get("TAURI_SIGNING_PRIVATE_KEY"):
        raise ValueError("Configure the updater public key and signing private key")
    bundle = {"createUpdaterArtifacts": True}
    if sys.platform == "darwin":
        identity = os.environ["APPLE_SIGNING_IDENTITY"]
        if not identity.startswith("Developer ID Application:") or not os.environ["JOT_INSTALLER_IDENTITY"].startswith("Developer ID Installer:"):
            raise ValueError("Configure Developer ID Application and Installer identities")
        bundle["macOS"] = {"signingIdentity": identity}
    elif sys.platform == "win32":
        bundle["windows"] = {
            "certificateThumbprint": os.environ["JOTIVA_CERT_THUMBPRINT"],
            "digestAlgorithm": "sha256", "timestampUrl": "http://timestamp.digicert.com",
        }
    return {"plugins": {"updater": {"pubkey": key}}, "bundle": bundle}


def manifest(directory, version):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Expected a release version")
    platforms = {}
    for pattern, targets in [
        ("Jotiva.app.tar.gz", ["darwin-aarch64", "darwin-x86_64"]),
        (f"*_{version}_x64-setup.exe", ["windows-x86_64"]),
        (f"*_{version}_amd64.AppImage", ["linux-x86_64"]),
    ]:
        matches = list(directory.glob(pattern))
        if len(matches) != 1 or not matches[0].stat().st_size:
            raise ValueError(f"Expected one nonempty {pattern} in {directory}")
        artifact = matches[0]
        signature = artifact.with_name(artifact.name + ".sig").read_text().strip()
        lines = base64.b64decode(signature, validate=True).decode().splitlines()
        if len(lines) != 4 or not lines[2].startswith("trusted comment:") or f"version:{version}" not in lines[2].split():
            raise ValueError(f"Expected a Tauri signature bound to {version} for {artifact.name}")
        url = f"https://github.com/lin037/jotiva/releases/download/v{version}/{quote(artifact.name)}"
        for target in targets:
            platforms[target] = {"signature": signature, "url": url}
    return {"version": version, "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": platforms}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("config").add_argument("output", type=Path)
    release = commands.add_parser("manifest")
    release.add_argument("directory", type=Path)
    release.add_argument("version")
    args = parser.parse_args()
    if args.command == "config":
        result, output = config(), args.output
    else:
        result, output = manifest(args.directory, args.version), args.directory / "latest.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
