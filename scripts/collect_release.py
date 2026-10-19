#!/usr/bin/env python3
"""Validate both native artifact sets and make a closed, release-ready directory."""
import argparse
import json
from pathlib import Path
import shutil

from release import require, sha, version, write_json


def collect(source, output, tag, commit, publishing=False):
    value = version(tag)
    require(not output.exists(), "Collection output must be fresh")
    assets = {}
    metadata = []
    for system, archive, executable in (("windows", "zip", ".exe"), ("linux", "tar.gz", "")):
        stem = f"doinstall-v{value}-{system}-x64"
        expected = {stem + "-tools." + archive, stem + "-setup" + executable, stem + "-metadata.json"}
        directory = source / ("native-" + system)
        require({p.name for p in directory.iterdir()} == expected | {"SHA256SUMS"}, "Unexpected native artifact inventory")
        lines = (directory / "SHA256SUMS").read_text(encoding="ascii").splitlines()
        sums = dict(line.split("  ", 1)[::-1] for line in lines)
        require(len(lines) == len(sums) and set(sums) == expected, "Invalid native checksum inventory")
        for name in expected:
            path = directory / name
            require(path.is_file() and not path.is_symlink() and sha(path.read_bytes()) == sums[name], "Native checksum mismatch")
            assets[name] = path
        info = json.loads((directory / (stem + "-metadata.json")).read_text(encoding="utf-8"))
        require(info["version"] == value and info["tag"] == "v" + value and
                info["source_commit"] == commit and info["platform"] == system and
                info["architecture"] == "x64", "Native metadata mismatch")
        if publishing:
            require(not info["source_dirty"], "Cannot publish dirty-source artifacts")
        metadata.append(info)
    output.mkdir(parents=True)
    for name, source_path in assets.items():
        shutil.copyfile(source_path, output / name)
    write_json(output / "RELEASE-METADATA.json", {"schema": 1, "tag": "v" + value,
               "source_commit": commit, "platforms": metadata})
    (output / "SHA256SUMS").write_text("".join(
        f"{sha(p.read_bytes())}  {p.name}\n" for p in sorted(output.iterdir())), encoding="ascii")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--tag")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--publishing", action="store_true")
    args = parser.parse_args()
    collect(args.source, args.output, args.tag, args.commit, args.publishing)


if __name__ == "__main__":
    main()
