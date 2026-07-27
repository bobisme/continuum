#!/usr/bin/env python3
"""Generate MANIFEST.md for the Continuum dossier."""
from __future__ import annotations

from collections import Counter
from datetime import date
from pathlib import Path
import hashlib
import re

ROOT = Path(__file__).resolve().parents[1]
EXCLUDE = {Path("MANIFEST.md"), Path("SHA256SUMS.txt")}


def markdown_title(path: Path) -> str:
    try:
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("# "):
                return line[2:].strip()
    except UnicodeDecodeError:
        pass
    return ""


def word_count(path: Path) -> int:
    if path.suffix.lower() != ".md":
        return 0
    return len(re.findall(r"\b\w[\w'’+-]*\b", path.read_text(encoding="utf-8")))


def main() -> None:
    files = [
        p for p in ROOT.rglob("*")
        if p.is_file() and p.relative_to(ROOT) not in EXCLUDE and "__pycache__" not in p.parts
    ]
    files.sort(key=lambda p: p.relative_to(ROOT).as_posix())
    by_ext = Counter((p.suffix.lower() or "[none]") for p in files)
    top = Counter((p.relative_to(ROOT).parts[0] if len(p.relative_to(ROOT).parts) > 1 else ".") for p in files)
    total_bytes = sum(p.stat().st_size for p in files)
    md_words = sum(word_count(p) for p in files)

    out = [
        "# Dossier Manifest — Revision 3",
        "",
        f"**Generated:** {date.today().isoformat()}  ",
        "**Root archive name:** `continuum-project-dossier-revision-3/`  ",
        "**Integrity file:** `SHA256SUMS.txt`",
        "",
        "## Summary",
        "",
        f"- Inventoried files, excluding this manifest and checksums: **{len(files)}**",
        f"- Bytes, excluding this manifest and checksums: **{total_bytes:,}**",
        f"- Approximate Markdown words: **{md_words:,}**",
        f"- ADRs: **{len(list((ROOT / 'adr').glob('[0-9][0-9][0-9][0-9]-*.md')))}**",
        f"- RFCs: **{len(list((ROOT / 'rfcs').glob('[0-9][0-9][0-9][0-9]-*.md')))}**",
        f"- Research programs: **{len(list((ROOT / 'research').glob('[0-9][0-9]-*.md')))}**",
        f"- Lean source modules: **{len(list((ROOT / 'lean').rglob('*.lean')))}**",
        "",
        "### By extension",
        "",
        "| Extension | Files |",
        "|---|---:|",
    ]
    for ext, count in sorted(by_ext.items()):
        out.append(f"| `{ext}` | {count} |")

    out += ["", "### By top-level area", "", "| Area | Files |", "|---|---:|"]
    for area, count in sorted(top.items()):
        out.append(f"| `{area}` | {count} |")

    out += ["", "## Complete inventory", ""]
    grouped: dict[str, list[Path]] = {}
    for path in files:
        rel = path.relative_to(ROOT)
        area = rel.parts[0] if len(rel.parts) > 1 else "."
        grouped.setdefault(area, []).append(path)
    for area in sorted(grouped, key=lambda x: (x != ".", x)):
        out += [f"### `{area}`", ""]
        for path in grouped[area]:
            rel = path.relative_to(ROOT)
            title = markdown_title(path) if path.suffix.lower() == ".md" else ""
            suffix = f" — {title}" if title else ""
            out.append(f"- `{rel.as_posix()}` — {path.stat().st_size:,} bytes{suffix}")

    (ROOT / "MANIFEST.md").write_text("\n".join(out) + "\n", encoding="utf-8")

    checksum_paths = [
        p for p in ROOT.rglob("*")
        if p.is_file()
        and p.relative_to(ROOT) != Path("SHA256SUMS.txt")
        and "__pycache__" not in p.parts
    ]
    checksum_lines = []
    for path in sorted(checksum_paths, key=lambda p: p.relative_to(ROOT).as_posix()):
        checksum = hashlib.sha256(path.read_bytes()).hexdigest()
        checksum_lines.append(f"{checksum}  ./{path.relative_to(ROOT).as_posix()}")
    (ROOT / "SHA256SUMS.txt").write_text("\n".join(checksum_lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
