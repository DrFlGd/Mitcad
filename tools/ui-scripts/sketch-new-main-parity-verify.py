#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Check real GUI queries, undo/redo, and the public synthetic GUI save."""
import json
import math
from pathlib import Path
import sys

scenario, directory = sys.argv[1:]
root = Path(directory)


def load(name):
    return json.loads((root / name).read_text(encoding='utf-8'))


def near(actual, expected, label):
    assert math.isfinite(actual) and abs(actual - expected) <= 1e-6, f'{label}: {actual} != {expected}'


def equal(actual, expected, label):
    if isinstance(actual, bool) or isinstance(expected, bool):
        assert actual == expected, label
    elif isinstance(actual, (int, float)) and isinstance(expected, (int, float)):
        near(actual, expected, label)
    elif isinstance(actual, dict) and isinstance(expected, dict):
        assert actual.keys() == expected.keys(), f'{label}: keys changed'
        for key in actual:
            equal(actual[key], expected[key], f'{label}/{key}')
    elif isinstance(actual, list) and isinstance(expected, list):
        assert len(actual) == len(expected), f'{label}: list length changed'
        for index, (a, b) in enumerate(zip(actual, expected)):
            equal(a, b, f'{label}/{index}')
    else:
        assert actual == expected, f'{label}: {actual!r} != {expected!r}'


def records(state, key):
    return {record['id']: record for record in state.get(key, [])}


def same(actual, expected, label):
    for key in ('entities', 'constraints', 'dimensions'):
        equal(records(actual, key), records(expected, key), label + '/' + key)


def curves(state, kind):
    return [e for e in state['entities'] if e['type'] == kind and not e.get('construction') and not e.get('axis')]


def count(state, kind):
    return sum(c['type'] == kind for c in state['constraints'])


saved = load('model.mitcad')
baseline = load('baseline.mitcad')
if scenario == 'measure':
    for key in ('features', 'parameters', 'analyses', 'components', 'occurrences', 'annotations'):
        equal(saved.get(key), baseline.get(key), 'read-only Measure save/' + key)
    report = {'scenario': scenario, 'unchanged_saved_model': True,
              'interaction_evidence': 'Driver requires body vertices, sketch points, body, Keep/Keep Row, retained annotations '
                                      'after Escape, history restoration/copy/clear and two undo steps back to the baseline.'}
