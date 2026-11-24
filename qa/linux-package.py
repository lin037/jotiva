"""Launch two documents through the desktop entry shipped in a Debian package."""
from configparser import ConfigParser
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

cache = Path.home() / ".cache" / "jotiva-qa"
cache.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(dir=cache) as directory:
    work = Path(directory)
    package = work / "package"
    subprocess.run(["dpkg-deb", "-x", sys.argv[1], package], check=True)
    executable = package / "usr/bin/Jotiva"
    entry = package / "usr/share/applications/Jotiva.desktop"
    config = ConfigParser(interpolation=None)
    config.read(entry)
    desktop = config["Desktop Entry"]
    assert {"text/plain", "text/markdown"} <= set(desktop["MimeType"].split(";"))
    assert "%F" in desktop["Exec"].split()

    home = work / "home"
    home.mkdir()
    env = dict(os.environ, HOME=str(home), PATH=f"{executable.parent}:{os.environ['PATH']}",
               XDG_DATA_HOME=str(home / "data"), XDG_CONFIG_HOME=str(home / "config"),
               XDG_CACHE_HOME=str(home / "cache"))
    files = {str(work / "测试 文本.txt"): "UTF-8 文本\r\n原样", str(work / "第二份 文档.md"): "# Markdown"}
    for filename, body in files.items():
        Path(filename).write_bytes(body.encode())
    try:
        subprocess.run(["gio", "launch", entry, *files], env=env, check=True, timeout=15)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            records = [json.loads(p.read_bytes()) for p in (home / "data").glob("*/notes/[0-9]*.json")]
            if {r["file"]: r["markdown"] for r in records if r.get("file")} == files:
                break
            time.sleep(0.15)
        else:
            raise RuntimeError("The packaged app did not receive both files from the desktop launcher")
    finally:
        for process in Path("/proc").glob("[0-9]*"):
            try:
                if (process / "exe").readlink() == executable:
                    os.kill(int(process.name), signal.SIGTERM)
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                continue
    print("Packaged desktop entry opened both UTF-8 files, including paths with spaces, in Jotiva.")
