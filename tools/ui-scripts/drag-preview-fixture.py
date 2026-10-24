# SPDX-License-Identifier: MIT
"""Synthetic Join Extrude, optionally over a target with 83 through holes."""
import json
import sys

commands = [{"cmd": "add_feature", "name": "Target", "def": {
    "type": "box", "plane": "xy", "corner": [0, 0], "length": 120,
    "width": 100, "height": 20, "operation": "new_body"}}]
sketch = "F2"
if sys.argv[1] == "curved":
    commands.append({"cmd": "sketch.create"})
    holes = [(x, y) for x in range(8, 115, 10) for y in range(8, 95, 10)
             if not (45 < x < 80 and 35 < y < 70)]
    assert len(holes) == 83
    commands.extend({"cmd": "sketch.add_circle", "sketch": sketch,
                     "center": [x, y], "diameter": 4} for x, y in holes)
    commands.append({"cmd": "add_feature", "def": {
        "type": "extrude", "profiles": [{"sketch": sketch, "region": f"r{{c{2*i+1}}}"}
                                         for i in range(len(holes))],
        "extent": {"type": "through_all"}, "operation": "cut", "participants": ["F1.b0"]}})
    sketch = "F4"
commands.extend([
    {"cmd": "sketch.create"},
    {"cmd": "sketch.add_rectangle", "sketch": sketch, "corner": [50, 45], "width": 20, "height": 10},
    {"cmd": "add_feature", "name": "Drag boss", "def": {
        "type": "extrude", "profiles": [{"sketch": sketch,
            "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
        "start": {"type": "offset", "offset": 20},
        "extent": {"type": "distance", "distance": 5}, "operation": "join", "participants": ["F1.b0"]}},
    {"expect": {"bodies": 1}},
])
json.dump(commands, sys.stdout)