else:
    before, after = load('before.json'), load('after.json')
    same(load('action-undo.json'), load('first.json') if scenario == 'rounded' else before, 'action undo')
    same(load('action-redo.json'), after, 'action redo')
    if scenario == 'rounded':
        first = load('first.json')
        same(load('first-undo.json'), before, 'first rectangle undo')
        same(load('first-redo.json'), first, 'first rectangle redo')
        near(len(curves(first, 'arc')), 4, 'first rectangle arc count')
        near(len(curves(after, 'arc')), 8, 'both rectangle arc count')
        equal(sorted(e['geometry']['radius'] for e in curves(after, 'arc')), [2.0] * 4 + [3.0] * 4, 'typed radii')
        assert len(curves(after, 'line')) == 8, 'rounded rectangle straight sides missing'
        assert count(after, 'tangent') == 16 and count(after, 'equal') == 6, 'rounded corners lost tangent/equal relations'
        assert sum(d['type'] == 'radius' for d in after['dimensions']) == 2, 'one radius dimension per rectangle required'
        equal(sorted(r['area'] for r in after['regions']),
              sorted([1200 - 9 * (4 - math.pi), 800 - 4 * (4 - math.pi)]), 'two rounded profile areas')
    elif scenario == 'fillet':
        assert len(curves(before, 'line')) == 4 and not curves(before, 'arc'), 'sharp seed changed'
        assert len(curves(after, 'line')) == 4 and len(curves(after, 'arc')) == 4, 'not all four corners filleted'
        for curve in curves(after, 'arc'):
            near(curve['geometry']['radius'], 3, 'shared fillet radius')
        assert count(after, 'tangent') == 8 and count(after, 'equal') == 3, 'multi-corner relations missing'
        assert sum(d['type'] == 'radius' for d in after['dimensions']) == 1, 'fillet must use one radius dimension'
        assert len(after['regions']) == 1, 'fillet broke the closed profile'
        near(after['regions'][0]['area'], 1200 - 9 * (4 - math.pi), 'filleted profile area')
    elif scenario == 'symmetry':
        circles = curves(after, 'circle')
        assert len(circles) == 2, 'Symmetry changed curve count'
        assert count(after, 'symmetric') == count(before, 'symmetric') + 1, 'one symmetric constraint required'
        points = records(after, 'entities')
        a, b = [points[e['center']]['at'] for e in circles]
        near(a[0], b[0], 'mirrored circle x')
        near(a[1], -b[1], 'mirrored circle y')
        near(circles[0]['geometry']['radius'], circles[1]['geometry']['radius'], 'mirrored radii')
        assert any(e.get('axis') == 'x' for e in after['entities']), 'actual x-axis pick was not retained'
    elif scenario == 'first-dimension':
        old, new = records(before, 'entities'), records(after, 'entities')
        assert before['dof'] == 7 and not before['dimensions'], 'seed must have free sizes and no prior dimension'
        assert not any(e.get('fixed') for e in old.values()), 'seed must have no fixed non-origin geometry'
        assert count(before, 'horizontal') == 2 and count(before, 'vertical') == 2, 'rectangle H/V relations missing'
        assert len(before['constraints']) == 4 and len(curves(before, 'line')) == 4, 'unexpected seed relations/curves'
        assert old.keys() == new.keys(), 'first dimension created or removed geometry'
        for key, entity in old.items():
            if entity['type'] == 'point':
                expected = entity['at'] if entity.get('axis') else [10 * coordinate for coordinate in entity['at']]
                equal(new[key]['at'], expected, 'whole-sketch scale/' + key)
        for old_circle, new_circle in zip(curves(before, 'circle'), curves(after, 'circle')):
            near(new_circle['geometry']['radius'], 10 * old_circle['geometry']['radius'], 'unrelated circle scaled')
        assert len(curves(after, 'circle')) == 1, 'unrelated circle missing'
        equal(records(after, 'constraints'), records(before, 'constraints'), 'scale retains original constraints')
        assert not before['dimensions'] and len(after['dimensions']) == 1, 'first dimension count'
        assert after['dimensions'][0]['type'] == 'length', 'wrong first dimension kind'
        assert not after['dimensions'][0].get('driven'), 'first dimension must drive the scaled sketch'
        near(after['dimensions'][0]['measured'], 100, 'first dimension measured length')
    else:
        raise AssertionError('unknown scenario: ' + scenario)
    feature = next(f for f in saved['features'] if f['uid'] == 'F2')
    actual = records(after, 'entities')
    stored = records(feature, 'entities')
    assert stored.keys() == actual.keys(), 'GUI save lost sketch entities'
    for key, entity in stored.items():
        for name in ('type', 'at', 'start', 'end', 'center', 'control', 'radius', 'minor_radius', 'axis', 'construction'):
            if name in entity:
                equal(entity[name], actual[key].get(name), f'GUI save/{key}/{name}')
    for key in ('constraints', 'dimensions'):
        assert records(feature, key).keys() == records(after, key).keys(), 'GUI save lost ' + key
    original_body = next(f for f in baseline['features'] if f['uid'] == 'F1')
    equal(next(f for f in saved['features'] if f['uid'] == 'F1'), original_body, 'unrelated body definition')
    report = {'scenario': scenario, 'entities': len(actual), 'constraints': len(after['constraints']),
              'dimensions': len(after['dimensions']), 'undo_redo_exact': True, 'gui_save_verified': True}
(root / 'verification.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print(json.dumps(report))
