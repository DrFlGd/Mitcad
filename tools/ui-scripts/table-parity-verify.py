#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Check the actual GUI save after table header edits and a cell undo."""
import json
import math
import sys
from pathlib import Path

saved = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8'))
table = next(t for t in saved['tables'] if t['name'] == 'Sizes')
assert table['comment'] == 'Synthetic sizes', 'the table comment changed'
assert table['columns'] == [{'name': 'Key', 'unit': 'Text'}, {'name': 'Height', 'unit': 'cm'}], \
    'the renamed header and selected unit were not saved'
assert [row[0] for row in table['rows']] == ['M4', 'M5'], 'the text keys changed'
assert len(table['rows']) == 2, 'the table rows changed'
for row, expected in zip(table['rows'], (0.7, 2.0)):
    assert math.isclose(row[1], expected, rel_tol=0, abs_tol=1e-9), \
        'unit conversion or one-step cell undo was not saved'
parameters = {p['name']: p for p in saved['parameters']}
height = parameters['Height']
assert 'Sizes.Height' in height['expression'] and 'Sizes.Value' not in height['expression'], \
    'the dependent expression lost the renamed column'
assert height['unit'] == 'mm' and math.isclose(height['value'], 20.0, rel_tol=0, abs_tol=1e-9), \
    'the dependent height changed after undo'
for name, expected in (('d1', 60.0), ('d2', 40.0)):
    assert math.isclose(parameters[name]['value'], expected, rel_tol=0, abs_tol=1e-9), \
        'the original rectangle dimension changed'
print('Table parity GUI save: renamed Height/cm, converted rows, dependent reference and cell undo verified')
