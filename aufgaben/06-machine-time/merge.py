"""Merge Level-6 outputs: keep the shortest program per pattern.

Usage: python3 merge.py <daten-dir> <candidate-dir>...
Every candidate dir holds out_level-6_*.txt files produced (and verified) by the solver.
"""
import pathlib
import sys


def programs(path):
    text = path.read_text().strip("\n")
    return [b for b in text.split("\n\n")] if text else []


data = pathlib.Path(sys.argv[1])
cands = [pathlib.Path(p) for p in sys.argv[2:]]
total_big = 0
for out in sorted(data.glob("out_level-6_*.txt")):
    best = programs(out)
    for c in cands:
        for f in c.rglob(out.name):
            other = programs(f)
            if len(other) != len(best):
                continue
            best = [o if o.count("\n") < b.count("\n") else b for b, o in zip(best, other)]
    out.write_text("".join(b + "\n\n" for b in best))
    lines = sum(b.count("\n") + 1 for b in best)
    if any(k in out.name for k in ("medium", "large", "enormous")):
        total_big += lines
    print(f"{out.name}: {lines}")
print(f"medium+large+enormous: {total_big}")
