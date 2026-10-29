# SPDX-License-Identifier: MIT
"""The fastener generator's dimension tables (mitcad#900), read from
tools/libraries/make-fastener-library.py without mitcad-cli: every size of
a table is one of SIZES (a size spelt otherwise would be left out without
a word), every size has its diameter and the ISO 261 coarse pitch, the
lengths are ascending in the standards' length series, the dimensions
make a part (a head wider than the thread, a socket inside the head, a
countersunk head of 90 degrees, a washer's hole over the thread), each
standard's rows and default row, and every row of the library's earlier
versions is still there (designs may use them).

    python3 fastener-tables-test.py --libraries <tools/libraries>
"""

import argparse
import importlib.util
import math
import os
import sys

# ISO 261 coarse pitches.
COARSE_PITCH = {1.6: 0.35, 2: 0.4, 2.5: 0.45, 3: 0.5, 4: 0.7, 5: 0.8, 6: 1, 8: 1.25, 10: 1.5, 12: 1.75, 14: 2,
                16: 2, 20: 2.5, 24: 3}

# The nominal lengths of the screw standards' length series.
LENGTH_SERIES = {2, 2.5, 3, 4, 5, 6, 8, 10, 12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100, 110,
                 120, 130, 140, 150, 160, 180, 200}

# Each standard: its rows, first and last row, default row.
EXPECTED = {
    "iso4762": (190, "M1.6x2.5", "M24x200", "M5x16"),
    "iso4017": (170, "M1.6x2", "M24x150", "M5x20"),
    "iso10642": (121, "M3x8", "M20x100", "M5x16"),
    "iso4032": (14, "M1.6", "M24", "M5"),
    "iso7089": (14, "M1.6", "M24", "M5"),
}

# The rows of the library up to mitcad-parts 2.7.0 (M3 to M12), kept.
EARLIER_LENGTHS = {
    "iso4762": {
        "M3": [5, 6, 8, 10, 12, 16, 20, 25, 30],
        "M4": [6, 8, 10, 12, 16, 20, 25, 30, 35, 40],
        "M5": [8, 10, 12, 16, 20, 25, 30, 35, 40, 45, 50],
        "M6": [8, 10, 12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60],
        "M8": [10, 12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80],
        "M10": [16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100],
        "M12": [20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100, 110, 120],
    },
    "iso4017": {
        "M3": [6, 8, 10, 12, 16, 20, 25, 30],
        "M4": [8, 10, 12, 16, 20, 25, 30, 35, 40],
        "M5": [10, 12, 16, 20, 25, 30, 35, 40, 45, 50],
        "M6": [10, 12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60],
        "M8": [12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80],
        "M10": [16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100],
        "M12": [20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100, 110, 120],
    },
    "iso10642": {
        "M3": [8, 10, 12, 16, 20, 25, 30],
        "M4": [8, 10, 12, 16, 20, 25, 30, 35, 40],
        "M5": [8, 10, 12, 16, 20, 25, 30, 35, 40, 45, 50],
        "M6": [8, 10, 12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60],
        "M8": [10, 12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80],
        "M10": [12, 16, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100],
        "M12": [20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 80, 90, 100],
    },
}
EARLIER_SIZES = ["M3", "M4", "M5", "M6", "M8", "M10", "M12"]

FAILURES = []


def check(condition, message):
    if not condition:
        FAILURES.append(message)


