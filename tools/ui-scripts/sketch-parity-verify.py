# SPDX-License-Identifier: MIT
"""Verify synthetic canvas edits from actual model queries and the GUI save."""
import bisect
import json
import math
from pathlib import Path
import sys

root = Path(sys.argv[1])


def load(name):
    return json.loads((root / name).read_text(encoding='utf-8'))


def entities(state):
    return {entity['id']: entity for entity in state['entities']}


def points(state):
    return {key: value['at'] for key, value in entities(state).items() if value['type'] == 'point'}


def near(actual, expected, label, tolerance=1e-6):
    assert len(actual) == len(expected), label
    assert all(math.isfinite(a) and abs(a - b) <= tolerance for a, b in zip(actual, expected)), \
        f'{label}: {actual} != {expected}'


def same_state(actual, expected, label):
    a, b = entities(actual), entities(expected)
    assert a.keys() == b.keys(), f'{label}: entity IDs changed'
    for key in a:
        assert a[key]['type'] == b[key]['type'], f'{label}: {key} type changed'
        if a[key]['type'] == 'point':
            near(a[key]['at'], b[key]['at'], f'{label}: {key}')
        for field in ('start', 'end', 'control', 'points', 'knots', 'weights', 'degree'):
            assert a[key].get(field) == b[key].get(field), f'{label}: {key}/{field} changed'
    assert actual.get('constraints', []) == expected.get('constraints', []), f'{label}: constraints changed'
    assert actual.get('dimensions', []) == expected.get('dimensions', []), f'{label}: dimensions changed'


def evaluate(curve, fraction):
    """De Boor evaluation, independent of the inserted control-point layout."""
    degree = curve['degree']
    control = curve['control']
    count = len(control)
    knots = curve.get('knots') or ([0.0] * (degree + 1) +
        [i / (count - degree) for i in range(1, count - degree)] + [1.0] * (degree + 1))
    weights = curve.get('weights') or [1.0] * count
    parameter = knots[degree] + fraction * (knots[count] - knots[degree])
    span = min(count - 1, max(degree, bisect.bisect_right(knots, parameter) - 1))
    values = [[control[i][0] * weights[i], control[i][1] * weights[i], weights[i]]
              for i in range(span - degree, span + 1)]
    for order in range(1, degree + 1):
        for j in range(degree, order - 1, -1):
            index = span - degree + j
            denominator = knots[index + degree - order + 1] - knots[index]
            alpha = (parameter - knots[index]) / denominator if denominator else 0.0
            values[j] = [(1 - alpha) * a + alpha * b for a, b in zip(values[j - 1], values[j])]
    return [value / values[degree][2] for value in values[degree][:2]]


before = load('before.json')
dragged = load('drag.json')
old, moved = points(before), points(dragged)
lines = [entity for entity in entities(before).values() if entity['type'] == 'line' and not entity.get('axis')]
assert len(lines) == 2, 'the fixture needs exactly two free lines'
group = {line[field] for line in lines for field in ('start', 'end')}
standalone = [key for key, value in old.items() if abs(value[0] - 45) < 1e-6 and abs(value[1] - 10) < 1e-6]
assert len(standalone) == 1, 'the independent selected point is missing'
group.add(standalone[0])
first = lines[0]['start']
offset = [a - b for a, b in zip(moved[first], old[first])]
# Pointer coordinates are rounded to logical pixels. The group must move
# by one identical offset; its requested 5/7 mm target is pixel-quantized.
near(offset, [5, 7], 'pointer drag target', 0.5)
assert old.keys() == moved.keys(), 'the drag created or deleted points'
for key in old:
    wanted = [value + delta for value, delta in zip(old[key], offset)] if key in group else old[key]
    near(moved[key], wanted, f'group drag {key}')
same_state(load('drag-undo.json'), before, 'group undo')
same_state(load('drag-redo.json'), dragged, 'group redo')

inserted = load('insert.json')
a, b = entities(dragged), entities(inserted)
splines = [entity for entity in a.values() if entity['type'] == 'spline']
assert len(splines) == 1, 'the control spline is missing'
curve = splines[0]
now = b[curve['id']]
assert len(curve['control']) == 3 and len(now['control']) == 4, 'Insert Point did not add one control point'
assert all(point in now['control'] for point in curve['control']), 'Insert Point lost an existing control ID'
assert len(b) == len(a) + 1, 'Insert Point did not add exactly one entity'
for sample in range(21):
    near(evaluate(now['geometry'], sample / 20), evaluate(curve['geometry'], sample / 20),
         f'spline shape sample {sample}')
for key, value in points(dragged).items():
    if key not in curve['control']:
        near(points(inserted)[key], value, f'Insert Point unrelated point {key}')
same_state(load('insert-undo.json'), dragged, 'Insert Point undo')
same_state(load('insert-redo.json'), inserted, 'Insert Point redo')

saved = load('sketch-parity.mitcad')
feature = next(feature for feature in saved['features'] if feature['uid'] == 'F2')
saved_entities = entities(feature)
assert saved_entities.keys() == b.keys(), 'the GUI save lost sketch entities'
for key, point in points(inserted).items():
    near(saved_entities[key]['at'], point, f'persisted {key}')
assert saved_entities[curve['id']]['control'] == now['control'], 'the GUI save lost the inserted spline point'
created = next(feature for feature in saved['features'] if feature['uid'] == 'F3')
assert created['type'] == 'sketch' and created['plane'] == 'xy', 'the selected origin plane was not saved as XY'
new_points = [entity['at'] for entity in created['entities'] if entity['type'] == 'point' and not entity.get('axis')]
assert len(new_points) == 1, 'the new XY sketch did not retain its created point'
near(new_points[0], [5, 5], 'new origin-plane sketch point', 0.5)
print('Sketch parity evidence: group offset', offset, 'one-point spline insertion preserves 21 shape samples;',
      'both undos/redos and actual GUI save verified; origin XY sketch retained')
