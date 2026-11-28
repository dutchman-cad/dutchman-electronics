"""We build and check the same portable archives on every release runner."""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]


def version():
    with (ROOT / "Cargo.toml").open("rb") as source:
        return tomllib.load(source)["workspace"]["package"]["version"]


def check_tag(tag, expected):
    match = re.fullmatch(r"v(\d+\.\d+\.\d+)(?:-rc\.\d+)?", tag)
    if match is None or match[1] != expected:
        raise ValueError(f"Tag {tag} must name Cargo.toml version {expected} (vX.Y.Z or vX.Y.Z-rc.N)")


def smoke(binary):
    binary = str(Path(binary).resolve())
    output = subprocess.check_output([binary, "--version"], text=True)
    if output.splitlines() != [f"dutchman-electronics {version()}"]:
        raise ValueError(f"Unexpected --version output: {output!r}")
    info = json.loads(subprocess.check_output([binary, "info", "--json"], text=True))
    if info.get("interface") != 1 or "pcbdoc" not in info.get("extensions", []):
        raise ValueError(f"Unexpected module interface: {info}")
    board = ROOT / "crates/altium-pcbdoc/tests/boards/empty-outline.PcbDoc"
    with tempfile.TemporaryDirectory() as folder:
        subprocess.run([binary, "read", str(board), "--out", folder,
                        "--accept-overlay", "laminate"], check=True)
        if not (Path(folder) / "manifest.json").is_file():
            raise ValueError("The smoke test did not write manifest.json")


NOTICE_GLOBS = ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "UNLICENSE*")


def third_party(target, root=ROOT):
    """The licenses of every crate built into the program, as one text."""
    platform = "aarch64-apple-darwin" if target == "macos-universal" else target
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked",
         "--filter-platform", platform], cwd=root, text=True))
    members = set(meta["workspace_members"])
    used = {node["id"] for node in meta["resolve"]["nodes"]}
    parts = ["Third-party software built into dutchman-electronics, with its licenses.\n"]
    for pkg in sorted(meta["packages"], key=lambda p: (p["name"], p["version"])):
        if pkg["id"] in members or pkg["id"] not in used:
            continue
        folder = Path(pkg["manifest_path"]).parent
        parts.append("=" * 78)
        parts.append(f"{pkg['name']} {pkg['version']}  License: {pkg.get('license') or 'see below'}")
        if pkg.get("repository"):
            parts.append(pkg["repository"])
        texts = sorted({f for g in NOTICE_GLOBS for f in folder.glob(g) if f.is_file()})
        for text in texts:
            parts.append(f"--- {text.name} ---")
            parts.append(text.read_text(encoding="utf-8", errors="replace").rstrip())
        parts.append("")
    return "\n".join(parts) + "\n"


def package(binary, target, root=ROOT, notices=None):
    name = f"dutchman-electronics-{version()}-{target}"
    windows = target == "x86_64-pc-windows-msvc"
    files = [(Path(binary), "dutchman-electronics.exe" if windows else "dutchman-electronics")]
    files.append((root / "README.md", "README.md"))
    files.extend((p, p.name) for p in sorted(root.glob("LICENSE*")) if p.is_file())
    if (root / "INSTALL.md").is_file():
        files.append((root / "INSTALL.md", "INSTALL.md"))
    # The Hershey font data is built in, and its use restriction must travel with it.
    hershey = root / "crates/dutchman-electronics/fonts/hershey/README.txt"
    if hershey.is_file():
        files.append((hershey, "LICENSE-Hershey-fonts.txt"))
    dist = root / "dist"
    dist.mkdir(exist_ok=True)
    if notices is not None:
        third = root / "target" / f"THIRD-PARTY-LICENSES-{target}.txt"
        third.parent.mkdir(exist_ok=True)
        third.write_text(notices, encoding="utf-8", newline="\n")
        files.append((third, "THIRD-PARTY-LICENSES.txt"))
    archive = dist / (name + (".zip" if windows else ".tar.gz"))
    if windows:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for path, filename in files:
                output.write(path, f"{name}/{filename}")
    else:
        with tarfile.open(archive, "w:gz") as output:
            for path, filename in files:
                output.add(path, arcname=f"{name}/{filename}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8", newline="\n"
    )
    return archive


if __name__ == "__main__":
    command = sys.argv[1]
    if command == "version":
        value = version()
        if os.environ.get("GITHUB_EVENT_NAME") == "push":
            check_tag(os.environ["GITHUB_REF_NAME"], value)
        print(f"version={value}")
    elif command == "smoke":
        smoke(sys.argv[2])
    elif command == "package":
        package(sys.argv[2], sys.argv[3], notices=third_party(sys.argv[3]))
    else:
        raise ValueError(f"Unknown command: {command}")
