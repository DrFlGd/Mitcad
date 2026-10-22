# SPDX-License-Identifier: MIT
"""Selection owners in both Qt layouts, using the actual GUI and MCP bridge.

Run in the approved platform VM (Linux under Xvfb):
  python3 ui-selection-owners-test.py <app> <cli> <output> [qt-bin]
The TestDriver reads the visible QLabel, checks its plain text and saves
screenshots. MCP selects named OCCT entities through the public UI API.
"""

import importlib.util
import os
from pathlib import Path
import struct
import sys
import tempfile


def support_module():
    spec = importlib.util.spec_from_file_location(
        "selection_mcp_support", Path(__file__).with_name("ui-macos-mcp-test.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MCP = support_module()


def run(app, cli, out, qt_bin):
    out.mkdir(parents=True, exist_ok=True)
    for chrome in ("docked", "floating"):
        with tempfile.TemporaryDirectory(prefix=chrome + "-", dir=out) as directory:
            work = Path(directory)
            env = dict(os.environ)
            env.pop("MITCAD_TEST_INPUT", None)
            for key, folder in (("MITCAD_SETTINGS_DIR", "settings"), ("MITCAD_AUTOSAVE_DIR", "autosave"),
                                ("MITCAD_PROJECTS_DIR", "projects"), ("MITCAD_THUMBNAIL_DIR", "thumbnails"),
                                ("MITCAD_CRASH_DIR", "crashes"), ("MITCAD_MCP_DISCOVERY_DIR", "discovery")):
                env[key] = str(work / folder)
            env.update(MITCAD_RESULT_STORE="off", MITCAD_NO_UPDATE_CHECK="1",
                       QT_FORCE_STDERR_LOGGING="1", MITCAD_SELECTION_WORK=str(work))
            if qt_bin:
                env["PATH"] = qt_bin + os.pathsep + env.get("PATH", "")
            # Each selected block must name its own owners before its measurements.
            nested = r"Component: Bench:1 › Screw M8:1\nBody: Screw Body"
            cases = [
                ("root-edge", r"(?s)\A(?!.*Component:).*Body: Root Plate\nLength:"),
                ("nested-edge", "(?s)" + nested + r"\nLength:"),
                ("nested-face", "(?s)" + nested + r"\nArea:"),
                ("nested-body", "(?s)" + nested + r"\nArea:.*\nVolume:"),
                ("mixed", "(?s)" + nested + r"\nLength:.*Body: Root Plate\nLength:"),
            ]
            script = "expect-ever 'View area'\n"
            for name, expected in cases:
                script += "expect-file '${MITCAD_SELECTION_WORK}/" + name + "' 60\n"
                script += "expect-selection-info '" + expected + "'\n"
                script += "screenshot '${MITCAD_SELECTION_WORK}/" + name + ".png'\n"
            script += "expect-file '${MITCAD_SELECTION_WORK}/finish' 60\nquit\n"
            step_count = len(script.splitlines())
            gui = MCP.Gui(app, env, out, chrome, script, "qt", ["--chrome=" + chrome])
            bridge = None
            try:
                discovery = Path(env["MITCAD_MCP_DISCOVERY_DIR"])
                MCP.wait_for(lambda: discovery.exists() and bool(list(discovery.glob("*.json"))),
                             "selection GUI endpoint", gui)
                bridge = MCP.Bridge(cli, env, out, chrome + "-bridge")
                bridge.initialize()
                document = bridge.tool("document_list", {})["documents"][0]["document"]

                def command(value):
                    bridge.tool("model_command", {"document": document, "command": value})

                def query(value):
                    return bridge.tool("model_query", {"document": document, "query": value})["result"]

                # A fresh GUI document has an active Part1 occurrence. The
                # root-body case must explicitly create geometry in C0.
                command({"cmd": "activate_component", "component": "C0"})
                command(MCP.BOX)
                root = query({"query": "bodies"})[0]["uid"]
                command({"cmd": "rename_body", "uid": root, "name": "Root Plate"})
                command({"cmd": "create_component", "name": "Bench"})
                command({"cmd": "create_component", "name": "Screw M8"})
                command(MCP.BOX)
                child = next(body["uid"] for body in query({"query": "bodies"}) if body["uid"] != root)
                command({"cmd": "rename_body", "uid": child, "name": "Screw Body"})
                command({"cmd": "activate_component", "component": "C0"})
                components = query({"query": "components"})
                bench = next(occurrence for occurrence in components["occurrences"]
                             if occurrence["name"] == "Bench:1")
                screw = next(occurrence for occurrence in bench["children"]
                             if occurrence["name"] == "Screw M8:1")
                occurrence = bench["uid"] + "/" + screw["uid"]
                MCP.check(bench["name"] == "Bench:1" and screw["name"] == "Screw M8:1",
                          "The fixture does not contain the expected nested occurrences")

                def entity(kind, body, placed=""):
                    item = {"kind": kind, "owner": body, "occurrence": placed}
                    if kind != "body":
                        entries = query({"query": {"edge": "edges", "face": "faces"}[kind],
                                         "body": body})
                        item["name"] = entries[0]["names"][0] if kind == "face" else entries[0]["name"]
                    return item

                root_edge = entity("edge", root)
                edge = entity("edge", child, occurrence)
                selections = [[root_edge], [edge], [entity("face", child, occurrence)],
                              [entity("body", child, occurrence)],
                              [edge, root_edge]]
                for index, ((name, _), selected) in enumerate(zip(cases, selections)):
                    bridge.tool("ui_select", {"document": document, "items": selected})
                    (work / name).touch()
                    screenshot = work / (name + ".png")
                    # QImage opens the file before encoding is complete. The
                    # following driver gate proves that saving has returned.
                    following = cases[index + 1][0] if index + 1 < len(cases) else "finish"
                    marker = ": expect-file '${MITCAD_SELECTION_WORK}/" + following + "'"
                    MCP.wait_for(lambda: marker in gui.log_text(),
                                 chrome + " " + name + " selection screenshot saved", gui)
                    image = screenshot.read_bytes()
                    MCP.check(image[:8] == b"\x89PNG\r\n\x1a\n", "Selection screenshot is not PNG")
                    width, height = struct.unpack(">II", image[16:24])
                    MCP.check(width >= 300 and height >= 200, "Selection screenshot is too small")
                    (out / (chrome + "-" + name + ".png")).write_bytes(image)
                bridge.finish()
                (work / "finish").touch()
                gui.process.wait(timeout=30)
                completed = "TestDriver: quit (%d of %d steps)" % (step_count, step_count)
                MCP.check(gui.process.returncode == 0 and completed in gui.log_text()
                          and "TestDriver: FAIL" not in gui.log_text(), gui.log_text())
                print("PASS:", chrome, "root, nested, multi-item selection owners")
            finally:
                if bridge is not None:
                    bridge.stop()
                gui.stop()


if __name__ == "__main__":
    if len(sys.argv) not in (4, 5):
        raise SystemExit("usage: ui-selection-owners-test.py <app> <cli> <output> [qt-bin]")
    run(str(Path(sys.argv[1]).resolve()), str(Path(sys.argv[2]).resolve()),
        Path(sys.argv[3]).resolve(), sys.argv[4] if len(sys.argv) == 5 else "")
