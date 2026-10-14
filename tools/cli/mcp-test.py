# SPDX-License-Identifier: MIT
"""Exercise the actual MCP CLI over stdio; run only in an isolated build.

Usage: python3 mcp-test.py /absolute/path/mitcad-cli /build/test/directory
"""

import json
import os
from pathlib import Path
import queue
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest


CLI = str(Path(sys.argv[1]).resolve())
WORK = Path(sys.argv[2]).resolve()
TIMEOUT = 20
PROTOCOL = "2025-11-25"
TOOLS = {"document_create", "document_open", "document_list", "document_close",
         "model_command", "model_query", "document_save", "model_export"}
BOX = {"cmd": "add_feature", "def": {
    "type": "box", "plane": {"type": "origin", "plane": "xy"},
    "corner": [0, 0], "length": 30, "width": 20, "height": 10,
    "operation": "new_body"}}


def make_private(path, directory):
    """Make a discovery path private as the app does (LocalEndpoint.cpp).

    On Unix the owner-only mode bits; on Windows the owner set to this user
    and a protected DACL granting only this user and SYSTEM, which mode bits
    cannot express (WindowsEndpointSecurity.cpp checks the ACL).
    """
    if os.name != "nt":
        path.chmod(0o700 if directory else 0o600)
        return
    user = subprocess.run(["whoami"], capture_output=True, text=True, check=True).stdout.strip()
    inherit = "(OI)(CI)" if directory else ""
    subprocess.run(["icacls", str(path), "/setowner", user], capture_output=True, check=True)
    subprocess.run(["icacls", str(path), "/inheritance:r", "/grant:r", f"{user}:{inherit}F",
                    "/grant:r", f"*S-1-5-18:{inherit}F"], capture_output=True, check=True)


class Session:
    """Reader thread gives pipe reads a timeout on Windows as well as Unix."""

    def __init__(self, workspace, read_only=False):
        self.stderr = tempfile.TemporaryFile(dir=workspace)
        args = [CLI, "mcp", "--workspace", str(workspace)]
        if read_only:
            args.append("--read-only")
        self.process = subprocess.Popen(args, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.stderr)
        self.messages = queue.Queue()
        self.next_id = 0
        self.reader = threading.Thread(target=self.read_stdout, daemon=True)
        self.reader.start()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=TIMEOUT)
        self.reader.join(timeout=TIMEOUT)
        self.process.stdin.close()
        self.process.stdout.close()
        self.stderr.close()

    def read_stdout(self):
        while True:
            line = self.process.stdout.readline()
            self.messages.put(line)
            if not line:
                return

    def send_raw(self, line):
        self.process.stdin.write(line + b"\n")
        self.process.stdin.flush()

    def send(self, message):
        self.send_raw(json.dumps(message, separators=(",", ":")).encode("utf-8"))

    def receive(self):
        try:
            line = self.messages.get(timeout=TIMEOUT)
        except queue.Empty as error:
            raise AssertionError("MCP response timed out") from error
        if not line:
            self.stderr.seek(0)
            raise AssertionError("MCP stdout closed: " + self.stderr.read().decode("utf-8"))
        try:
            message = json.loads(line)
        except (ValueError, UnicodeDecodeError) as error:
            raise AssertionError("Non-MCP stdout: " + repr(line)) from error
        assert isinstance(message, dict) and message.get("jsonrpc") == "2.0", message
        return message

    def request(self, method, params=None):
        self.next_id += 1
        request = {"jsonrpc": "2.0", "id": self.next_id, "method": method}
        if params is not None:
            request["params"] = params
        self.send(request)
        response = self.receive()
        assert response.get("id") == self.next_id, response
        return response

    def notify(self, method, params=None):
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        self.send(message)

    def initialize(self, protocol=PROTOCOL):
        response = self.request("initialize", {
            "protocolVersion": protocol, "capabilities": {},
            "clientInfo": {"name": "mitcad-integration-test", "version": "1"}})
        assert response["result"]["protocolVersion"] == PROTOCOL, response
        self.notify("notifications/initialized")
        return response["result"]

    def tool(self, name, arguments, error=False):
        response = self.request("tools/call", {"name": name, "arguments": arguments})
        assert "error" not in response, response
        result = response["result"]
        assert bool(result.get("isError", False)) == error, result
        content = result["structuredContent"]
        assert json.loads(result["content"][0]["text"]) == content, result
        if error:
            if content.get("applied") is True:
                assert any(item["status"] == "error" for item in content["timeline"]["features"]), content
            else:
                assert isinstance(content["error"], str) and content["error"], content
        return content

    def finish(self):
        self.process.stdin.close()
        self.process.wait(timeout=TIMEOUT)
        self.reader.join(timeout=TIMEOUT)
        self.stderr.seek(0)
        diagnostics = self.stderr.read().decode("utf-8")
        assert self.process.returncode == 0, diagnostics
        assert self.messages.get(timeout=TIMEOUT) == b"", "Unexpected output before EOF"
        assert self.messages.empty(), "Unexpected output after EOF"


class McpTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="session-", dir=WORK)
        self.addCleanup(self.temp.cleanup)
        self.workspace = Path(self.temp.name)

    def test_connect_waits_for_busy_live_discovery(self):
        discovery = self.workspace / "discovery"
        discovery.mkdir(mode=0o700)
        make_private(discovery, True)
        instance, token = "1" * 32, "a" * 64
        errors = []
        accepted = []
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            listener.settimeout(3)
            record = discovery / (instance + ".json")
            record.write_text(json.dumps({"protocol": 1, "host": "127.0.0.1",
                                         "port": listener.getsockname()[1], "pid": os.getpid(),
                                         "instance": instance, "token": token}), encoding="utf-8")
            make_private(record, False)

            def serve():
                try:
                    for index in range(2):
                        connection, _ = listener.accept()
                        with connection:
                            connection.settimeout(5)
                            with connection.makefile("rb") as source:
                                auth = json.loads(source.readline())
                                if auth != {"mitcadAuth": token}:
                                    raise AssertionError("Unexpected authentication frame")
                                if index == 0:
                                    # A live GUI can be busy longer than the listing probe's 500 ms.
                                    time.sleep(0.8)
                                connection.sendall(b'{"authenticated":true}\n')
                                accepted.append(index)
                                if index == 1:
                                    source.read()
                except Exception as error:
                    errors.append(error)

            server = threading.Thread(target=serve, daemon=True)
            server.start()
            env = dict(os.environ, MITCAD_MCP_DISCOVERY_DIR=str(discovery))
            result = subprocess.run([CLI, "mcp", "--connect"], env=env, input=b"",
                                    capture_output=True, timeout=TIMEOUT)
            server.join(timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(server.is_alive(), "Discovery server did not finish")
        self.assertFalse(errors, errors)
        self.assertEqual(accepted, [0, 1])
        self.assertEqual(result.stdout, b"")

    def test_cli_arguments(self):
        for args in ([], ["--workspace"], ["--workspace", str(self.workspace / "missing")],
                     ["--workspace", str(self.workspace), "--unknown"],
                     ["--workspace", str(self.workspace), "--workspace", str(self.workspace)]):
            result = subprocess.run([CLI, "mcp"] + args, capture_output=True, timeout=TIMEOUT)
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertEqual(result.stdout, b"")
            self.assertTrue(result.stderr)
        result = subprocess.run([CLI, "mcp", "--help"], capture_output=True, timeout=TIMEOUT)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(b"--workspace", result.stdout)

    def test_model_save_open_export_and_eof(self):
        with Session(self.workspace) as client:
            capabilities = client.initialize()["capabilities"]
            self.assertIn("tools", capabilities)
            self.assertIn("resources", capabilities)
            tools = client.request("tools/list")["result"]["tools"]
            self.assertEqual({tool["name"] for tool in tools}, TOOLS)
            resources = client.request("resources/list")["result"]["resources"]
            self.assertIn("mitcad://reference/commands", {item["uri"] for item in resources})
            reference = client.request("resources/read", {
                "uri": "mitcad://reference/commands"})["result"]["contents"]
            self.assertIn("add_feature", reference[0]["text"])
            self.assertEqual(client.request("resources/templates/list")["result"]["resourceTemplates"], [])
            first = client.tool("document_create", {})["document"]
            second = client.tool("document_create", {})["document"]
            self.assertNotEqual(first, second)
            listed = client.tool("document_list", {})["documents"]
            self.assertEqual({item["document"] for item in listed}, {first, second})
            client.tool("model_command", {"document": first, "command": BOX})
            bodies = client.tool("model_query", {
                "document": first, "query": {"query": "bodies", "properties": True}})["result"]
            self.assertEqual(len(bodies), 1)
            self.assertAlmostEqual(bodies[0]["volume"], 6000, places=5)
            empty = client.tool("model_query", {
                "document": second, "query": {"query": "bodies"}})["result"]
            self.assertEqual(empty, [])
            client.tool("document_close", {"document": first}, error=True)
            self.assertEqual(len(client.tool("document_list", {})["documents"]), 2)
            client.tool("document_save", {"document": first, "path": "box.mitcad"})
            self.assertTrue((self.workspace / "box.mitcad").is_file())
            client.tool("document_save", {"document": first, "path": "box.mitcad"}, error=True)
            client.tool("document_save", {"document": first, "path": "box.mitcad", "overwrite": True})
            client.tool("model_export", {"document": first, "path": "box.step", "format": "step"})
            self.assertGreater((self.workspace / "box.step").stat().st_size, 0)
            (self.workspace / "box.mtl").write_text("existing material\n", encoding="utf-8")
            client.tool("model_export", {"document": first, "path": "box.obj", "format": "obj"}, error=True)
            self.assertFalse((self.workspace / "box.obj").exists())
            self.assertEqual((self.workspace / "box.mtl").read_text(encoding="utf-8"), "existing material\n")
            self.assertTrue(client.tool("document_close", {"document": first})["closed"])
            client.tool("model_query", {"document": first, "query": {"query": "document"}}, error=True)
            reopened = client.tool("document_open", {"path": "box.mitcad"})["document"]
            self.assertNotEqual(reopened, first)
            bodies = client.tool("model_query", {
                "document": reopened, "query": {"query": "bodies", "properties": True}})["result"]
            self.assertAlmostEqual(bodies[0]["volume"], 6000, places=5)
            client.tool("document_close", {"document": reopened})
            client.tool("document_close", {"document": second, "discard": True})
            self.assertEqual(client.tool("document_list", {})["documents"], [])
            client.finish()

    def test_protocol_errors_and_notifications_do_not_mutate(self):
        with Session(self.workspace) as client:
            self.assertIn("error", client.request("tools/list"))
            client.initialize()
            document = client.tool("document_create", {})["document"]
            client.notify("tools/call", {"name": "model_command", "arguments": {
                "document": document, "command": BOX}})
            client.notify("tools/call", {"name": "document_create", "arguments": {}})
            client.notify("notifications/cancelled", {"requestId": 999, "reason": "test"})
            bodies = client.tool("model_query", {
                "document": document, "query": {"query": "bodies"}})["result"]
            self.assertEqual(bodies, [])
            self.assertEqual(len(client.tool("document_list", {})["documents"]), 1)
            client.send_raw(b"{bad json")
            self.assertEqual(client.receive()["error"]["code"], -32700)
            client.send({"jsonrpc": "2.0", "id": 10000, "method": 4})
            self.assertEqual(client.receive()["error"]["code"], -32600)
            self.assertEqual(client.request("unknown/method")["error"]["code"], -32601)
            self.assertIn("error", client.request("tools/call", {"name": "model_command", "arguments": {
                "document": document, "command": "not an object"}}))
            client.tool("model_command", {"document": document, "command": {"cmd": "unknown"}}, error=True)
            state = client.tool("model_query", {
                "document": document, "query": {"query": "document"}})["result"]
            self.assertEqual(state["features"], 0)
            client.finish()

    def test_workspace_paths_and_read_only(self):
        with Session(self.workspace) as client:
            client.initialize()
            document = client.tool("document_create", {})["document"]
            client.tool("model_command", {"document": document, "command": BOX})
            for path in ("../escape.mitcad", str(self.workspace.parent / "escape.mitcad")):
                client.tool("document_save", {"document": document, "path": path}, error=True)
            client.tool("document_open", {"path": "../escape.mitcad"}, error=True)
            client.tool("model_command", {"document": document, "command": {
                "cmd": "export", "path": "escaped.step"}}, error=True)
            client.tool("model_export", {"document": document, "path": "../escape.step", "format": "step"}, error=True)
            client.tool("document_save", {"document": document, "path": "box.mitcad"})
            client.finish()
        self.assertFalse((self.workspace.parent / "escape.mitcad").exists())
        with Session(self.workspace, read_only=True) as client:
            client.initialize()
            client.tool("document_create", {}, error=True)
            document = client.tool("document_open", {"path": "box.mitcad"})["document"]
            bodies = client.tool("model_query", {
                "document": document, "query": {"query": "bodies"}})["result"]
            self.assertEqual(len(bodies), 1)
            client.tool("model_command", {"document": document, "command": BOX}, error=True)
            client.tool("document_save", {"document": document, "path": "forbidden.mitcad"}, error=True)
            client.tool("model_export", {"document": document, "path": "forbidden.step", "format": "step"}, error=True)
            client.tool("document_close", {"document": document})
            client.finish()
        self.assertFalse((self.workspace / "forbidden.mitcad").exists())
        self.assertFalse((self.workspace / "forbidden.step").exists())

    def test_autoselect_profiles_are_available_in_read_only_documents(self):
        with Session(self.workspace) as client:
            client.initialize()
            document = client.tool("document_create", {})["document"]
            client.tool("model_command", {"document": document, "command": {
                "cmd": "sketch.create"}})
            client.tool("model_command", {"document": document, "command": {
                "cmd": "sketch.add_circle", "sketch": "F1",
                "center": [0, 0], "diameter": 10}})
            before = client.tool("model_query", {"document": document,
                "query": {"query": "autoselect_profiles"}})["result"]
            self.assertTrue(before["eligible"])
            self.assertEqual(before["sketch"], "F1")
            self.assertEqual(len(before["drawn"]), 1)
            self.assertEqual(before["all"], before["drawn"])
            client.tool("document_save", {"document": document, "path": "sketch.mitcad"})
            client.finish()
        with Session(self.workspace, read_only=True) as client:
            client.initialize()
            document = client.tool("document_open", {"path": "sketch.mitcad"})["document"]
            after = client.tool("model_query", {"document": document,
                "query": {"query": "autoselect_profiles"}})["result"]
            self.assertEqual(after, before)
            client.finish()

    def test_geometry_failure_is_applied_and_can_be_undone(self):
        with Session(self.workspace) as client:
            client.initialize()
            document = client.tool("document_create", {})["document"]
            client.tool("model_command", {"document": document, "command": BOX})
            # Positive sizes pass definition validation. The cut fails only
            # during evaluation because it does not touch its participant.
            cut = {"cmd": "add_feature", "def": {
                "type": "box", "plane": {"type": "origin", "plane": "xy"},
                "corner": [100, 100], "length": 5, "width": 5, "height": 5,
                "operation": "cut", "participants": ["F1.b0"]}}
            failed = client.tool("model_command", {"document": document, "command": cut}, error=True)
            self.assertTrue(failed["applied"])
            timeline = client.tool("model_query", {
                "document": document, "query": {"query": "timeline"}})["result"]
            self.assertEqual(len(timeline["features"]), 2)
            self.assertEqual(timeline["features"][1]["status"], "error")
            self.assertIn("cut into", timeline["features"][1]["error"])
            client.tool("model_export", {"document": document,
                        "path": "failed.step", "format": "step"}, error=True)
            self.assertFalse((self.workspace / "failed.step").exists())
            client.tool("model_command", {"document": document, "command": {"cmd": "undo"}})
            timeline = client.tool("model_query", {
                "document": document, "query": {"query": "timeline"}})["result"]
            self.assertEqual(len(timeline["features"]), 1)
            self.assertEqual(timeline["features"][0]["status"], "ok")
            client.tool("document_close", {"document": document, "discard": True})
            client.finish()

    def test_sheet_metal_and_flat_pattern_dxf(self):
        # Sheet-metal features, picks and the Flat Pattern DXF (mitcad#210).
        with Session(self.workspace) as client:
            client.initialize()
            document = client.tool("document_create", {})["document"]
            for command in (
                    {"cmd": "sketch.create"},
                    {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 40, "height": 20},
                    {"cmd": "add_feature", "def": {
                        "type": "sheet_face", "profiles": [
                            {"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
                        "rules": {"thickness": 1, "bend_radius": 1, "allowance": {"type": "k_factor", "k": 0.44},
                                  "reference_side": "front", "bend_relief": {"type": "round", "width": 1, "depth": 3},
                                  "corner_relief": {"type": "round", "width": 1, "depth": 3}}}},
                    {"cmd": "set_flat_pattern", "body": "F3.b0"}):
                client.tool("model_command", {"document": document, "command": command})
            bodies = client.tool("model_query", {
                "document": document, "query": {"query": "sheet_bodies"}})["result"]
            self.assertEqual(bodies[0]["flat_pattern"]["status"], "valid")
            picked = client.tool("model_query", {"document": document, "query": {
                "query": "sheet_reference", "body": "F3.b0", "face": "F3:sheet_front(0)"}})["result"]
            self.assertEqual(picked["domain"]["kind"], "panel")
            client.tool("model_export", {"document": document, "path": "flat.dxf", "format": "dxf",
                                         "flat_pattern": "F3.b0"})
            self.assertIn("CUT_OUTER", (self.workspace / "flat.dxf").read_text(encoding="ascii"))
            client.tool("model_export", {"document": document, "path": "both.dxf", "format": "dxf",
                                         "flat_pattern": "F3.b0", "sketch": "F1"}, error=True)
            client.tool("document_close", {"document": document, "discard": True})
            client.finish()

    def test_symlink_cannot_leave_workspace(self):
        with tempfile.TemporaryDirectory(prefix="outside-", dir=WORK) as outside:
            link = self.workspace / "outside"
            try:
                os.symlink(outside, link, target_is_directory=True)
            except (OSError, NotImplementedError) as error:
                self.skipTest("directory symlinks unavailable: " + str(error))
            with Session(self.workspace) as client:
                client.initialize()
                document = client.tool("document_create", {})["document"]
                client.tool("document_save", {"document": document,
                            "path": "outside/escape.mitcad"}, error=True)
                self.assertFalse((Path(outside) / "escape.mitcad").exists())
                outside_file = Path(outside) / "existing.mitcad"
                outside_file.write_text("untouched", encoding="utf-8")
                os.symlink(outside_file, self.workspace / "existing.mitcad")
                os.symlink(Path(outside) / "missing.mitcad", self.workspace / "dangling.mitcad")
                for path in ("existing.mitcad", "dangling.mitcad"):
                    client.tool("document_save", {"document": document, "path": path,
                                "overwrite": True}, error=True)
                    client.tool("document_open", {"path": path}, error=True)
                temporary = self.workspace / (".mitcad-mcp.%d.0.tmp" % client.process.pid)
                os.symlink(outside_file, temporary)
                client.tool("document_save", {"document": document, "path": "safe.mitcad"})
                self.assertTrue((self.workspace / "safe.mitcad").is_file())
                self.assertTrue(temporary.is_symlink())
                self.assertEqual(outside_file.read_text(encoding="utf-8"), "untouched")
                # Preflight must refuse the store path before parsing these
                # synthetic bytes as B-rep data; no third-party model needed.
                project = self.workspace / ".mitcad"
                project.mkdir()
                (project / "project.json").write_text("{}", encoding="utf-8")
                digest = "a" * 64
                (Path(outside) / "aa").mkdir()
                (Path(outside) / "aa" / (digest + ".brep.zlib")).write_bytes(b"synthetic")
                os.symlink(outside, project / "brep", target_is_directory=True)
                fixture = {"format": "mitcad", "version": 3, "features": [
                    {"type": "base", "bodies": [{"brep": {"format": "occt",
                     "compression": "zlib", "size": 9, "sha256": digest}}]}]}
                (self.workspace / "outside-store.mitcad").write_text(json.dumps(fixture), encoding="utf-8")
                error = client.tool("document_open", {"path": "outside-store.mitcad"}, error=True)
                self.assertIn("leaves the MCP workspace", error["error"])
                self.assertEqual(len(client.tool("document_list", {})["documents"]), 1)
                client.finish()

    def test_legacy_version_negotiation(self):
        with Session(self.workspace) as client:
            self.assertEqual(client.request("server/discover")["error"]["code"], -32601)
            client.initialize("2024-11-05")
            self.assertIn("result", client.request("ping"))
            client.finish()


if __name__ == "__main__":
    WORK.mkdir(parents=True, exist_ok=True)
    unittest.main(argv=[sys.argv[0]])
