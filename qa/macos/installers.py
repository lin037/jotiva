"""Verify the macOS installers against their source app without installing it."""

import argparse
import filecmp
import json
import os
import plistlib
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

from ds_store import DSStore
from mac_alias import Alias


def compare_app(source, copy):
    subprocess.run(["codesign", "--verify", "--deep", "--strict", copy], check=True)
    files = {p.relative_to(source) for p in source.rglob("*")}
    assert files == {p.relative_to(copy) for p in copy.rglob("*")}, "App contents changed"
    for path in files:
        left, right = source / path, copy / path
        if left.is_symlink():
            assert right.is_symlink() and left.readlink() == right.readlink(), path
        elif left.is_file():
            assert filecmp.cmp(left, right, shallow=False), path


def verify_dmg(app, dmg, mount, layout):
    subprocess.run(["hdiutil", "verify", dmg], check=True)
    mount.mkdir()
    subprocess.run([
        "hdiutil", "attach", "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", mount, dmg,
    ], check=True)
    try:
        assert os.readlink(mount / "Applications") == "/Applications"
        assert (mount / "Applications").samefile("/Applications"), "Applications drop target cannot resolve"
        assert {p.name for p in mount.iterdir() if not p.name.startswith(".")} == {app.name, "Applications"}
        with DSStore.open(str(mount / ".DS_Store"), "r") as store:
            assert store["."]["icvl"] == (b"type", b"icnv")
            for name, field in [(app.name, "appPosition"), ("Applications", "applicationFolderPosition")]:
                point = layout[field]
                assert store[name]["Iloc"] == (point["x"], point["y"])
            view = store["."]["icvp"]
            window = store["."]["bwsp"]
            size = layout["windowSize"]
            assert f"{{{size['width']}, {size['height']}}}" in window["WindowBounds"]
            assert not window["ShowSidebar"] and not window["ShowToolbar"]
            assert view["backgroundType"] == 2 and view["arrangeBy"] == "none"
            alias = Alias.from_bytes(view["backgroundImageAlias"])
            info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
            assert alias.target.filename == ".background.tiff"
            assert alias.volume.name == f"{info['CFBundleName']} {info['CFBundleShortVersionString']}"
            image = mount / alias.target.filename
            details = subprocess.check_output(["tiffutil", "-info", image], text=True, stderr=subprocess.STDOUT)
            for scale in (1, 2):
                assert f"Image Width: {size['width'] * scale} Image Length: {size['height'] * scale}" in details
        compare_app(app, mount / app.name)
    finally:
        subprocess.run(["hdiutil", "detach", mount], check=True)


def verify_pkg(app, pkg, expanded, info):
    domains = subprocess.check_output(["installer", "-dominfo", "-pkg", pkg], text=True).splitlines()
    assert {"CurrentUserHomeDirectory", "LocalSystem"} <= set(domains), domains
    choices = plistlib.loads(subprocess.check_output([
        "installer", "-pkg", pkg, "-target", "CurrentUserHomeDirectory", "-showChoicesXML",
    ]))
    assert choices[0]["choiceIsSelected"], "The current user's home is not an available target"
    subprocess.run(["pkgutil", "--expand-full", pkg, expanded], check=True)
    distribution = ET.parse(expanded / "Distribution")
    assert distribution.findtext("title") == info["CFBundleName"]
    assert distribution.find("options").get("require-scripts") == "false"
    assert distribution.find(".//os-version").get("min") == info["LSMinimumSystemVersion"]
    binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
    archs = subprocess.check_output(["lipo", "-archs", binary], text=True).split()
    assert set(distribution.find("options").get("hostArchitectures").split(",")) == set(archs)
    [component] = expanded.glob("*.pkg")
    metadata = ET.parse(component / "PackageInfo")
    assert metadata.getroot().get("install-location") == "/Applications"
    assert metadata.getroot().get("version") == info["CFBundleShortVersionString"]
    assert metadata.getroot().get("identifier") == info["CFBundleIdentifier"]
    assert metadata.find("relocate/bundle") is None and metadata.find("scripts") is None
    assert not (component / "Scripts").exists()
    payload = component / "Payload"
    assert [p.name for p in payload.iterdir()] == [app.name]
    compare_app(app, payload / app.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("app", "dmg", "pkg"):
        parser.add_argument(key, type=Path)
    parser.add_argument("--zip", type=Path)
    args = parser.parse_args()
    info = plistlib.loads((args.app / "Contents/Info.plist").read_bytes())
    config = Path(__file__).resolve().parents[2] / "src-tauri/tauri.conf.json"
    layout = json.loads(config.read_text())["bundle"]["macOS"]["dmg"]
    with tempfile.TemporaryDirectory(prefix="jot-installer-qa-") as temporary:
        verify_pkg(args.app, args.pkg, Path(temporary) / "expanded", info)
        verify_dmg(args.app, args.dmg, Path(temporary) / "mount", layout)
        if args.zip:
            extracted = Path(temporary) / "unzipped"
            subprocess.run(["ditto", "-x", "-k", args.zip, extracted], check=True)
            assert [p.name for p in extracted.iterdir()] == [args.app.name]
            compare_app(args.app, extracted / args.app.name)
    print("Installer domains, package contents, Finder layout and app signatures verified.")


if __name__ == "__main__":
    main()
