# SPDX-License-Identifier: MIT
"""The library generators' render step (mitcad#526): two fasteners'
previews rendered in the studio of tools/libraries/library_previews.py,
a socket head cap screw (ISO 4762, black oxide, make-fastener-library.py)
and a countersunk wood screw (zinc plated steel, make-profile-library.py
on the shipped timber-fasteners.json): each a PNG of 256 x 256 pixels
within the library's 512 KB, not blank, its part's colours in the range
of its finish and reading as metal (the zinc plated screw a light
silvery grey with near-white highlights, the black oxide one a dark grey
whose darkest parts are not black, with visible highlights, the one
clearly darker than the other; neither flat); the ISO screw's design
lists its cosmetic thread (`threads`), and its preview, which models the
thread, differs from a render of the plain shank; `library check` passes; a
second run keeps the pictures (nothing changed); and without a render
worker the generator draws the preview and says so. Skipped (after the
drawn case) where no render worker is given or found.

    python3 library-previews-test.py --cli <mitcad-cli> [--worker <mitcad-render>]
        --libraries <tools/libraries> --work <folder>
"""

import argparse
import os
import shutil
import subprocess
import sys


def fail(message):
    sys.exit(f"library previews: FAILED: {message}")


def run(command, what):
    result = subprocess.run([str(c) for c in command], capture_output=True, text=True, encoding="utf-8",
                            errors="replace")
    output = result.stdout + result.stderr
    if result.returncode != 0:
        fail(f"{what} failed with exit status {result.returncode}:\n{output}")
    print(output)
    return output


def expect(text, output, what):
    if text not in output:
        fail(f"{what}: no '{text}' in:\n{output}")


