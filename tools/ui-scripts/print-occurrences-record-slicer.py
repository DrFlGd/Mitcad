#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Record the real detached slicer launch without opening another GUI."""
import itertools
import json
import os
from pathlib import Path
import shutil
import sys

work = Path(os.environ["MITCAD_TEST_WORK"]).resolve()
receipts = work / "receipts"
receipts.mkdir(exist_ok=True)
arguments = sys.argv[1:]
assert len(arguments) == 1 and arguments[0].endswith(".3mf"), arguments
source = Path(arguments[0])
assert source.is_file(), source
for number in itertools.count(1):
    try:
        descriptor = os.open(receipts / f"{number}.started", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.close(descriptor)
        break
    except FileExistsError:
        continue
shutil.copyfile(source, receipts / f"{number}.3mf")
temporary = receipts / f"{number}.pending"
temporary.write_text(json.dumps({"arguments": arguments, "pid": os.getpid()}) + "\n")
temporary.replace(receipts / f"{number}.json")
