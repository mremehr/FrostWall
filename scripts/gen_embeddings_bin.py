#!/usr/bin/env python3
"""Convert clip_embeddings.rs to compact binary format.

Binary format:
  [u32 LE] number_of_categories
  For each category:
    [u32 LE] name_length_in_bytes
    [UTF-8]  name_bytes
    [f32 LE x 512] embedding_values
"""

import re
import struct
import sys
from pathlib import Path

EMBEDDING_DIM = 512


def parse_rust_embeddings(rust_path: Path) -> list[tuple[str, list[float]]]:
    """Parse CATEGORY_EMBEDDINGS from a Rust source file."""
    content = rust_path.read_text()

    # Find all category blocks: ("name", [...])
    # Pattern: ("category_name", [float, float, ...])
    categories = []

    # Split by category start pattern
    blocks = re.split(r'\(\s*"(\w+)",\s*\[', content)

    # blocks[0] is header, then alternating: name, float_block
    for i in range(1, len(blocks), 2):
        name = blocks[i]
        if i + 1 >= len(blocks):
            break
        float_block = blocks[i + 1]

        # Extract all floats from this block (up to the closing ])
        bracket_end = float_block.find("]")
        if bracket_end < 0:
            continue
        float_text = float_block[:bracket_end]

        floats = re.findall(r"[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?", float_text)
        values = [float(f) for f in floats]

        if len(values) == EMBEDDING_DIM:
            categories.append((name, values))
        else:
            print(f"Warning: {name} has {len(values)} values, expected {EMBEDDING_DIM}", file=sys.stderr)

    return categories


def write_binary(categories: list[tuple[str, list[float]]], output_path: Path):
    """Write embeddings to compact binary format."""
    with open(output_path, "wb") as f:
        # Header: number of categories
        f.write(struct.pack("<I", len(categories)))

        for name, values in categories:
            name_bytes = name.encode("utf-8")
            # Name length + name bytes
            f.write(struct.pack("<I", len(name_bytes)))
            f.write(name_bytes)
            # Embedding values as f32 LE
            for v in values:
                f.write(struct.pack("<f", v))

    size_kb = output_path.stat().st_size / 1024
    print(f"Wrote {len(categories)} categories to {output_path} ({size_kb:.1f} KB)")


def main():
    script_dir = Path(__file__).parent
    project_dir = script_dir.parent

    rust_path = project_dir / "src" / "clip_embeddings.rs"
    output_path = project_dir / "data" / "embeddings.bin"

    if not rust_path.exists():
        print(f"Error: {rust_path} not found", file=sys.stderr)
        sys.exit(1)

    categories = parse_rust_embeddings(rust_path)
    print(f"Parsed {len(categories)} categories from {rust_path.name}")
    for name, _ in categories:
        print(f"  - {name}")

    write_binary(categories, output_path)


if __name__ == "__main__":
    main()