def check_picture(previews, path, what):
    """The picture's size and coverage; its data and what it shows
    (library_previews.content)."""
    with open(path, "rb") as f:
        data = f.read()
    if len(data) > previews.MAX_BYTES:
        fail(f"{what}: {len(data)} bytes, more than the library's {previews.MAX_BYTES}")
    shown = previews.content(data)
    width, height, share = shown["width"], shown["height"], shown["share"]
    print(f"{what}: {width} x {height}, {len(data)} bytes, the part on {share:.1%} of the pixels, "
          f"mean colour {[round(c) for c in shown['mean']]}, inside luma {shown['dark']:.0f} (5 %) to "
          f"{shown['bright']:.0f} (98 %)")
    if (width, height) != (previews.PREVIEW_SIZE, previews.PREVIEW_SIZE):
        fail(f"{what}: {width} x {height} pixels, not {previews.PREVIEW_SIZE} x {previews.PREVIEW_SIZE}")
    if not 0.04 <= share <= 0.9:
        fail(f"{what}: the part covers {share:.1%} of the picture (blank, or nothing but the part)")
    if len(shown["luma"]) < 200:
        fail(f"{what}: the part has only {len(shown['luma'])} pixels inside its edges")
    return data, shown


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", required=True)
    parser.add_argument("--worker", default="")
    parser.add_argument("--libraries", required=True, help="tools/libraries")
    parser.add_argument("--work", required=True)
    args = parser.parse_args()
    sys.path.insert(0, os.path.abspath(args.libraries))
    import library_previews as previews

    work = os.path.abspath(args.work)
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    python = sys.executable
    fasteners = os.path.join(args.libraries, "make-fastener-library.py")
    profiles = os.path.join(args.libraries, "make-profile-library.py")
    socket = ["--standards", "iso4762", "--sizes", "M5"]

    # Without a render worker: the preview drawn from the dimensions.
    out = run([python, fasteners, "--cli", args.cli, "--out", os.path.join(work, "drawn"), "--work", work,
               "--render-worker", os.path.join(work, "no-such-worker")] + socket, "the generator without a worker")
    expect("warning: no render worker", out, "the generator without a worker")
    expect("preview iso4762: drawn (black_oxide)", out, "the generator without a worker")
    drawn = previews.content(previews.read_old(os.path.join(work, "drawn", "previews", "iso4762.png")))
    if (drawn["width"], drawn["height"]) != (256, 256) or drawn["share"] < 0.04:
        fail(f"the drawn preview: {drawn['width']} x {drawn['height']}, the part on {drawn['share']:.1%}")

    worker = os.path.abspath(args.worker) if args.worker else ""
    available, reason = previews.renderer_available(args.cli, worker or None)
    if not worker or not os.path.isfile(worker) or not available:
        print(f"library previews: skipped (no render worker: {reason or worker or 'none given'})")
        return

    # The socket head cap screw, black oxide.
    library = os.path.join(work, "fasteners")
    command = [python, fasteners, "--cli", args.cli, "--out", library, "--work", work,
               "--render-worker", worker, "--strict"] + socket
    out = run(command, "the fastener generator")
    expect("preview iso4762: rendered (black_oxide)", out, "the fastener generator")
    picture = os.path.join(library, "previews", "iso4762.png")
    data, socket = check_picture(previews, picture, "ISO 4762 M5x16")
    if not (previews.png_text(data) or "").startswith("studio"):
        fail("the rendered preview carries no key of its studio")
    out = run([args.cli, "library", "check", library], "the fastener library's check")
    # The screw is threaded: a cosmetic ISO metric thread the threads query
    # lists (the design's default size), and its preview shows the thread
    # modelled: it differs from a render of the plain shank.
    design = os.path.join(library, "screws", "iso4762.mitcad")
    check = os.path.join(work, "threads.json")
    with open(check, "w", encoding="utf-8") as f:
        f.write('[{"expect": {"query": {"query": "threads"}, '
                '"result": [{"designation": "M5x0.8", "modeled": false}]}}]\n')
    run([args.cli, "run", check, "--open", design], "the screw's threads")
    plain_scratch = os.path.join(work, "plain")
    os.makedirs(plain_scratch)
    plain, _ = previews.render_preview(args.cli, design, plain_scratch, worker, up=[0, 0, 1], force=True)
    _, _, threaded_rows = previews.read_png(data)
    _, _, plain_rows = previews.read_png(plain)
    changed = sum(1 for a, b in zip(threaded_rows, plain_rows) for x in range(0, len(a), 4)
                  if max(abs(a[x + c] - b[x + c]) for c in range(3)) > 30)
    print(f"ISO 4762 M5x16: {changed} pixels differ from the render without the thread")
    if changed < 0.01 * socket["share"] * previews.PREVIEW_SIZE ** 2:
        fail(f"the preview does not show the thread: {changed} pixels differ from the plain shank's")
    # Nothing changed: the picture is kept as it is.
    out = run(command, "the fastener generator again")
    expect("preview iso4762: kept", out, "the fastener generator again")
    if previews.read_old(picture) != data:
        fail("the kept preview changed")

    # The countersunk wood screw of the shipped group file, zinc plated.
    groups = os.path.join(work, "groups")
    os.makedirs(groups)
    shutil.copyfile(os.path.join(args.libraries, "profiles", "timber-fasteners.json"),
                    os.path.join(groups, "timber-fasteners.json"))
    source = os.path.join(work, "no-source")
    os.makedirs(source)
    wood = os.path.join(work, "wood")
    out = run([python, profiles, "--cli", args.cli, "--source", source, "--out", wood, "--groups", groups,
               "--only", "wood-screw", "--work", work, "--render-worker", worker, "--strict"],
              "the profile generator")
    expect("preview wood-screw: rendered (", out, "the profile generator")
    _, zinc = check_picture(previews, os.path.join(wood, "previews", "wood-screw.png"), "wood screw 5x50")

    # The finishes: both nearly neutral greys (no channel far from the
    # others) that read as metal: highlights well above their darkest
    # parts. Zinc plating a light silvery grey with near-white highlights;
    # black oxide a dark grey, its darkest parts not black, with visible
    # highlights; the one clearly darker than the other.
    luma = previews.luminance
    for shown, what in ((socket, "the black oxide screw"), (zinc, "the zinc plated screw")):
        colour = [round(c) for c in shown["mean"]]
        if max(colour) - min(colour) > 40:
            fail(f"{what}'s mean colour {colour} is no grey")
        if shown["bright"] - shown["dark"] < 60:
            fail(f"{what} is flat: its inside's luma from {shown['dark']:.0f} to {shown['bright']:.0f}")
    if not 130 <= luma(zinc["mean"]) <= 235 or zinc["bright"] < 200:
        fail(f"the zinc plated screw is no light silvery grey: mean luma {luma(zinc['mean']):.0f}, "
             f"highlights {zinc['bright']:.0f}")
    if not 35 <= luma(socket["mean"]) <= 125:
        fail(f"the black oxide screw is no dark grey: mean luma {luma(socket['mean']):.0f}")
    # The modelled thread's grooves are in deep shadow, so the darkest 5 %
    # may well be black; the mean and the highlights say it is dark grey.
    if socket["bright"] < 120:
        fail(f"the black oxide screw has no highlights: its brightest luma {socket['bright']:.0f}")
    if luma(socket["mean"]) + 40 > luma(zinc["mean"]):
        fail(f"the black oxide screw (mean luma {luma(socket['mean']):.0f}) is not darker than the zinc plated "
             f"one ({luma(zinc['mean']):.0f})")
    print("library previews: passed")


if __name__ == "__main__":
    main()
