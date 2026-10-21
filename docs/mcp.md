<!-- SPDX-License-Identifier: MIT -->
# Mitcad Model Context Protocol server

`mitcad-cli mcp` exposes Mitcad's native parametric model to an MCP client.
Use `--connect` to edit the document in an already-open desktop application,
or `--workspace DIR` to run without a UI. The headless process owns up to 64 open
documents, each identified by an opaque handle such as `D1`. Handles last
until the document closes or the process exits. The server uses the same
JSON commands, queries and OCCT geometry kernel as the application.

## Start and configure a client

Create a directory for the files the client may access, then start the
native executable with an absolute workspace path:

```bash
/absolute/path/to/mitcad-cli mcp --workspace /absolute/path/to/designs
```

On Windows use `mitcad-cli.exe`. `--workspace` is required and must name
an existing directory. Add `--read-only` to inspect existing designs;
this permits opening, listing, querying and closing documents, and
refuses creation, model commands, saving and exporting.

The executable is the `mitcad-cli` build target, usually under
`build/dev/tools/cli/`; use its actual absolute path for the chosen
platform and build configuration. It needs the same native libraries as
the other CLI commands. See [development.md](development.md) for builds
in the approved isolated environments.

For clients that accept an `mcpServers` configuration, the setup is:

```json
{
  "mcpServers": {
    "mitcad": {
      "command": "/absolute/path/to/mitcad-cli",
      "args": ["mcp", "--workspace", "/absolute/path/to/designs"]
    }
  }
}
```

On Windows, JSON accepts forward slashes, for example
`"command": "C:/Mitcad/mitcad-cli.exe"` and workspace `"C:/Designs"`.
Use the equivalent command-and-arguments fields if the client uses a
different configuration layout. The client launches the process and
owns its stdin and stdout pipes; no HTTP listener, desktop session or
plugin package is required. stdout carries only MCP messages and stderr
carries diagnostics. Closing stdin ends the headless session; unsaved documents
then leave memory without being written.

## Connect to an open desktop document

The desktop application starts a local MCP listener automatically. An agent
can discover it after the app is already open; no restart with special
arguments is needed. Start Mitcad with `--no-mcp` to disable this listener.

On macOS the installed bundle includes the CLI:

```bash
/Applications/Mitcad.app/Contents/MacOS/mitcad-cli mcp --list-instances
/Applications/Mitcad.app/Contents/MacOS/mitcad-cli mcp --connect
```

`--list-instances` prints `{"instances":[...]}` with each live instance's
`instance`, `pid`, `host` and `port`. It never prints authentication tokens.
If several instances are open, `--connect` refuses to choose; pass
`--connect --instance ID` using an ID from the list.

An instance is live only when its record's token authenticates on its
port; a port that another program reuses is not enough. A record whose
`pid` is no longer a running process (an app that crashed or was killed)
is skipped without connecting to its port, and the bridge and the app
remove such records from the user's discovery directory. A process the
user may not query counts as running. The remaining records are probed in
parallel, so instances that do not answer cost one timeout in all (500 ms
for `--list-instances`, 5 s for `--connect`). `--connect --instance ID`
connects only to that record and uses the connection that authenticates
it; `--connect` alone probes every record to find the single live one.

Configure an MCP client to launch the stdio bridge:

```json
{
  "mcpServers": {
    "mitcad-desktop": {
      "command": "/Applications/Mitcad.app/Contents/MacOS/mitcad-cli",
      "args": ["mcp", "--connect"]
    }
  }
}
```

The bridge authenticates the local socket before forwarding MCP messages.
The listener binds only to `127.0.0.1` on an automatically selected port.
Its discovery record and token are private to the current user, under
`GenericCacheLocation/Mitcad/mcp` (`~/Library/Caches/Mitcad/mcp` on macOS,
`%LOCALAPPDATA%\cache\Mitcad\mcp` on Windows, `~/.cache/Mitcad/mcp` on
Linux).
`MITCAD_MCP_DISCOVERY_DIR` overrides that directory for isolated tests or a
separate client environment; the app and bridge must use the same absolute
directory. Public or symlinked discovery records are refused. This is a
local stdio bridge, not an HTTP endpoint for remote clients.

### Windows

The installer puts the CLI beside the application: in
`C:\Program Files\Mitcad\bin` when Mitcad is installed for all users, in
`%LOCALAPPDATA%\Programs\Mitcad\bin` when just for the user. With Mitcad
open:

