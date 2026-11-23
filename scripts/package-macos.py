"""Create a macOS disk image, Installer package, and ZIP archive."""

import argparse
import json
import os
import plistlib
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

from dmgbuild import build_dmg


ROOT = Path(__file__).resolve().parents[1]


def build_pkg(app, output, work, info, architectures):
    payload = work / "payload"
    payload.mkdir()
    subprocess.run(["ditto", app, payload / app.name], check=True)
    components = work / "components.plist"
    components.write_bytes(plistlib.dumps([{
        "RootRelativeBundlePath": app.name,
        "BundleIsRelocatable": False,
        "BundleIsVersionChecked": True,
        "BundleHasStrictIdentifier": True,
        "BundleOverwriteAction": "upgrade",
    }]))
    component = work / f"{app.stem}-component.pkg"
    subprocess.run([
        "pkgbuild", "--root", payload, "--component-plist", components,
        "--identifier", info["CFBundleIdentifier"], "--version", info["CFBundleShortVersionString"],
        "--install-location", "/Applications", component,
    ], check=True)
    requirements = work / "requirements.plist"
    requirements.write_bytes(plistlib.dumps({
        "os": [info["LSMinimumSystemVersion"]], "arch": architectures, "home": True,
    }))
    distribution = work / "Distribution"
    subprocess.run([
        "productbuild", "--synthesize", "--product", requirements,
        "--package", component, distribution,
    ], check=True)
    product = ET.parse(distribution)
    ET.SubElement(product.getroot(), "title").text = info["CFBundleName"]
    ET.SubElement(product.getroot(), "product", {
        "id": info["CFBundleIdentifier"], "version": info["CFBundleShortVersionString"],
    })
    product.write(distribution, encoding="utf-8", xml_declaration=True)
    command = ["productbuild", "--distribution", distribution, "--package-path", work]
    identity = os.environ.get("JOT_INSTALLER_IDENTITY")
    if identity:
        command.extend(["--sign", identity])
    subprocess.run([*command, output], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path, help="Path to the app bundle produced by Tauri")
    parser.add_argument("--output-dir", type=Path, help="Defaults to release/<version>")
    parser.add_argument("--format", choices=("dmg", "pkg", "zip"), help="Build only this format (defaults to all three)")
    args = parser.parse_args()
    app = args.app.resolve()
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    version = info["CFBundleShortVersionString"]
    name = info["CFBundleName"]
    subprocess.run(["codesign", "--verify", "--strict", "--deep", app], check=True)
    binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
    archs = subprocess.check_output(["lipo", "-archs", binary], text=True).split()
    suffix = "universal" if len(archs) > 1 else {"arm64": "aarch64"}.get(archs[0], archs[0])
    output = (args.output_dir or ROOT / "release" / version).resolve()
    output.mkdir(parents=True, exist_ok=True)
    config = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())
    layout = config["bundle"]["macOS"]["dmg"]
    size, left, right = (layout[key] for key in ("windowSize", "appPosition", "applicationFolderPosition"))
    with tempfile.TemporaryDirectory(dir=output, prefix=".jot-package-") as temporary:
        work = Path(temporary)
        stem = f"{name}_{version}_{suffix}"
        pkg = work / f"{stem}.pkg"
        dmg = work / f"{stem}.dmg"
        archives = []
        if args.format in (None, "pkg"):
            build_pkg(app, pkg, work, info, archs)
            archives.append(pkg)
        if args.format in (None, "dmg"):
            build_dmg(str(dmg), f"{name} {version}", settings={
                "format": "UDZO",
                "filesystem": "HFS+",
                "files": [str(app)],
                "symlinks": {"Applications": "/Applications"},
                "icon": str(app / "Contents/Resources" / info["CFBundleIconFile"]),
                "background": str(ROOT / "src-tauri" / layout["background"]),
                "window_rect": ((150, 170), (size["width"], size["height"])),
                "icon_locations": {app.name: (left["x"], left["y"]), "Applications": (right["x"], right["y"])},
                "icon_size": 88,
                "text_size": 13,
            })
            archives.append(dmg)
        if args.format in (None, "zip"):
            zip_file = work / f"{stem}.zip"
            subprocess.run(["ditto", "-c", "-k", "--keepParent", app, zip_file], check=True)
            archives.append(zip_file)
        for archive in archives:
            archive.replace(output / archive.name)
            print(output / archive.name)


if __name__ == "__main__":
    main()
