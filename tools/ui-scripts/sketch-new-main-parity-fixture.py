#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Public synthetic seeds; operations under test are performed by the GUI."""
import json
import sys

scenario = sys.argv[1]
if scenario == 'measure':
    commands = [
        {'cmd': 'sketch.create'},
        {'cmd': 'sketch.add_rectangle', 'sketch': 'F1', 'corner': [-60, -35], 'width': 120, 'height': 70},
        {'cmd': 'add_feature', 'def': {'type': 'extrude', 'profiles': [{'sketch': 'F1',
            'region': 'r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}'}],
            'extent': {'type': 'symmetric', 'distance': 15, 'full_length': False}, 'operation': 'new_body'}},
        {'cmd': 'set_feature_visible', 'uid': 'F1', 'visible': True},
    ]
else:
    commands = [
        {'cmd': 'add_feature', 'def': {'type': 'box', 'plane': 'xy', 'corner': [-35, -35],
            'length': 10, 'width': 10, 'height': 10, 'operation': 'new_body'}},
        {'cmd': 'sketch.create'},
    ]
    if scenario != 'first-dimension':
        # A construction guide keeps all pointer targets in the initial fit.
        commands.append({'cmd': 'sketch.circle', 'sketch': 'F2', 'mode': 'center',
                         'center': [0, 0], 'radius': 80, 'construction': True})
    if scenario == 'fillet':
        commands.append({'cmd': 'sketch.add_rectangle', 'sketch': 'F2', 'corner': [10, 10], 'width': 40, 'height': 30})
    elif scenario == 'symmetry':
        commands.extend({'cmd': 'sketch.circle', 'sketch': 'F2', 'mode': 'center', 'center': center, 'radius': radius}
                        for center, radius in [([-30, 20], 4), ([-27, -16], 5)])
    elif scenario == 'first-dimension':
        # The drawing command leaves four H/V relations and free sizes.
        # Legacy add_rectangle fixes its corner and adds width/height
        # dimensions, which correctly disqualify first-dimension scaling.
        commands.extend([
            {'cmd': 'sketch.rectangle', 'sketch': 'F2', 'mode': 'two_point', 'a': [10, 10], 'b': [20, 15]},
            {'cmd': 'sketch.circle', 'sketch': 'F2', 'mode': 'center', 'center': [35, 20], 'radius': 2},
        ])
    elif scenario != 'rounded':
        raise SystemExit('unknown scenario: ' + scenario)
json.dump([{'comment': 'SPDX-License-Identifier: MIT'}, *commands], sys.stdout, indent=2)
print()
