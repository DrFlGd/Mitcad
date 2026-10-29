#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Public synthetic counterparts of the existing Move UI fixtures."""
import json
import sys

region = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"
commands = [{"comment": "SPDX-License-Identifier: MIT"}]


def add(command, **values):
    commands.append({"cmd": command, **values})


def extrude(sketch, profiles, distance, operation, **values):
    add("add_feature", **{"def": {"type": "extrude", "profiles": profiles,
        "extent": {"type": "distance", "distance": distance}, "operation": operation, **values}})


if sys.argv[1] == "face":
    add("sketch.create")
    add("sketch.add_rectangle", sketch="F1", corner=[0, 0], width=60, height=40)
    extrude("F1", [{"sketch": "F1", "region": region}], 20, "new_body")
else:
    add("sketch.create")
    add("sketch.add_circle", sketch="F1", center=[0, 0], diameter=30)
    extrude("F1", [{"sketch": "F1", "region": "r{c1}"}], 20, "new_body")
    add("sketch.create")
    add("sketch.add_rectangle", sketch="F3", corner=[0, 0], width=30, height=20)
    extrude("F3", [{"sketch": "F3", "region": region}], 10, "new_component")
    add("set_occurrence_transform", occurrence="Component1:1", transform={"translation": [60, 0, 0]})
    add("activate_component", component="Root")
    add("create_component", name="Pegs", transform={"translation": [0, 120, 0]})
    add("sketch.create")
    add("sketch.add_rectangle", sketch="F5", corner=[-8, -8], width=100, height=100)
    extrude("F5", [{"sketch": "F5", "region": region}], 4, "new_body")
    add("sketch.create")
    profiles = []
    for i in range(8):
        for j in range(8):
            add("sketch.add_circle", sketch="F7", center=[i * 12, j * 12], diameter=8)
            profiles.append({"sketch": "F7", "region": f"r{{c{2 * (i * 8 + j) + 1}}}"})
    extrude("F7", profiles, 30, "join", participants=["F6.b0"])
    add("activate_component", component="Root")
print(json.dumps(commands, indent=2))
