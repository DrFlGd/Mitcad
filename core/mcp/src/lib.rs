// SPDX-License-Identifier: MIT
//! MCP 2025-11-25 protocol for stdio and live GUI tools, without geometry.
//!
//! The backend owns documents and validates their operations and filesystem
//! paths. This crate owns protocol envelopes, discovery and tool argument shapes.
//! The live profile defers tool execution to the application's document owner.
//! Calls run synchronously; cancellation of an active kernel call is not offered.

mod catalog;
mod framing;

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

/// The initialization-based MCP revision implemented by this server.
pub const PROTOCOL_VERSION: &str = "2025-11-25";
/// Maximum bytes in one request or response, excluding its newline delimiter.
/// Kept in sync with the live transport's `mcp/LocalEndpoint.hpp`.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
// Leave room for a compact error that preserves the request id and, for
// an applied command, acknowledges that the document already changed.
const RESPONSE_RESERVE_BYTES: usize = 4096;
/// Resource containing the model's command and query reference.
pub const COMMANDS_URI: &str = "mitcad://reference/commands";

const COMMANDS_REFERENCE: &str = include_str!("../../model/src/api/commands.md");

/// Data returned by a document tool. Errors remain visible to the model.
pub struct ToolResult {
    pub value: Value,
    pub is_error: bool,
}

/// Document operations supplied by the application or a mock geometry backend.
pub trait Backend {
    /// An error here is a tool execution failure, rather than a protocol error.
    fn call(&mut self, name: &str, arguments: &Value) -> Result<ToolResult, String>;
}

