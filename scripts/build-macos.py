"""Build the macOS release with portable source paths in compiler diagnostics."""

import importlib.util
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[1]
    os.chdir(root)
    home = Path.home()
    cargo = Path(os.environ.get("CARGO_HOME", home / ".cargo")).resolve()
    toolchain = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
    paths = {
        home: "/build/home",
        cargo: "/build/cargo",
        Path(toolchain): "/build/rust",
        root: "/build/jot",
    }
    # These directory names may identify the developer's registry mirror.
    for registry in (cargo / "registry/src").glob("*"):
        if registry.is_dir():
            paths[registry] = "/build/crates"
    # rustc applies the last matching prefix. A workspace may be under $HOME.
    mappings = [f"{path}={paths[path]}" for path in sorted(paths, key=lambda p: len(str(p)))]
    env = os.environ.copy()
    if "CARGO_ENCODED_RUSTFLAGS" in env:
        rust = env["CARGO_ENCODED_RUSTFLAGS"].split("\x1f") if env["CARGO_ENCODED_RUSTFLAGS"] else []
    else:
        rust = env.get("RUSTFLAGS", "").split()
    rust.extend(f"--remap-path-prefix={mapping}" for mapping in mappings)
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(rust)

    for key in ("CFLAGS", "CXXFLAGS"):
        previous = env.get(key, "")
        flags = shlex.split(previous) if env.get("CC_SHELL_ESCAPED_FLAGS") == "1" else previous.split()
        flags.extend(f"-ffile-prefix-map={mapping}" for mapping in mappings)
        env[key] = shlex.join(flags)
    env["CC_SHELL_ESCAPED_FLAGS"] = "1"
    args = sys.argv[1:] or ["--bundles", "app", "--", "--locked"]
    tauri_args = args[:args.index("--")] if "--" in args else args
    local_config = root / "src-tauri/tauri.release.local.conf.json"
    if local_config.is_file() and not any(
        arg in {"-c", "--config"} or arg.startswith("--config=") for arg in tauri_args
    ):
        # Keep the configured identity on subsequent builds, including an offline build.
        args = ["--config", str(local_config), *args]
    command = ["pnpm", "tauri", "build", *args]
    if sys.argv[1:]:
        os.execvpe("pnpm", command, env)

    if importlib.util.find_spec("dmgbuild") is None:
        sys.exit("Install scripts/requirements-macos.txt in your Python virtual environment first.")
    subprocess.run(command, env=env, check=True)
    metadata = subprocess.check_output([
        "cargo", "metadata", "--format-version", "1", "--no-deps", "--manifest-path", "src-tauri/Cargo.toml",
    ], text=True, env=env)
    target = Path(json.loads(metadata)["target_directory"])
    if env.get("CARGO_BUILD_TARGET"):
        target /= env["CARGO_BUILD_TARGET"]
    config = json.loads((root / "src-tauri/tauri.conf.json").read_text())
    app = target / "release/bundle/macos" / f"{config['productName']}.app"
    os.execv(sys.executable, [sys.executable, str(root / "scripts/package-macos.py"), str(app)])


if __name__ == "__main__":
    main()
