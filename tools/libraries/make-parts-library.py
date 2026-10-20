# SPDX-License-Identifier: MIT
"""Builds Mitcad's parts library (mitcad-parts), the one library of the parts
Mitcad offers: the metric fasteners (make-fastener-library.py, CC0-1.0) and
the BUILD tab's slot profiles, connectors, nuts, caps, panels and linear
parts (make-profile-library.py) in one folder with one manifest. The
library is MIT; the fasteners keep CC0-1.0 as their own licence.

    python3 make-parts-library.py --cli build/dev/tools/cli/mitcad-cli \\
        --source <folder of the supplier's files> --out <clone of the parts repository> \\
        [--groups tools/libraries/profiles] [--version 2.3.0] [--work <folder>] [--no-render] [--commit]

--out is a clone of the parts repository (the fastener library until
1.0.0), or a new folder. Its earlier versions stay: with --commit the
result is recorded on top of what the clone has and tagged v<version>.
The version is 2.0.0 for the first parts library, since the library's id
changed (mitcad-fasteners became mitcad-parts); designs that recorded the
fasteners at 1.0.0 keep reading them from the same repository. The
default version is the next one to publish: 2.3.0 adds the modelled shafts,
lead screws and threaded rods with their threads (mitcad#449).
"""

import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
LIBRARY_ID = "mitcad-parts"
LIBRARY_NAME = "Mitcad parts"


def fail(message):
    sys.exit(f"make-parts-library: {message}")


def run(command, what):
    """Runs a step, its output shown; a failure stops the build."""
    result = subprocess.run([str(c) for c in command], capture_output=True, text=True, encoding="utf-8",
                            errors="replace")
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    if result.returncode != 0:
        fail(f"{what} failed with exit status {result.returncode}")
    return result.stdout


def read(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return ""


def write(path, text):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def section(text, title):
    """A generator's SOURCES.md as a section of the library's: its title
    replaced, its headings one level down."""
    lines = text.strip().splitlines()
    if lines and lines[0].startswith("# "):
        lines = lines[1:]
    lines = ["#" + line if line.startswith("#") else line for line in lines]
    return f"## {title}\n" + "\n".join(lines).strip("\n") + "\n"


LICENSE_TEXT = """Mitcad parts: licences

The library as a whole, and every part without a licence of its own (the
slot profiles, connectors, nuts, caps, panels and linear parts of the
BUILD tab), is licensed under the MIT licence:

Copyright (c) 2026 Mitcad contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

The fasteners (the screws, nuts and washers to ISO standards: screws/,
nuts/iso*.mitcad, washers/, their tables and images) name CC0-1.0 as
their own licence in the manifest: they are dedicated to the public
domain under Creative Commons Zero v1.0 Universal.
CC0-1.0's text: https://creativecommons.org/publicdomain/zero/1.0/legalcode
"""

README_TEXT = f"""# {LIBRARY_NAME}

Mitcad's library of parts: ISO metric screws, nuts and washers, each standard
one design with a table of its sizes; and the BUILD tab's slot profiles,
connectors, nuts, caps, panels and linear parts, parametric (a profile is
cut to any length), each naming its supplier and part number.

In Mitcad: Libraries, add this repository's URL, Fetch; then Insert
Component from Library, or the BUILD tab's pickers. Designs record the
version they use.

This repository was the fastener library (mitcad-fasteners) up to 1.0.0;
from 2.0.0 it is the parts library. Designs made with the fastener library
keep reading their parts at the versions they recorded.

Licences: the library MIT, the fasteners CC0-1.0 (LICENSE). Where the parts
come from: SOURCES.md.
"""


def main():
    # The parts' names and source paths are UTF-8 (a supplier's file names);
    # a console in another code page must not stop the build.
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cli", required=True, help="mitcad-cli")
    parser.add_argument("--source", required=True, help="the folder of the supplier's files (never copied)")
    parser.add_argument("--out", required=True, help="a clone of the parts repository, or a new folder")
    parser.add_argument("--groups", default=os.path.join(HERE, "profiles"), help="the profile parts' group files")
    parser.add_argument("--version", default="2.3.0", help="the library's version")
    parser.add_argument("--work", help="folder for intermediate files (default: next to --out)")
    parser.add_argument("--no-render", action="store_true", help="passed to make-profile-library.py")
    parser.add_argument("--commit", action="store_true",
                        help="record the folder as a git version on top of the clone's and tag it v<version>")
    args = parser.parse_args()

    out = os.path.abspath(args.out)
    work = ["--work", os.path.abspath(args.work)] if args.work else []
    python = sys.executable
    # 1. The fasteners, with a manifest of their own.
    run([python, os.path.join(HERE, "make-fastener-library.py"), "--cli", args.cli, "--out", out,
         "--version", args.version] + work, "make-fastener-library.py")
    fasteners = read(os.path.join(out, "SOURCES.md"))
    # 2. The BUILD parts into the same manifest; this checks the folder.
    run([python, os.path.join(HERE, "make-profile-library.py"), "--cli", args.cli, "--source", args.source,
         "--out", out, "--groups", args.groups, "--version", args.version, "--merge"]
        + work + (["--no-render"] if args.no_render else []), "make-profile-library.py")
    profiles = read(os.path.join(out, "SOURCES.md"))
    # 3. One licence note, README and sources for both.
    write(os.path.join(out, "LICENSE"), LICENSE_TEXT)
    write(os.path.join(out, "README.md"), README_TEXT)
    write(os.path.join(out, "SOURCES.md"),
          "# Sources of the parts\n\n" + section(fasteners, "Fasteners") + "\n"
          + section(profiles, "BUILD parts"))
    with open(os.path.join(out, "mitcad-library.json"), encoding="utf-8") as f:
        manifest = json.load(f)
    if manifest.get("id") != LIBRARY_ID:
        fail(f"the manifest's id is {manifest.get('id')}, not {LIBRARY_ID}")
    run([args.cli, "library", "check", out], "the library check")
    if args.commit:
        identity = ["-c", "user.name=Mitcad developers", "-c", "user.email=devs@mitcad.org"]
        if not os.path.isdir(os.path.join(out, ".git")):
            run(["git", "init", "-q", "-b", "main", out], "git init")
        run(["git", "-C", out, "add", "-A"], "git add")
        status = run(["git", "-C", out, "status", "--porcelain"], "git status")
        if status.strip():
            run(["git", "-C", out] + identity + ["commit", "-q", "-m", f"{LIBRARY_NAME} {args.version}"],
                "git commit")
        run(["git", "-C", out] + identity + ["tag", "-a", "-f", f"v{args.version}", "-m",
                                              f"{LIBRARY_NAME} {args.version}"], "git tag")
    print(f"{LIBRARY_ID} {args.version}: {len(manifest.get('components') or [])} components in {out}")


if __name__ == "__main__":
    main()
