"""Refresh the notices shipped with Jotiva after a dependency upgrade."""

import io
import json
import re
import shutil
import subprocess
import tarfile
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'src-tauri/notices'
LICENSE = re.compile(r'^(?:licen[cs]es?|copying|copyright|notice|unlicense)(?:[-._ ].*)?$', re.I)


def installed_notices(directory):
    texts = []
    for file in sorted(directory.iterdir()):
        if LICENSE.fullmatch(file.name):
            files = sorted(file.rglob('*')) if file.is_dir() else [file]
            texts.extend(f.read_text() for f in files if f.is_file())
    return texts


def installed_js():
    pending = [ROOT]
    found = {}
    while pending:
        directory = pending.pop()
        package = json.loads((directory / 'package.json').read_text())
        key = package['name'], package['version']
        if key in found:
            continue
        found[key] = directory
        for dep in package.get('dependencies', {}):
            for parent in (directory, *directory.parents):
                candidate = parent / 'node_modules' / dep
                if candidate.is_dir():
                    pending.append(candidate.resolve())
                    break
            else:
                raise RuntimeError(f'Install dependency {dep} before generating notices')
    return {key: path for key, path in found.items() if path != ROOT}


def bundled_js(packages):
    # These two runtime entry points ship code embedded at upstream build time.
    bundled = {}
    for (name, _), directory in packages.items():
        if name not in ('mermaid', '@mermaid-js/parser'):
            continue
        entry = directory / json.loads((directory / 'package.json').read_text())['exports']['.']['import']
        maps = sorted((entry.parent / 'chunks' / entry.stem).glob('*.mjs.map'))
        if not maps:
            raise ValueError(f'Missing source maps for {name}')
        for source_map in [Path(str(entry) + '.map'), *maps]:
            data = json.loads(source_map.read_text())
            for source, code in zip(data['sources'], data['sourcesContent'], strict=True):
                if 'node_modules/' not in source:
                    continue
                match = re.search(r'/\.pnpm/[^/]+?@(\d+\.\d+\.\d+(?:-[\w.]+)?)(?:_[^/]+)?/node_modules/((?:@[^/]+/)?[^/]+)/', source)
                if match:
                    version, package = match.groups()
                    bundled[package, version] = None
                elif source == 'webpack://LIB/node_modules/path-browserify/index.js':
                    header = code.split("'use strict';", 1)[0]
                    if 'Permission is hereby granted' not in header:
                        raise ValueError(f'Missing embedded notice for {source}')
                    bundled['path-browserify', 'embedded in @mermaid-js/parser'] = header
                else:
                    raise ValueError(f'Unrecognized embedded package path: {source}')
    return bundled


def registry_notices(name, version):
    filename = name.split('/')[-1]
    url = f'https://registry.npmjs.org/{name}/-/{filename}-{version}.tgz'
    archive = subprocess.check_output(['curl', '--fail', '--silent', '--show-error', url])
    with tarfile.open(fileobj=io.BytesIO(archive), mode='r:gz') as tar:
        metadata = json.load(tar.extractfile('package/package.json'))
        if (metadata['name'], metadata['version']) != (name, version):
            raise ValueError(f'Unexpected package from {url}')
        return [tar.extractfile(f).read().decode() for f in tar.getmembers()
                if f.isfile() and f.name.count('/') == 1 and LICENSE.fullmatch(Path(f.name).name)]


def generate():
    groups = defaultdict(set)

    def include(label, texts):
        if not texts or not all(t.strip() for t in texts):
            raise ValueError(f'No license text found for {label}')
        for text in texts:
            normalized = '\n'.join(line.rstrip() for line in text.splitlines())
            groups[normalized.strip()].add(label)

    js = installed_js()
    for (name, version), directory in sorted(js.items()):
        texts = installed_notices(directory)
        if not texts and name in ('fastdom', 'strictdom'):
            texts = [(directory / 'README.md').read_text().split('## License\n', 1)[1]]
        include(f'npm {name} {version}', texts)
    for (name, version), header in sorted(bundled_js(js).items()):
        if (name, version) not in js:
            include(f'npm {name} {version}', [header] if header else registry_notices(name, version))

    metadata = json.loads(subprocess.check_output([
        'cargo', 'metadata', '--format-version', '1', '--locked', '--filter-platform',
        'aarch64-apple-darwin', '--filter-platform', 'x86_64-apple-darwin',
        '--filter-platform', 'x86_64-pc-windows-msvc', '--filter-platform', 'x86_64-unknown-linux-gnu',
        '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'),
    ], cwd=ROOT))
    crates = {c['name']: Path(c['manifest_path']).parent for c in metadata['packages']}
    for crate in metadata['packages']:
        if not crate['source']:
            continue
        name, version = crate['name'], crate['version']
        texts = installed_notices(Path(crate['manifest_path']).parent)
        if not texts:
            if crate['repository'] == 'https://github.com/madsmtm/objc2':
                texts = [(OUTPUT / 'objc2-MIT.txt').read_text()]
            elif name == 'alloc-stdlib':
                texts = installed_notices(crates['alloc-no-stdlib'])
            elif name == 'selectors':
                texts = installed_notices(crates['cssparser'])
            elif name == 'libappindicator-sys':
                texts = installed_notices(crates['libappindicator'])
            elif name == 'tauri-plugin':
                texts = installed_notices(crates['tauri-runtime'])
            elif name in ('dlopen2', 'dlopen2_derive', 'webview2-com', 'webview2-com-macros', 'webview2-com-sys'):
                repository = crate['repository'].removeprefix('https://github.com/')
                vcs = json.loads((Path(crate['manifest_path']).parent / '.cargo_vcs_info.json').read_text())
                revision = vcs['git']['sha1']
                url = f'https://raw.githubusercontent.com/{repository}/{revision}/LICENSE'
                texts = [subprocess.check_output(['curl', '--fail', '--silent', '--show-error', url], text=True)]
        # Link the unmodified sources, including every crate under the MPL.
        include(f'Rust {name} {version} — https://static.crates.io/crates/{name}/{name}-{version}.crate', texts)

    editor = ROOT / 'node_modules/@jot-md/editor'
    include('Editor fonts and WebAssembly dependencies', [(editor / 'THIRD-PARTY-NOTICES.txt').read_text()])
    include('Jotiva', [(ROOT / 'LICENSE').read_text()])
    editor_std = editor / 'dist/wasm/COPYRIGHT-library.html'
    sysroot = subprocess.check_output(['rustc', '--print', 'sysroot'], cwd=ROOT, text=True).strip()
    native_std = Path(sysroot) / 'share/doc/rust/COPYRIGHT-library.html'
    shutil.copyfile(editor_std, OUTPUT / 'COPYRIGHT-library.html')
    std_note = 'Rust standard library notices for the app and its editor are in COPYRIGHT-library.html.'
    native_output = OUTPUT / 'COPYRIGHT-native-library.html'
    if native_std.read_bytes() != editor_std.read_bytes():
        shutil.copyfile(native_std, native_output)
        std_note = f'Rust standard library notices are in {native_output.name} (native) and COPYRIGHT-library.html (editor).'
    else:
        native_output.unlink(missing_ok=True)
    output = ['Jotiva | Open source licenses', std_note]
    for text, labels in sorted(groups.items(), key=lambda pair: sorted(pair[1])):
        output.extend(['\n'.join(sorted(labels)), text, '-' * 72])
    (OUTPUT / 'THIRD-PARTY-NOTICES.txt').write_text('\n\n'.join(output) + '\n')


if __name__ == '__main__':
    generate()