/// Read requests until EOF and flush one compact JSON response per line.
///
/// Oversized messages are drained through their newline without allocating an
/// unbounded buffer. Diagnostics belong on stderr, never on `writer`.
pub fn serve<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    backend: impl Backend,
) -> io::Result<()> {
    let mut session = Session::new(backend);
    let mut frame = Vec::new();
    while let Some(status) = framing::read_frame(&mut reader, &mut frame, MAX_FRAME_BYTES)? {
        let response = match status {
            framing::FrameStatus::TooLarge => {
                Some(error(None, -32600, "Message exceeds the 8 MiB input limit"))
            }
            framing::FrameStatus::Complete => match serde_json::from_slice(&frame) {
                Ok(message) => session.handle(message),
                Err(_) => Some(error(None, -32700, "Invalid JSON message")),
            },
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut writer, &response).map_err(io::Error::other)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Uninitialized,
    AwaitingInitialized,
    Ready,
}

#[derive(Clone, Copy)]
enum Profile {
    Headless,
    Live,
}

enum Dispatch {
    Response(Option<Value>),
    Tool {
        id: Value,
        name: String,
        arguments: Value,
    },
}

impl Dispatch {
    fn response(value: Value) -> Self {
        Self::Response(Some(value))
    }
}

struct Session<B> {
    phase: Phase,
    backend: B,
    profile: Profile,
}

impl<B: Backend> Session<B> {
    fn new(backend: B) -> Self {
        Self {
            phase: Phase::Uninitialized,
            backend,
            profile: Profile::Headless,
        }
    }

    fn handle(&mut self, message: Value) -> Option<Value> {
        let response = match self.prepare(message) {
            Dispatch::Response(response) => response,
            Dispatch::Tool {
                id,
                name,
                arguments,
            } => Some(match self.backend.call(&name, &arguments) {
                Ok(result) => success(&id, tool_content(result.value, result.is_error)),
                Err(message) => success(&id, tool_error(message)),
            }),
        };
        response.map(bounded_response)
    }

    fn tools(&self) -> Vec<Value> {
        match self.profile {
            Profile::Headless => catalog::tools(),
            Profile::Live => catalog::live_tools(),
        }
    }

    fn prepare(&mut self, message: Value) -> Dispatch {
        let Some(object) = message.as_object() else {
            return Dispatch::response(error(None, -32600, "Expected one JSON-RPC object"));
        };
        let id = object.get("id").filter(|id| valid_id(id));
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Dispatch::response(error(id, -32600, "jsonrpc must be 2.0"));
        }
        // No server-to-client requests are issued, so there is no response to
        // consume. Never answer a response with another response.
        if !object.contains_key("method")
            && (object.contains_key("result") || object.contains_key("error"))
        {
            return Dispatch::Response(None);
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return Dispatch::response(error(id, -32600, "method must be a string"));
        };
        let notification = !object.contains_key("id");
        if !notification && id.is_none() {
            return Dispatch::response(error(None, -32600, "id must be a string or integer"));
        }
        if object.contains_key("result") || object.contains_key("error") {
            return Dispatch::Response(
                (!notification)
                    .then(|| error(id, -32600, "A request cannot contain result or error")),
            );
        }
        let empty = json!({});
        let params = object.get("params").unwrap_or(&empty);
        if !params.is_object() {
            return Dispatch::Response(
                (!notification).then(|| error(id, -32602, "params must be an object")),
            );
        }
        if notification {
            if method == "notifications/initialized" && self.phase == Phase::AwaitingInitialized {
                self.phase = Phase::Ready;
            }
            // Request-only methods, unknown notifications, and cancellation
            // notifications for completed or uncancellable calls do no work.
            return Dispatch::Response(None);
        }
        let id = id.expect("validated request id");
        if !fits_json(id, MAX_FRAME_BYTES - RESPONSE_RESERVE_BYTES) {
            return Dispatch::response(error(
                None,
                -32600,
                "Request ID leaves no room for a bounded response",
            ));
        }
        let result = match method {
            "initialize" => return Dispatch::response(self.initialize(id, params)),
            "ping" => json!({}),
            "tools/list"
            | "tools/call"
            | "resources/list"
            | "resources/read"
            | "resources/templates/list" => {
                if self.phase != Phase::Ready {
                    return Dispatch::response(error(
                        Some(id),
                        -32600,
                        "Send initialize and notifications/initialized before using tools or resources",
                    ));
                }
                match method {
                    "tools/list" => {
                        if let Some(response) = reject_cursor(id, params) {
                            return Dispatch::response(response);
                        }
                        json!({"tools": self.tools()})
                    }
                    "tools/call" => return self.prepare_call(id, params),
                    "resources/list" => {
                        if let Some(response) = reject_cursor(id, params) {
                            return Dispatch::response(response);
                        }
                        json!({"resources": [{
                            "uri": COMMANDS_URI,
                            "name": "Mitcad document API",
                            "description": "JSON command and query reference, identities and metric units",
                            "mimeType": "text/markdown"
                        }]})
                    }
                    "resources/templates/list" => {
                        if let Some(response) = reject_cursor(id, params) {
                            return Dispatch::response(response);
                        }
                        json!({"resourceTemplates": []})
                    }
                    "resources/read" => {
                        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                            return Dispatch::response(error(
                                Some(id),
                                -32602,
                                "uri must be a string",
                            ));
                        };
                        if uri != COMMANDS_URI {
                            return Dispatch::response(error(
                                Some(id),
                                -32002,
                                "Unknown resource URI",
                            ));
                        }
                        json!({"contents": [{
                            "uri": COMMANDS_URI,
                            "mimeType": "text/markdown",
                            "text": COMMANDS_REFERENCE
                        }]})
                    }
                    _ => unreachable!(),
                }
            }
            // In particular, a modern client's server/discover probe gets a
            // deterministic method-not-found reply before legacy initialization.
            _ => return Dispatch::response(error(Some(id), -32601, "Unknown method")),
        };
        Dispatch::response(success(id, result))
    }

    fn initialize(&mut self, id: &Value, params: &Value) -> Value {
        if self.phase != Phase::Uninitialized {
            return error(Some(id), -32600, "Server is already initialized");
        }
        if params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .is_none()
            || !params.get("capabilities").is_some_and(Value::is_object)
            || !params.get("clientInfo").is_some_and(|info| {
                info.is_object()
                    && info.get("name").and_then(Value::as_str).is_some()
                    && info.get("version").and_then(Value::as_str).is_some()
            })
        {
            return error(
                Some(id),
                -32602,
                "initialize requires protocolVersion, capabilities, and clientInfo with name and version",
            );
        }
        // MCP legacy negotiation returns a supported version when the client's
        // proposed revision differs. The client decides whether it can proceed.
        self.phase = Phase::AwaitingInitialized;
        let instructions = match self.profile {
            Profile::Headless => {
                "Local headless CAD documents. Read mitcad://reference/commands for model operations. Use the explicit document handle on every call. Files are confined to the configured workspace. Commands are single objects; lengths default to millimetres. Documents last until closed or the server exits."
            }
            Profile::Live => {
                "Control the document already open in the Mitcad application, alongside its user. Call document_list to obtain its current handle; handles become invalid when the document is replaced. Read mitcad://reference/commands for model operations. Commands and queries are single objects; lengths default to millimetres. Queries work at any time except while the model computes (the result then says busy and when to retry). While a sketch is open, its sketch.* commands go through the sketch session; other model commands wait until the sketch or command panel is finished, and editing is refused when the document is read-only. ui_state shows what the user sees (mode, selection, Sketch Palette warnings, messages), ui_log the application's log, ui_screenshot the view; ui_select points at entities, ui_command runs the application's commands by id, document_save saves to the document's own path. Changes you make are labelled (agent) in the undo text and undo with Ctrl+Z. File imports, opening, closing, saving elsewhere and exporting are not available through this connection. Disconnecting leaves the application's document open, including unsaved changes."
            }
        };
        success(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "mitcad", "version": env!("CARGO_PKG_VERSION")},
                "instructions": instructions
            }),
        )
    }

    fn prepare_call(&mut self, id: &Value, params: &Value) -> Dispatch {
        if params.get("task").is_some() {
            return Dispatch::response(error(
                Some(id),
                -32602,
                "Task-augmented tool calls are not supported",
            ));
        }
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Dispatch::response(error(Some(id), -32602, "tools/call requires a tool name"));
        };
        let Some(tool) = self
            .tools()
            .into_iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        else {
            return Dispatch::response(error(Some(id), -32602, "Unknown tool name"));
        };
        let empty = json!({});
        let arguments = params.get("arguments").unwrap_or(&empty);
        if !arguments.is_object() {
            return Dispatch::response(error(Some(id), -32602, "arguments must be an object"));
        }
        if let Err(message) = catalog::validate(arguments, &tool["inputSchema"], "arguments") {
            return Dispatch::response(error(Some(id), -32602, &message));
        }
        Dispatch::Tool {
            id: id.clone(),
            name: name.to_owned(),
            arguments: arguments.clone(),
        }
    }
}

