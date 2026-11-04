#!/usr/bin/env python3
"""Compile the skill's component examples and optionally check catalog coverage."""

import argparse
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--studio", default="studio", help="Installed Studio CLI executable")
    parser.add_argument("--platform-root", type=Path, help="Optional Studio Platform checkout")
    args = parser.parse_args()
    executable = shutil.which(args.studio)
    if executable is None:
        parser.error(f"Studio executable not found: {args.studio}")

    skill = Path(__file__).resolve().parents[1]
    references = skill / "references/components"
    examples = []
    components = set()
    for document in sorted(references.glob("*.md")):
        source = document.read_text()
        if document.name != "contracts.md":
            for name in re.findall(r"^## (\w+)$", source, re.M):
                if name in components:
                    parser.error(f"Duplicate component reference: {name}")
                components.add(name)
                section = source.split(f"## {name}\n", 1)[1].split("\n## ", 1)[0]
                if re.search(rf"<{re.escape(name)}(?:\s|/|>)", section) is None:
                    parser.error(f"Missing {name} example in {document}")
        for index, code in enumerate(re.findall(r"```svelte\n(.*?)\n```", source, re.S), 1):
            examples.append((f"{document.stem}-{index}", code))
    if not components or not examples:
        parser.error("No component references or examples found")

    if args.platform_root:
        source = (args.platform_root / "crates/studio-protocol/src/ui.rs").read_text()
        enum = source.split("pub enum NodeKind {", 1)[1].split("\n}", 1)[0]
        declared = set(re.findall(r"^    (\w+),", enum, re.M))
        if components != declared:
            parser.error(
                f"Catalog mismatch; missing={sorted(declared - components)}, "
                f"extra={sorted(components - declared)}"
            )
        print(f"All {len(declared)} declared component kinds have references and examples")

    failed = []
    with tempfile.TemporaryDirectory(prefix="studio-component-examples-") as directory:
        for name, code in examples:
            path = Path(directory) / f"{name}.studio"
            path.write_text(code + "\n")
            result = subprocess.run(
                [executable, "check", str(path)],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            if result.returncode:
                failed.append(name)
                print(f"FAIL {name}\n{result.stdout}{result.stderr}")
    if failed:
        raise SystemExit(f"{len(failed)} of {len(examples)} examples failed compiler checks")
    print(f"All {len(examples)} examples passed Studio source checks")
    print("Live rendering, event delivery, and mount-time values were not exercised")


if __name__ == "__main__":
    main()
