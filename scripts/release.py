#!/usr/bin/env python3
"""Native release assembly and acceptance. Python 3.10+, standard library only.

Only allowlisted files enter a delivery. Never archive a build directory, the
checkout, environment dumps, receipts, PDBs, or a developer's installed tools.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ("doinstall", "doinstall-builder", "doinstall-launcher", "doinstall-server")
RUST = "1.95.0"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args, capture=False, env=None):
    result = subprocess.run([str(arg) for arg in args], cwd=ROOT, env=env,
                            check=True, text=True, encoding="utf-8", stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def version(tag=None):
    # Cargo's ordinary three-component package version is the release authority;
    # generated application build stamps are not doinstall release versions.
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    value = re.search(r'^version = "([^"]+)"$', text, re.M).group(1)
    require(re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", value),
            "Release version must be MAJOR.MINOR.PATCH")
    if tag is not None:
        require(tag == "v" + value, "Tag must exactly match Cargo.toml: v" + value)
    return value


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def private_markers():
    paths = [ROOT, Path.home(), Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")),
             Path(run("rustc", "--print", "sysroot", capture=True))]
    for key in ("RUNNER_TEMP", "GITHUB_WORKSPACE", "CARGO_TARGET_DIR"):
        if os.environ.get(key):
            paths.append(Path(os.environ[key]))
    return {str(path) for path in paths if len(str(path)) > 3}


def scan(data, label, markers):
    # Also check UTF-16LE used by Windows resources. Never print matched secret
    # values. These checks complement the closed inventory; they are not a claim
    # that arbitrary secrets can be recognized perfectly by pattern matching.
    text = data.decode("latin1") + "\n" + data.decode("utf-16le", errors="ignore")
    folded = text.replace("\\", "/").casefold()
    for marker in markers:
        normalized = marker.replace("\\", "/").removeprefix("//?/").casefold()
        require(normalized not in folded, f"Private build path in {label}")
    patterns = (
        r"[A-Za-z]:[/\\](?:Users|agent|actions-runner|tmp)[/\\]",
        r"/(?:home/[^/\s]+|Users/[^/\s]+|root/\.cargo|__w/|github/workspace/)",
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|AKIA[A-Z0-9]{16})\b",
        r"(?i)(?:GITHUB_TOKEN|ACTIONS_RUNTIME_TOKEN|AWS_SECRET_ACCESS_KEY)\s*=\s*\S+",
    )
    for pattern in patterns:
        require(not re.search(pattern, text), f"Private content pattern in {label}")


def release_env():
    env = os.environ.copy()
    # An encoded list avoids shell quoting bugs for Windows home directories
    # containing spaces. Release compilation deliberately owns this policy.
    flags = []
    paths = [(ROOT, "/doinstall"),
             (Path(run("rustc", "--print", "sysroot", capture=True)), "/rust-toolchain"),
             (Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")), "/cargo"),
             (Path.home(), "/build-home")]
    for path, public in paths:
        for spelling in sorted({str(path), str(path).replace("\\", "/"),
                                str(path).removeprefix("\\\\?\\")}):
            flags.append(f"--remap-path-prefix={spelling}={public}")
    if os.name == "nt":
        flags += ["-C", "target-feature=+crt-static", "-C", "link-arg=/PDBALTPATH:doinstall.pdb"]
    env.pop("RUSTFLAGS", None)
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    return env


def build_command(*args, env=None):
    if os.name == "nt":
        # All arguments here are local generated paths or validated versions;
        # subprocess handles cmd's quoting through the normal .cmd dispatch.
        return run(ROOT / "build.cmd", *args, env=env)
    return run("bash", ROOT / "build.sh", *args, env=env)


def licenses(stage, env):
    target = "x86_64-pc-windows-msvc" if os.name == "nt" else "x86_64-unknown-linux-gnu"
    metadata = json.loads(run("cargo", "metadata", "--locked", "--format-version", "1",
                              "--filter-platform", target, capture=True, env=env))
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [next(p["id"] for p in metadata["packages"] if p["name"] == "doinstall" and p["source"] is None)]
    included = set()
    while pending:
        identifier = pending.pop()
        if identifier not in included:
            included.add(identifier)
            pending.extend(nodes[identifier]["dependencies"])
    notices = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["source"] is None or package["id"] not in included:
            continue
        directory = Path(package["manifest_path"]).parent
        # Preserve all supplied license/notice texts, including dual-license
        # alternatives, rather than copying a license expression alone.
        name = package["name"] + "-" + package["version"]
        files = []
        for entry in sorted(directory.iterdir()):
            if entry.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")):
                if entry.is_dir():
                    files.extend(p for p in entry.rglob("*") if p.is_file())
                elif entry.is_file():
                    files.append(entry)
        if not files:
            # These published crates omit their upstream license texts. The
            # checked-in copies are tied to exact versions and upstream commits.
            directory = ROOT / "packaging" / "licenses" / name
            files = sorted(directory.glob("LICENSE*"))
        require(files, f"Missing dependency license text: {name}")
        destination = stage / "LICENSES" / name
        destination.mkdir(parents=True)
        for file in files:
            relative = file.relative_to(directory)
            (destination / relative).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(file, destination / relative)
        notices.append({"name": package["name"], "version": package["version"],
                        "license": package["license"],
                        "texts": [f"LICENSES/{name}/{p.relative_to(directory).as_posix()}" for p in files]})
    write_json(stage / "THIRD-PARTY.json", notices)


def inventory(stage):
    return {p.relative_to(stage).as_posix(): sha(p.read_bytes())
            for p in sorted(stage.rglob("*")) if p.is_file()}


def archive_tools(stage, archive, windows):
    if windows:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for name in inventory(stage):
                output.write(stage / name, name)
    else:
        with tarfile.open(archive, "w:gz", format=tarfile.PAX_FORMAT) as output:
            for name in inventory(stage):
                info = output.gettarinfo(str(stage / name), arcname=name)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                with (stage / name).open("rb") as source:
                    output.addfile(info, source)


def archive_entries(archive):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            entries = []
            for entry in source.infolist():
                mode = entry.external_attr >> 16
                require(not entry.is_dir() and mode & 0o170000 in (0, 0o100000),
                        "Portable archives must contain only regular files")
                entries.append((entry.orig_filename, source.read(entry), mode))
    else:
        with tarfile.open(archive, "r:gz") as source:
            entries = []
            for entry in source:
                require(entry.isfile(), "Portable archives must contain only regular files")
                entries.append((entry.name, source.extractfile(entry).read(), entry.mode))
    names = [name for name, _, _ in entries]
    require(len(names) == len(set(names)), "Duplicate archive entries")
    for name in names:
        require(not name.startswith("/") and "\\" not in name and ":" not in name and
                all(part not in ("", ".", "..") for part in name.split("/")), "Unsafe archive path")
    return entries


def query_tools(directory, windows):
    suffix = ".exe" if windows else ""
    result = json.loads(run(directory / ("doinstall-builder" + suffix), "--print-tool-paths", capture=True))
    for field, name in (("Installer", "doinstall"), ("LauncherTemplate", "doinstall-launcher")):
        require(Path(result[field]).resolve() == (directory / (name + suffix)).resolve(),
                "Builder did not discover its adjacent " + name)
    for name in ("doinstall", "doinstall-builder", "doinstall-server"):
        run(directory / (name + suffix), "--help", capture=True)


def inspect_installer(installer, expected, markers):
    data = installer.read_bytes()
    start, length, manifest_length, magic = struct.unpack("<QQQ16s", data[-40:])
    require(magic == b"DOINSTALL-SE-v1!", "Invalid installer footer")
    require(start + length + manifest_length == len(data) - 40, "Invalid installer bounds")
    scan(data[:start], "installer launcher", markers)
    manifest_data = data[start + length:-40]
    scan(manifest_data, "installer manifest", markers)
    manifest = json.loads(manifest_data)
    with zipfile.ZipFile(io.BytesIO(data[start:start + length])) as source:
        runtime = manifest["installer"]
        allowed = {runtime, runtime.removesuffix(".exe") + ".yaml"}
        allowed |= {"payload/" + name for name in expected}
        # Directory records carry permissions but no data.
        files = {entry.filename for entry in source.infolist() if not entry.is_dir()}
        require(files == allowed, "Unexpected self-installer file inventory")
        for name in files:
            value = source.read(name)
            scan(value, "installer:" + name, markers)
            if name.startswith("payload/"):
                require(sha(value) == expected[name[8:]], "Installer payload differs from portable tools")


def accept(archive, installer, expected, windows, markers):
    entries = archive_entries(archive)
    require({name: sha(data) for name, data, _ in entries} == expected, "Portable archive inventory mismatch")
    with tempfile.TemporaryDirectory(prefix="doinstall-release-acceptance-") as temporary:
        temporary = Path(temporary)
        extracted = temporary / "portable tools"
        extracted.mkdir()
        for name, data, mode in entries:
            scan(data, "portable:" + name, markers)
            path = extracted / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            if not windows:
                path.chmod(mode)
        query_tools(extracted, windows)
        inspect_installer(installer, expected, markers)
        root, commands, source = (temporary / name for name in ("installed tools", "commands", "retained source"))
        run(installer, "--extract-to", source, "--", "--non-interactive", "--install-dir", root,
            "--bin-dir", commands, "--set", "add-to-path=false")
        suffix = ".exe" if windows else ""
        query = json.loads(run(commands / ("doinstall-builder" + suffix), "--print-tool-paths", capture=True))
        installed = Path(query["Installer"]).parent
        require(installed.resolve().is_relative_to(root.resolve()), "Installed tools escaped temporary root")
        query_tools(installed, windows)
        for name, digest in expected.items():
            require(sha((installed / name).read_bytes()) == digest, "Installed payload mismatch: " + name)
        # Exercise the downloaded builder/runtime/template together: assemble a
        # second self-installer entirely from the portable tools and then run it.
        payload = temporary / "app payload"
        payload.mkdir()
        shutil.copy2(extracted / ("doinstall" + suffix), payload / ("example" + suffix))
        config = {"schema": 1, "application": {"id": "release-smoke", "name": "Release smoke", "version": "1.0.0"},
                  "platform": "windows" if windows else "linux", "architecture": "x64", "payload": str(payload),
                  "runtime": {"interactive": False, "commands": [{"name": "release-smoke", "executable": "example" + suffix}]}}
        # JSON is a YAML subset accepted by the ordinary public builder.
        write_json(temporary / "app.yaml", config)
        run(extracted / ("doinstall-builder" + suffix), "--config", temporary / "app.yaml",
            "--installer", extracted / ("doinstall" + suffix),
            "--launcher-template", extracted / ("doinstall-launcher" + suffix),
            "--output", temporary / "delivery", "--self-extracting", "--result-file", temporary / "app-result.json")
        app = Path(json.loads((temporary / "app-result.json").read_text())["FilePath"])
        run(app, "--extract-to", temporary / "app source", "--", "--non-interactive",
            "--install-dir", temporary / "app install", "--bin-dir", temporary / "app commands",
            "--set", "add-to-path=false")
        run(temporary / "app commands" / ("release-smoke" + suffix), "--help", capture=True)
        run(temporary / "app install" / "installer" / ("installer" + suffix), "uninstall", "--non-interactive")
        run(root / "installer" / ("installer" + suffix), "uninstall", "--non-interactive")
        require(not root.exists() and not (commands / ("doinstall-builder" + suffix)).exists(),
                "Uninstall left active temporary tools")


def build(output, tag):
    value = version(tag)
    windows = os.name == "nt"
    require(platform.system() in ("Windows", "Linux") and platform.machine().lower() in ("amd64", "x86_64"),
            "Release build requires native Windows/Linux x64")
    if not windows:
        os_release = Path("/etc/os-release").read_text(encoding="utf-8")
        require(re.search(r'^ID=ubuntu$', os_release, re.M) and
                re.search(r'^VERSION_ID="22\.04"$', os_release, re.M) and
                platform.libc_ver() == ("glibc", "2.35"),
                "Linux release assembly requires the tested Ubuntu 22.04 / glibc 2.35 baseline")
    output = output.resolve()
    require(not output.is_relative_to(ROOT) and not ROOT.is_relative_to(output), "Release output must be outside source")
    require(not output.exists(), "Release output already exists; choose a fresh directory")
    rustc = run("rustc", "--version", "--verbose", capture=True)
    require(f"release: {RUST}\n" in rustc + "\n", "Wrong Rust toolchain")
    env = release_env()
    target = json.loads(run("cargo", "metadata", "--locked", "--format-version", "1", "--no-deps", capture=True, env=env))
    for key in ("target_directory", "build_directory"):
        if target.get(key):
            require(not Path(target[key]).resolve().is_relative_to(ROOT), "Cargo output is inside source")
    markers = private_markers() | {target["target_directory"]}
    output.mkdir(parents=True)
    with tempfile.TemporaryDirectory(prefix="doinstall-release-build-") as work:
        work = Path(work)
        stage = work / "tools"
        build_command("build", "--stage-dir", stage, env=env)
        if not windows:
            for name in TOOLS:
                binary = (stage / name).read_bytes()
                required = [tuple(map(int, match.split(b"."))) for match in
                            re.findall(rb"GLIBC_(\d+\.\d+)", binary)]
                require(required and max(required) <= (2, 35), "Tool exceeds glibc 2.35 baseline: " + name)
                require((stage / name).stat().st_mode & 0o111, "Missing Linux executable permission: " + name)
        shutil.copyfile(ROOT / "LICENSE.md", stage / "LICENSE.md")
        shutil.copyfile(ROOT / "packaging" / "PORTABLE-README.md", stage / "README.md")
        licenses(stage, env)
        metadata = {"schema": 1, "version": value, "tag": "v" + value,
                    "source_commit": run("git", "rev-parse", "HEAD", capture=True),
                    "source_dirty": bool(run("git", "status", "--porcelain", capture=True)),
                    "rustc": rustc, "cargo": run("cargo", "--version", capture=True),
                    "platform": "windows" if windows else "linux", "architecture": "x64",
                    "target": "x86_64-pc-windows-msvc" if windows else "x86_64-unknown-linux-gnu",
                    "compatibility_baseline": "Windows Server 2022 x64" if windows else "Ubuntu 22.04 x64, glibc 2.35",
                    "files": inventory(stage)}
        write_json(stage / "RELEASE.json", metadata)
        expected = inventory(stage)
        # Pass the exact same allowlisted files to the existing public recipe.
        # installer::assemble copies distribution data in addition to tools.
        build_command("package", "--output-parent", work / "installers", "--version", value,
                      "--result-file", work / "result.json", "--distribution-dir", stage, env=env)
        source = Path(json.loads((work / "result.json").read_text())["FilePath"])
        stem = f"doinstall-v{value}-{'windows' if windows else 'linux'}-x64"
        installer = output / (stem + "-setup" + (".exe" if windows else ""))
        shutil.copy2(source, installer)
        archive = output / (stem + ("-tools.zip" if windows else "-tools.tar.gz"))
        archive_tools(stage, archive, windows)
        accept(archive, installer, expected, windows, markers | {str(work)})
        write_json(output / (stem + "-metadata.json"), metadata)
        sums = "".join(f"{sha(p.read_bytes())}  {p.name}\n" for p in sorted(output.iterdir()))
        (output / "SHA256SUMS").write_text(sums, encoding="ascii")
    print("Native release acceptance passed: " + str(output))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("version")
    validate.add_argument("--tag")
    assemble = commands.add_parser("build")
    assemble.add_argument("--tag")
    assemble.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "version":
        print(version(args.tag))
    else:
        build(args.output, args.tag)


if __name__ == "__main__":
    main()