```powershell
& "C:\Program Files\Mitcad\bin\mitcad-cli.exe" mcp --list-instances
& "C:\Program Files\Mitcad\bin\mitcad-cli.exe" mcp --connect
```

The discovery directory is `%LOCALAPPDATA%\cache\Mitcad\mcp`, owned by the
user and with an access control list of theirs only;
`MITCAD_MCP_DISCOVERY_DIR` overrides it as on the other platforms. An MCP
client's configuration, in JSON with forward slashes or doubled
backslashes:

```json
{
  "mcpServers": {
    "mitcad-desktop": {
      "command": "C:/Program Files/Mitcad/bin/mitcad-cli.exe",
      "args": ["mcp", "--connect"]
    }
  }
}
```

### Live tools

The live catalog has `document_list`, `model_query`, `model_command`,
`document_save`, `document_open` and the application's own tools
`ui_state`, `ui_log`, `ui_screenshot`, `ui_select`, `ui_command` and
`ui_camera` (mitcad#265, mitcad#402, mitcad#405). The rule of
thumb: what the user can see and do in the window, an agent can read and do
through them, with the same undo and the same refusals. `document_list`
returns the GUI document's opaque handle, path, name, state, `busy` and
`editable` (model commands run now), and the folders agents may use.
Commands modify that actual document
through its normal computation and undo workflow, updating its viewport,
browser and history. New or Open in the GUI (or `document_open`)
invalidates the previous document handle; list again before editing the
replacement. The tools that act on the document (`model_query`,
`model_command`, `document_save`, `ui_select`, `ui_command`) take the
handle; `ui_state`, `ui_log`, `ui_screenshot` and `ui_camera` read or set
the window and take none, nor does `document_open`, which replaces the
document.

**Folders for agents** (mitcad#402), live mode's counterpart of headless
mode's `--workspace`: `document_save` with a `path` (Save As) and
`document_open` work only on `.mitcad` files in folders the user allows,
none by default. The user lists them in Preferences › Agents (the setting
`mcp/folders`), or allows one for a single run by starting the application
with `--mcp-folder DIR` (repeatable). A path is allowed when its folder, with
symbolic links resolved, is such a folder or below it; `..` is refused, a
relative path is relative to the first folder, an existing file is replaced
only with `overwrite: true` and never through a link. Without a `path`,
`document_save` writes the document's own file as before, in any folder.
So an agent working in the user's untitled window can make the work
crash-safe: `document_save` with a path once, then plain saves.

What blocks what:

- **Queries** (`model_query`) are read-only and work at any time: with a
  sketch or a command panel open, during drags and dialogs. They are refused
  only while the model computes (or the view draws, or an import runs):
  that result is an error with `busy: true`, `retry_after_ms` and the
  reason in `error`; retry after that many milliseconds. `ui_state`,
  `ui_log` and `ui_screenshot` are never refused for a sketch or command.
- **Model commands** (`model_command`) are refused while busy (as queries
  are), while a command panel is open or Create Sketch waits for its plane,
  during drags, animations, imports and dialogs, and when the window is
  read-only (with the reason). While a **sketch is being edited**, its own
  commands go through the sketch session as if the user did them: a
  `sketch.*` command whose `sketch` is the open sketch, `edit_feature` of
  it, and `undo` and `redo` (which undo the sketch's steps; undoing past
  its first step leaves it as the user's Undo does). The view, the Sketch
  Palette and the sketch's undo follow, and the result is the command's
  usual one, with `sketch`. Commands on other features are refused with the
  reason until the sketch is finished (`ui_command` with `sketch.finish`).
- **The ui tools** work in every mode: `ui_state` and `ui_log` at any time,
  `ui_screenshot`, `ui_select` and `ui_command` but while busy (as above);
  with a command panel open, `ui_select`'s entities go to the
  panel's active input as clicks in the view would.

Which commands and queries MCP allows follows their class in the model's
API, not a list of names: every command, query and feature type carries
one ([core/model/src/api/classes.rs](../core/model/src/api/classes.rs)),
and a unit test fails for one without a class. MCP allows every **model**
command (the document's data: features, sketches, parameters, attributes,
components, joints, views, analyses, undo and redo), every feature type and
every model query, and refuses **file IO** (imports, exports, other designs
and links, images and drawings by path, the saved state), **remote** (the
library resolver's repositories) and **internal** ones (a command panel's
checkpoints, the result caches), and a `cache` query that reads the store's
files (`disk`). A new model command therefore works through MCP without
another edit.

Three commands are classified per call, not by name:
`create_appearance`, `edit_appearance` and `set_render_settings` run through
MCP when the call reads no file (parameters, `based_on`, a null texture, a
texture whose images are embedded with `data` or kept with
`"embedded": true`, an environment that is not an image path), and are
refused, naming the path, when a texture image or `environment.image` would
be read from a file. A custom or emissive appearance can thus be made and
assigned in a live session without reopening the design.

Each change an agent makes is labelled in the undo text and in the version
history as `… (agent)` (`Add Line (agent)`, `Edit Sketch2 (agent)` when an
agent changed the sketch being edited), and undoes with Ctrl+Z like the
user's own. While an MCP client is connected (has initialized a session),
the project indicator (in the status bar of the Docked layout, the title
row of the Floating one, the toolbar of the native macOS window) says
"Agent connected", and its menu's Disconnect Agent ends the connection.

Closing the bridge's stdin ends that connection after forwarding its
requested replies; the GUI and its unsaved work remain open. A new bridge
can reconnect to the same document. `--workspace` and `--read-only` belong
to headless mode and cannot be combined with `--connect` or
`--list-instances`.

| Tool | Arguments | Result |
|---|---|---|
| `document_list` | `{}` | `documents`: the window's one, with `document`, `path`, `name`, `state`, `busy`, `editable`; `folders`: the folders agents may save to and open from (empty: none) |
| `model_query` | `document`, `query` | `document`, native JSON in `result` |
| `model_command` | `document`, `command` | `document`, `applied`, native `result`, `failed_features`, `state`, `timeline`, `bodies`; in a sketch also `sketch` ([failed features](#transport-errors-and-limits)) |
| `document_save` | `document`; `path`, `overwrite` (default false) | `document`, `path`, `saved`, `status` (the status bar's text, with the version recorded), `state`. Without `path`, saves to the document's own path as File > Save does, with the project's version and commit message (`Save …: …`) in a project with versions. With `path`, saves as Save As does to that `.mitcad` file in a folder for agents (above); the document is that file afterwards, and an existing file needs `overwrite: true`. Refused for a document never saved and no `path`, a path outside the folders, a read-only document, while a command panel is open, and when saving would ask the user something (the question's dialog is closed and named) |
| `document_open` | `path`; `discard` (default false) | `document` (the new handle), `path`, `name`, `state`, `busy`, `editable`, `opened`, `replaced` (the old handle). Opens a `.mitcad` design from a folder for agents in the window, as File > Open does, replacing the open document. Refused outside the folders, while the open document has unsaved changes (unless `discard: true`, which loses them and their recovery data), while a sketch or command panel is open, and when opening would ask the user something |
| `ui_state` | `{}` | `document`, `name`, `path`, `modified`, `read_only` (and `read_only_reason`), `mode` (`idle`, `sketch`, `command`, `pick_plane`), `busy`, `editable`, `active_component`, `undo`, `redo`, `undo_depth`, `selection` (entities as `ui_select` takes them, with `geometry`, `reference` and `text`), `hint` and `hint_is_error` (the status line; while Create Sketch waits for a plane, its banner's text, with the status line in `status_line` and `status_line_is_error` when it says something else, mitcad#474), `agents`, `sketch` (null, or `uid`, `name`, `dof`, `fully_constrained`, `existing`, `tool` and `palette`: `status`, `warnings` as `{"text", "entities"}` (the `sketch` query's `warning_entities`, mitcad#264), `clean_up_offered`), `command` (null, or the panel's `id`, `name`, `editing`, `in_sketch`, `inputs` by key with `label`, `type` and their values, `ready` or `message`, `preview_ok`, `preview_failed`, `preview_warnings`, `busy` and `panel`: `message`, `result`, `ok_enabled`, `notes` by key), `app` (`version`, `build`, `qt`, `platform`, `ui`, `chrome`) |
| `ui_log` | `since` (a sequence number; lines after it), `limit` (default 200, at most 2000) | `lines`: `seq`, `level`, `text`, `time` (ms since the epoch), the newest last; `next` (the next line's number), `dropped` (lines after `since` no longer kept). The application's own log lines (the same as on stderr: what commands did, `Selected: …`, `Preview …`, refusals, `MCP refused: …`), the newest 5000 kept |
| `ui_screenshot` | `window` (default false: the 3D view), `max_size` (default 1280, 64 to 4096) | `format` `png`, `of`, `width`, `height`, `full_width`, `full_height`, `bytes`, `png_base64`. The image the user sees, the sketch overlay included; the whole window with `window` (in the native macOS window as the window server composes it). Scaled to fit `max_size`, and further until the PNG is under 5 MiB, well within the 8 MiB frame |
| `ui_select` | `document`, `items` (entities: `kind` of `face`, `edge`, `vertex`, `body`, `profile`, `sketch_curve`, `sketch_point`, `sketch_constraint`, `sketch_dimension`, `plane`, `axis`, `point`, `component`, `sketch`, `feature`; `owner`; `name`; `occurrence`), `reveal` (`none`, `pan` default, `fit`) | `document`, `selected`, `selection`, `view` (`none`, `pan`, `zoom`), `camera` (as `ui_camera`'s, unless `reveal` is `none`). Replaces the selection as clicks would, so the user can press Delete or run a command on it; with `pan` the view moves to them when none is in sight, keeping the zoom unless they do not fit; `fit` zooms to them. The view is where it goes when the reply comes (its animation ended), so the next `ui_screenshot` shows it. Entities the model lacks are refused. With a command panel open the entities go to its active input instead, each as a click (one already there goes out again): `picked`, `into_command`, `command` (the panel's state) |
| `ui_command` | `document`; `id` (a command of the registry, [app/COMMANDS.md](../app/COMMANDS.md)) or `edit` (a feature's uid); `inputs` (by the panel's input keys: a number or text for values and texts, a choice's value, true or false for checks, an array of entities for selections); `finish` (`ok` default, `cancel`, `keep_open`) | `document`, `command`, `panel` (as `ui_state`'s `command`, after the preview), `committed`, `open`, `mode`, `sketch`, `undo`, `previews` (the `Preview …` log lines: validation, the preview's outcome and warnings), `log` (every line the command logged), `hint`; for a `view.*` command `camera` (as `ui_camera`'s, the view's animation ended). A command with a panel opens, takes the inputs as typing, choosing and picking would, previews, then OK, Cancel or stays open; OK refused leaves it open and returns an error with the panel's state; the OK of a command that repeats (the BUILD tab's Place, Connect and Rotate, mitcad#375) does it once and closes the panel, keeping it. An action runs (Finish Sketch, Undo, Fit). `edit` opens a feature as a double-click in the timeline does (a sketch in sketch mode). Without `id` and `edit`, `finish` ends the open panel, or the sketch being edited: `ok` finishes it, `cancel` takes back every change made since it was opened and leaves it. Commands that open a file dialog or another window (`file.*`, `insert.*`, `tools.*`, `help.*`, 3D Print, DXF import and export, ...) are refused (`document_save` with a path and `document_open` do Save As and Open); any other modal dialog a command opens is closed at once and the call refused with the dialog's name |
| `ui_camera` | `eye`, `target`, `up` (each `[x, y, z]`, mm), `height` (mm of the model shown at the target, top to bottom), `fov` (the perspective's vertical field of view, 1 to 170 degrees), `projection` (`orthographic`, `perspective`); `box` (`{"min": [x, y, z], "max": [x, y, z]}`) or `frame` (`none` default, `shown`, `selection`) | `camera`: `eye`, `target`, `up`, `direction` (unit, eye to target), `distance`, `height`, `width` (mm at the target), `projection`, `fov`, `view_size` (the view's pixels); `framed` (`none`, `box`, `shown`, `selection`); `changed`. Without arguments it reads the camera. The values given replace the camera's at once (no animation); a perspective camera's eye keeps its place when no `height` is given (the height follows from its distance), and with a `height` it moves along its line to show that much. Then `box` frames that box, `frame: "shown"` the shown bodies (only the isolated ones when some are isolated: not the origin, datums or sketches), `frame: "selection"` the selected entities: along the view direction, with Fit's margin, in the part of the view no panel covers. The next `ui_screenshot` shows exactly that view |

The UI-only flows this covers: Clean Up Stale Projections
(`{"id": "sketch.clean_up_stale_projections"}` in an open sketch), Project
(`sketch.project` with its `source` input), Finish Sketch (`sketch.finish`)
or cancelling the sketch (`{"finish": "cancel"}`), editing a sketch
(`{"edit": "F4"}`), and any feature panel.

For inspection screenshots (mitcad#405), aim the camera with `ui_camera`
and then take `ui_screenshot`: `{"box": {"min": [...], "max": [...]}}`
frames a region, `{"eye": ..., "target": ..., "up": ..., "height": ...}`
sets an exact view, `{"frame": "selection"}` frames what `ui_select`
selected. The reply's `camera` says where the view is. Fit (`view.fit`)
frames the selection when something is selected outside commands and
sketches, the isolated bodies when the design isolates some, and otherwise
everything shown; Home (`view.home`) takes a `Home` named view's eye,
target, up and height.

## Tools and results

`tools/list` supplies the full input schemas. The table lists the headless
catalog; live connections have their own (above). All arguments are JSON
objects; fields below without a default are required.

| Tool | Arguments | Result |
|---|---|---|
| `document_create` | `{}` | `document`, null `path`, `state`, `timeline`, `bodies`, `applied`, empty `failed_features` |
| `document_open` | `path` | New `document`, canonical `path`, recomputed `state`, `timeline`, `bodies`, `applied`, `failed_features`. A `.solidcraft` design (mitcad#450) is imported with its history into a new document without a `path` (save it with `document_save` and a path) |
| `document_list` | `{}` | `documents`: objects with `document`, `path`, `state` |
| `document_close` | `document`, `discard` (default false) | `document`, `closed`; refuses unsaved changes unless `discard` is true |
| `model_command` | `document`, `command` (native JSON object) | `document`, `applied`, native `result`, `failed_features`, `state`, `timeline`, `bodies` ([failed features](#transport-errors-and-limits)) |
| `model_query` | `document`, `query` (native JSON object) | `document`, native JSON in `result` |
| `document_save` | `document`, `path`, `overwrite` (default false) | `document`, canonical `path`, saved `state` |
| `model_export` | `document`, `path`, `format`, optional `bodies` (array of names or UIDs), optional `sketch`, or for DXF `flat_pattern` (a sheet-metal body: its Flat Pattern, mitcad#210); `format` `pdf` or `svg`, or `dxf` with `drawing`, writes a drawing (mitcad#471: views, sizes, hole diameters, a hole table and a title block), `drawing` holding `export_drawing`'s options (commands.md "Drawings"; the title the document's file name unless given) | `document`, canonical `path`, native export `result` |

Each tool result includes both `structuredContent` and a text content
block containing the same JSON. For example, `model_query` with
`{"document":"D1","query":{"query":"bodies","properties":true}}`
returns an object whose `result` is the native bodies array. The
`timeline` snapshot is the native timeline object, including its
`features` array; `state` is the native `document` query.

Export formats are `step`, `iges`, `brep`, `stl`, `obj`, `3mf`, `usdz` and `dxf`.
DXF requires a `sketch` name or UID and excludes `bodies`; other formats
accept `bodies` and exclude `sketch`. Export requires a new output path.
OBJ may also write the adjacent `.mtl` file, which must also be a new
path inside the workspace. STEP, IGES and BRep preserve exact geometry;
STL, OBJ, 3MF and USDZ tessellate it (USDZ with the appearances; texture
paths relative to the document's folder when it was saved or opened from a
file). Document save always writes a single
self-contained `.mitcad` file, even in a project with a B-rep store.

The command reference is available through `resources/list` and
`resources/read` at `mitcad://reference/commands`, as well as in
[the native API reference](../core/model/src/api/commands.md). Use that
reference for feature definitions, sketch entities, parameters and
topological names. The server permits the commands, feature types and
queries of the model class, those that operate on document data, and
refuses the file IO, remote and internal ones by their class
([core/model/src/api/classes.rs](../core/model/src/api/classes.rs),
"Live tools" above): file imports, external component refresh, raw
save/export commands, disk cache access and remote operations. Files are
accessed through the dedicated document and export tools.

## Create, edit, save and export a part

After initialization, call `document_create` with `{}` and retain the
returned handle. The following `tools/call` parameter objects assume
that handle is `D1`. They create a 30 × 20 × 10 mm box:

```json
{
  "name": "model_command",
  "arguments": {
    "document": "D1",
    "command": {
      "cmd": "add_feature",
      "def": {
        "type": "box",
        "plane": {"type": "origin", "plane": "xy"},
        "corner": [0, 0],
        "length": 30,
        "width": 20,
        "height": 10,
        "operation": "new_body"
      }
    }
  }
}
```

Inspect the geometry with:

```json
{"name":"model_query","arguments":{"document":"D1","query":{"query":"bodies","properties":true}}}
```

The body volume is 6000 mm³. `model_query` with
`{"query":"timeline"}` identifies the feature (`F1` in this new
document). Edit its definition to make the height 15 mm, recompute
through the model command, and inspect the resulting volume, 9000 mm³:

```json
{
  "name": "model_command",
  "arguments": {
    "document": "D1",
    "command": {
      "cmd": "edit_feature",
      "uid": "F1",
      "def": {
        "type": "box",
        "plane": {"type": "origin", "plane": "xy"},
        "corner": [0, 0],
        "length": 30,
        "width": 20,
        "height": 15,
        "operation": "new_body"
      }
    }
  }
}
```

Save the editable model, export the solid, and close the saved document:

```json
{"name":"document_save","arguments":{"document":"D1","path":"box.mitcad"}}
{"name":"model_export","arguments":{"document":"D1","path":"box.step","format":"step"}}
{"name":"document_close","arguments":{"document":"D1"}}
```

These are three separate calls, each sent after the previous response.
Reopen with `document_open` and `{"path":"box.mitcad"}` and use its new
handle for queries. Existing save destinations require an explicit
`"overwrite":true`; export destinations must be new. Use actual IDs
returned by queries when extending an existing design. Native lengths
are in millimetres and angles in radians, with expressions such as
`"15 mm"` or `"30 deg"` supported by the model.

## Transport, errors and limits

The transport is newline-delimited UTF-8 JSON-RPC 2.0 over stdio, with
one object per line. The supported MCP version is `2025-11-25`. A client
first sends `initialize` with `protocolVersion`, `capabilities` and
`clientInfo`, reads the negotiated response, then sends
`notifications/initialized`. If it proposes an older or unsupported
version, the server responds with `2025-11-25`; the client must support
that version to continue. Discovery and tool calls require the completed
handshake. `ping` is available; resource templates are empty. Requests
use string or integer IDs and batches are unsupported.

This follows the [2025-11-25 lifecycle and version negotiation](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle).
Clients using the newer lifecycle may probe `server/discover`; this
server returns method-not-found (`-32601`), allowing them to fall back
to `initialize`, as described by the
[official SDK's protocol-version guide](https://ts.sdk.modelcontextprotocol.io/v2/protocol-versions).

Malformed JSON and invalid requests return JSON-RPC errors; unknown
methods and invalid tool parameters also return protocol errors. A
notification never invokes a tool or mutates a document and has no
response. Model, workspace and file errors return tool results with
`isError:true`, usually `structuredContent:{"error":"..."}`. A failed
command validation leaves the document unchanged.

Request and response frames are limited to 8 MiB, including JSON escaping
and both text and structured tool content. An oversized tool result returns
`isError:true` with `error_code:"response_too_large"` and `result_omitted:true`;
the connection remains usable. If the operation already changed the document,
the compact result preserves `applied:true` and the document handle: do not
retry that operation. `original_is_error` reports whether the omitted result
itself contained an error. Large embedded images may exceed this response
limit even though the design file accepts them.

A failed feature does not reject a command: the command is applied and
the feature is skipped. Every applied `model_command` result has
`failed_features`, every feature in error after it in timeline order
(empty when none): `uid`, `name`, `error`, `edited` (the command added
the feature or edited it: its definition, suppression, component or
place, or it is the `uid` of `edit_feature`, `suppress_feature` or
`reorder_feature`, or the `sketch` of a sketch command) and `new` (it did
not fail before this command). `isError` is true only when the command
was refused or a feature it added or edited fails (mitcad#401). That
result has `isError:true` **and** `applied:true`, with the document's
current state, timeline and bodies: inspect the feature's error, then
edit or undo; do not treat it as proof that nothing changed. A failure
elsewhere in the timeline (an earlier failure, or one this command caused
in a later feature, `new:true`, for example through a parameter) is
listed in `failed_features` but leaves `isError` false: the command did
what it was asked. Opening a document with failed geometry returns a
handle, its snapshot and the failed features, not an error. Export
refuses documents with failed features. File IO errors can leave partial
output; inspect or remove it before choosing a retry destination.

`add_feature` takes `after` (a feature's uid) or `index` (a timeline
position, at most the marker) to insert a feature in the middle of the
timeline in one call: one undo step, one recompute of the features after
it, the marker staying after the feature it was after; a refused insert
changes nothing (mitcad#403, see the command reference).

In headless mode, paths are relative to the workspace or absolute inside it. Parent
traversal (`..`) and symlinks leading outside the workspace are refused;
output parent directories must already exist. Project discovery stops
at the workspace root. Version 3 documents can read their existing
B-rep store inside that root. Opening does not refresh linked
components, read per-user display state or configure the global library
resolver. Embedded bodies remain usable. Installed fonts may be used
when evaluating sketch text. Workspace path checks are intended for a
local client; they do not isolate the process from concurrent filesystem
changes or impose operating-system resource limits.

Requests run synchronously, one at a time. This version does not report
progress or interrupt a running geometry operation;
`notifications/cancelled` cannot cancel that operation. A client may
terminate the headless process, losing its unsaved session documents. That
mode has no background autosave. Document and B-rep reads have size limits; large
geometry computations can still use substantial time and memory.

## Validation

The integration source [tools/cli/mcp-test.py](../tools/cli/mcp-test.py)
launches the actual CLI and covers initialization, discovery (stale
records skipped by `pid` and removed without a connection, `--instance`
probing only its record, records probed in parallel), two document
handles, box modeling and volume, save/open, STEP export, protocol errors,
notifications, workspace boundaries, read-only mode, version negotiation
and EOF cleanup, plus persisted geometry failures, symlink escapes and
OBJ companion-file conflicts. Protocol unit tests accompany the Rust
protocol crate; the native CLI integration tests exercise the FFI backend.
[tools/cli/mcp-results-test.cmake](../tools/cli/mcp-results-test.cmake)
(`cli.mcp_results`, no Python) checks `isError` and `failed_features` with
a failed feature in the timeline, and `add_feature` with `after`.
The macOS `app.ui-macos-mcp` test
([tools/ui-macos-mcp-test.py](../tools/ui-macos-mcp-test.py)) launches the
actual GUI and CLI bridge. It verifies live discovery, private records,
authentication, model edits and undo, native history and viewport
screenshots, reconnect and EOF retention, stale document handles,
ambiguous instances, malformed and oversized input, full-width integer
request IDs, and `--no-mcp`; then the live debugging flow of
[tools/ui-mcp-live.py](../tools/ui-mcp-live.py) (mitcad#265) on a design
whose sketch holds a projection that no longer follows: queries and
`sketch.add_line` with the sketch open and undo, `ui_state` with the
Sketch Palette's warning and its entities, `ui_select`, `ui_screenshot`,
`ui_command` Clean Up Stale Projections and Finish Sketch, `document_save`
and `ui_log`. The same flow runs on Linux in
[tools/ui-mcp-test.sh](../tools/ui-mcp-test.sh) (Xvfb), and on Windows in the
ctest `app.ui-windows-mcp`
([tools/ui-windows-mcp-test.ps1](../tools/ui-windows-mcp-test.ps1), session
0 with Mesa, its own PowerShell client of the bridge). Save As, Open and
the camera (mitcad#402, mitcad#405) are covered on Linux by
[tools/ui-mcp-files-camera-test.sh](../tools/ui-mcp-files-camera-test.sh)
(Xvfb, a shell client of the bridge with jq): an untitled design saved into
the folder `--mcp-folder` allows and refused elsewhere, `..`, other
suffixes and an existing file without `overwrite`; `document_open` with
and without unsaved changes, the new handle and the old one refused;
`ui_camera` reading and setting the camera and framing two equal blocks
500 mm apart, whose screenshots then match; `ui_select`'s fit, Fit with
isolation and Home with a `Home` named view, each read back from the reply.
The folder rules' unit tests are in `app.unit` (`tests/agent_folders.cpp`).
The classification's
unit tests are in `core.model` (`api::classes`), the live catalog's in
`core.mcp`. Run these focused CTests only in the approved isolated
environments, as described in
[development.md](development.md#mcp-validation).