struct DeferredBackend;

impl Backend for DeferredBackend {
    fn call(&mut self, _: &str, _: &Value) -> Result<ToolResult, String> {
        Err("live tools must be completed by the application".to_owned())
    }
}

/// A validated message for the live application's asynchronous dispatcher.
pub enum PreparedMessage {
    /// Protocol reply, or no reply for a notification or client response.
    Response(Option<Value>),
    /// One tool invocation to complete with [`LiveSession::finish`].
    Tool { name: String, arguments: Value },
}

impl PreparedMessage {
    /// JSON dispatch plan for the application bridge. Protocol responses
    /// remain serialized text so a host JSON parser cannot round request IDs.
    pub fn into_plan(self) -> Value {
        match self {
            Self::Response(response) => {
                json!({"response": response.map(|response| response.to_string())})
            }
            Self::Tool { name, arguments } => json!({"tool": name, "arguments": arguments}),
        }
    }
}

/// MCP lifecycle and one pending tool call, without owning a CAD document.
/// The application retains ownership and executes tools on its model worker.
pub struct LiveSession {
    session: Session<DeferredBackend>,
    pending: Option<Value>,
}

impl Default for LiveSession {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveSession {
    pub fn new() -> Self {
        let mut session = Session::new(DeferredBackend);
        session.profile = Profile::Live;
        Self {
            session,
            pending: None,
        }
    }

    /// Parse one JSON frame using the same errors as the stdio transport.
    pub fn prepare_text(&mut self, message: &str) -> Result<PreparedMessage, String> {
        if self.pending.is_some() {
            return Err("a live MCP tool call is already pending".to_owned());
        }
        if message.len() > MAX_FRAME_BYTES {
            return Ok(PreparedMessage::Response(Some(error(
                None,
                -32600,
                "Message exceeds the 8 MiB input limit",
            ))));
        }
        match serde_json::from_str(message) {
            Ok(message) => self.prepare(message),
            Err(_) => Ok(PreparedMessage::Response(Some(error(
                None,
                -32700,
                "Invalid JSON message",
            )))),
        }
    }