def load(libraries):
    path = os.path.join(libraries, "make-fastener-library.py")
    spec = importlib.util.spec_from_file_location("make_fastener_library", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_tables(g):
    tables = {"iso4762": g.ISO4762, "iso4017": g.ISO4017, "iso10642": g.ISO10642, "iso4032": g.ISO4032,
              "iso7089": g.ISO7089}
    for size in g.SIZES:
        check(size in g.DIAMETER, f"{size}: no diameter")
        check(size in g.PITCH, f"{size}: no pitch")
        if size in g.DIAMETER:
            check(size == f"M{g.DIAMETER[size]:g}", f"{size}: diameter {g.DIAMETER[size]}")
            check(g.PITCH.get(size) == COARSE_PITCH.get(g.DIAMETER[size]),
                  f"{size}: pitch {g.PITCH.get(size)}, ISO 261 {COARSE_PITCH.get(g.DIAMETER[size])}")
    for sid, table in tables.items():
        for size in table:
            check(size in g.SIZES, f"{sid}: {size} is not one of SIZES {g.SIZES}")
    for sid in ("iso4762", "iso4017", "iso10642"):
        for size, entry in tables[sid].items():
            lengths = entry[-1]
            check(lengths == sorted(set(lengths)), f"{sid} {size}: lengths not ascending {lengths}")
            check(set(lengths) <= LENGTH_SERIES, f"{sid} {size}: lengths outside the series "
                                                 f"{sorted(set(lengths) - LENGTH_SERIES)}")
    for size, (dk, k, s, t, lengths) in g.ISO4762.items():
        d = g.DIAMETER.get(size, 0)
        check(dk > d and s < d and t < k and s * 2 / math.sqrt(3) < dk,
              f"iso4762 {size}: dk {dk}, k {k}, s {s}, t {t} make no socket head")
    for size, (s, k, lengths) in g.ISO4017.items():
        d = g.DIAMETER.get(size, 0)
        check(s > d and 0 < k < d, f"iso4017 {size}: s {s}, k {k} make no hexagon head")
    for size, (dk, k, s, t, lengths) in g.ISO10642.items():
        d = g.DIAMETER.get(size, 0)
        # A 90-degree head: its height half the difference of the diameters.
        check(abs(k - (dk - d) / 2) < 0.01, f"iso10642 {size}: k {k} is not (dk - d) / 2 = {(dk - d) / 2:g}")
        check(s < d and t < k and lengths[0] > k, f"iso10642 {size}: s {s}, t {t}, L {lengths[0]}")
    for size, (s, m) in g.ISO4032.items():
        d = g.DIAMETER.get(size, 0)
        check(s > d and 0.7 * d < m < d, f"iso4032 {size}: s {s}, m {m}")
    for size, (d1, d2, h) in g.ISO7089.items():
        d = g.DIAMETER.get(size, 0)
        check(d < d1 < d2 and 0 < h < d1, f"iso7089 {size}: d1 {d1}, d2 {d2}, h {h}")


def check_rows(g):
    items = {item["id"]: item for item in g.standards(g.SIZES)}
    for sid, (count, first, last, default) in EXPECTED.items():
        item = items.get(sid)
        if item is None:
            check(False, f"{sid}: not made")
            continue
        names = [r["name"] for r in item["rows"]]
        print(f"{sid}: {len(names)} rows, {names[0]} to {names[-1]}, default {item['default']}")
        check(len(names) == count, f"{sid}: {len(names)} rows, not {count}")
        check(len(set(names)) == len(names), f"{sid}: rows of the same name")
        check(names[0] == first and names[-1] == last, f"{sid}: rows {names[0]} to {names[-1]}, not {first} to "
                                                       f"{last}")
        check(item["default"] == default, f"{sid}: default row {item['default']}, not {default}")
        if sid in EARLIER_LENGTHS:
            missing = [f"{size}x{length}" for size, lengths in EARLIER_LENGTHS[sid].items() for length in lengths
                       if f"{size}x{length}" not in names]
        else:
            missing = [size for size in EARLIER_SIZES if size not in names]
        check(not missing, f"{sid}: the earlier rows {missing} are gone")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--libraries", required=True, help="tools/libraries")
    args = parser.parse_args()
    sys.path.insert(0, os.path.abspath(args.libraries))
    g = load(os.path.abspath(args.libraries))
    check_tables(g)
    check_rows(g)
    if FAILURES:
        sys.exit("fastener tables: FAILED:\n" + "\n".join(FAILURES))
    print("fastener tables: ok")


if __name__ == "__main__":
    main()