    /// Refuses another message until the previous tool has been completed.
    pub fn prepare(&mut self, message: Value) -> Result<PreparedMessage, String> {
        if self.pending.is_some() {
            return Err("a live MCP tool call is already pending".to_owned());
        }
        Ok(match self.session.prepare(message) {
            Dispatch::Response(response) => {
                PreparedMessage::Response(response.map(bounded_response))
            }
            Dispatch::Tool {
                id,
                name,
                arguments,
            } => {
                self.pending = Some(id);
                PreparedMessage::Tool { name, arguments }
            }
        })
    }

    /// Wrap the application's result and release the pending request ID.
    pub fn finish(&mut self, value: Value, is_error: bool) -> Result<Value, String> {
        let id = self
            .pending
            .take()
            .ok_or_else(|| "no live MCP tool call is pending".to_owned())?;
        Ok(bounded_response(success(
            &id,
            tool_content(value, is_error),
        )))
    }
}

fn valid_id(id: &Value) -> bool {
    id.is_string() || id.as_i64().is_some() || id.as_u64().is_some()
}

fn reject_cursor(id: &Value, params: &Value) -> Option<Value> {
    params
        .get("cursor")
        .map(|_| error(Some(id), -32602, "This list is not paginated; omit cursor"))
}

fn success(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: Option<&Value>, code: i32, message: &str) -> Value {
    // This MCP revision permits an omitted error ID for an unreadable request;
    // a null ID is not a valid MCP RequestId.
    let mut response = json!({"jsonrpc": "2.0", "error": {"code": code, "message": message}});
    if let Some(id) = id {
        response["id"] = id.clone();
    }
    response
}

fn tool_error(message: String) -> Value {
    tool_content(json!({"error": message}), true)
}

fn tool_content(value: Value, is_error: bool) -> Value {
    // The 2025-11-25 structuredContent field is an object. Wrap a backend's
    // scalar or array while keeping its JSON data intact and the text identical.
    let value = if value.is_object() {
        value
    } else {
        json!({"value": value})
    };
    json!({
        "content": [{"type": "text", "text": value.to_string()}],
        "structuredContent": value,
        "isError": is_error
    })
}

/// Count the actual escaped JSON bytes without allocating a serialized
/// copy. Stop as soon as the transport budget is exceeded.
fn fits_json(value: &Value, maximum: usize) -> bool {
    struct Budget(usize);
    impl Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.0 {
                return Err(io::Error::other("MCP response size limit"));
            }
            self.0 -= bytes.len();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget(maximum), value).is_ok()
}

/// The model can return images and histories much larger than a transport
/// frame. Report that as a tool error, without losing the connection or
/// concealing a mutation that has already been applied (mitcad#167).
fn bounded_response(response: Value) -> Value {
    if fits_json(&response, MAX_FRAME_BYTES) {
        return response;
    }
    let id = response.get("id");
    if let Some(original) = response["result"]["structuredContent"].as_object() {
        let applied = original.get("applied").and_then(Value::as_bool);
        let message = if applied == Some(true) {
            "The operation was applied, but its result exceeds the 8 MiB response limit. Do not retry it; query smaller parts of the document."
        } else {
            "The result exceeds the 8 MiB response limit. Request a smaller result."
        };
        let mut compact = json!({
            "error": message,
            "error_code": "response_too_large",
            "result_omitted": true,
            "original_is_error": response["result"]["isError"].as_bool().unwrap_or(false),
        });
        if let Some(applied) = applied {
            compact["applied"] = json!(applied);
        }
        if let Some(document) = original.get("document").and_then(Value::as_str)
            && document.len() <= 128
        {
            compact["document"] = json!(document);
        }
        if let Some(id) = id {
            let compact = success(id, tool_content(compact, true));
            if fits_json(&compact, MAX_FRAME_BYTES) {
                return compact;
            }
        }
    }
    let compact = error(id, -32000, "Response exceeds the 8 MiB limit");
    if fits_json(&compact, MAX_FRAME_BYTES) {
        compact
    } else {
        // Invalid envelopes can supply huge ids before normal request
        // validation. No tool has run for those messages.
        error(None, -32600, "Request ID exceeds the response size limit")
    }
}

#[cfg(test)]
mod tests;
