// SPDX-License-Identifier: MIT
//! What the runner takes from ctest: the tests and their properties
//! (`ctest --show-only=json-v1`), the command line that runs one test,
//! and the result line ctest prints for it.

use serde_json::Value;

/// One test of the build tree, with the properties the scheduler needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestDef {
    pub name: String,
    /// Has a command (ctest lists tests of other configurations without).
    pub available: bool,
    pub disabled: bool,
    pub depends: Vec<String>,
    pub fixtures_setup: Vec<String>,
    pub fixtures_cleanup: Vec<String>,
    pub fixtures_required: Vec<String>,
    pub resource_locks: Vec<String>,
    pub run_serial: bool,
    pub processors: f64,
    pub timeout: Option<f64>,
    pub labels: Vec<String>,
    /// The command's words and ENVIRONMENT (changed.rs).
    pub command: Vec<String>,
    pub environment: Vec<String>,
}

/// A property's value as a list of strings: a JSON list, or one string
/// (ctest writes lists as arrays; a single value may come as a string).
fn strings(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Value::String(s) if !s.is_empty() => s.split(';').map(str::to_owned).collect(),
        _ => Vec::new(),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => true_word(s),
        _ => false,
    }
}

/// CMake's true constants that ctest might pass through as text.
fn true_word(s: &str) -> bool {
    let upper = s.trim().to_ascii_uppercase();
    upper == "1" || upper == "ON" || upper == "TRUE" || upper == "YES"
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The tests of `ctest --show-only=json-v1`, in ctest's order.
pub fn parse_tests(json: &str) -> Result<Vec<TestDef>, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("ctest's JSON does not parse: {e}"))?;
    let Some(tests) = root.get("tests").and_then(Value::as_array) else {
        return Err("ctest's JSON has no tests".into());
    };
    let mut defs = Vec::new();
    for test in tests {
        let Some(name) = test.get("name").and_then(Value::as_str) else {
            continue;
        };
        let mut def = TestDef {
            name: name.to_owned(),
            available: test.get("command").is_some_and(|c| !strings(c).is_empty()),
            command: test.get("command").map(strings).unwrap_or_default(),
            ..TestDef::default()
        };
        let properties = test.get("properties").and_then(Value::as_array);
        for property in properties.into_iter().flatten() {
            let Some(key) = property.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(value) = property.get("value") else {
                continue;
            };
            match key {
                "DEPENDS" => def.depends = strings(value),
                "FIXTURES_SETUP" => def.fixtures_setup = strings(value),
                "FIXTURES_CLEANUP" => def.fixtures_cleanup = strings(value),
                "FIXTURES_REQUIRED" => def.fixtures_required = strings(value),
                "RESOURCE_LOCK" => def.resource_locks = strings(value),
                "RUN_SERIAL" => def.run_serial = truthy(value),
                "DISABLED" => def.disabled = truthy(value),
                "PROCESSORS" => def.processors = number(value).unwrap_or(0.0),
                "TIMEOUT" => def.timeout = number(value),
                "LABELS" => def.labels = strings(value),
                "ENVIRONMENT" => def.environment = strings(value),
                _ => {}
            }
        }
        defs.push(def);
    }
    Ok(defs)
}

/// The test name as a regular expression that matches only it.
pub fn exact_regex(name: &str) -> String {
    let mut out = String::from("^");
    for c in name.chars() {
        if "\\.^$|()[]{}*+?".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('$');
    out
}

/// ctest's arguments that run one test of the build tree alone: no
/// fixtures added (the runner orders the setup and cleanup tests itself),
/// the output of a failure in the log.
pub fn run_args(build: &str, config: Option<&str>, name: &str) -> Vec<String> {
    let mut args = vec![
        "--test-dir".to_owned(),
        build.to_owned(),
        "-R".to_owned(),
        exact_regex(name),
        "-FA".to_owned(),
        ".*".to_owned(),
        "--output-on-failure".to_owned(),
        "--no-tests=error".to_owned(),
    ];
    if let Some(config) = config {
        args.push("-C".to_owned());
        args.push(config.to_owned());
    }
    args
}

/// The result ctest printed for a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Passed,
    Skipped,
    Failed,
    Timeout,
    NotRun,
}

/// The result line of the test in ctest's output, e.g.
/// `1/1 Test #12: core.model ......   Passed    3.21 sec` or
/// `...***Timeout 1500.04 sec`; None when there is none.
pub fn parse_status(output: &str, name: &str) -> Option<Status> {
    let marker = format!(": {name} ");
    for line in output.lines() {
        if !line.contains("Test #") || !line.contains(&marker) {
            continue;
        }
        let rest = &line[line.find(&marker)? + marker.len()..];
        let verdict = rest.trim_start_matches([' ', '.']);
        let status = if verdict.starts_with("Passed") {
            Status::Passed
        } else if verdict.contains("Skipped") {
            Status::Skipped
        } else if verdict.contains("Timeout") {
            Status::Timeout
        } else if verdict.contains("Not Run") {
            Status::NotRun
        } else {
            // ***Failed, ***Exception: ..., and anything else.
            Status::Failed
        };
        return Some(status);
    }
    None
}

/// The log says the disk was full.
pub fn says_disk_full(output: &str) -> bool {
    const SIGNS: [&str; 3] = [
        "No space left on device",
        "There is not enough space on the disk",
        "ERROR_DISK_FULL",
    ];
    SIGNS.iter().any(|sign| output.contains(sign))
}

/// A simple pattern: `*` matches any run of characters, the rest itself.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    if !text.starts_with(first) || text.len() < first.len() + last.len() {
        return false;
    }
    if !text[first.len()..].ends_with(last) {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
      "kind": "ctestInfo",
      "version": { "major": 1, "minor": 0 },
      "tests": [
        {
          "name": "core.cargo_build",
          "command": ["cmake", "-P", "cargo-tests.cmake"],
          "properties": [
            { "name": "FIXTURES_SETUP", "value": ["cargo_tests_debug"] },
            { "name": "RUN_SERIAL", "value": true },
            { "name": "TIMEOUT", "value": 3600.0 }
          ]
        },
        {
          "name": "core.model",
          "command": ["cmake", "-P", "cargo-tests.cmake", "--", "run"],
          "properties": [
            { "name": "FIXTURES_REQUIRED", "value": ["cargo_tests_debug"] },
            { "name": "RESOURCE_LOCK", "value": ["cargo_build"] },
            { "name": "LABELS", "value": ["rust", "model"] },
            { "name": "PROCESSORS", "value": 4 }
          ]
        },
        { "name": "other.config", "properties": [] },
        {
          "name": "app.ui-windows",
          "command": ["pwsh"],
          "properties": [
            { "name": "DEPENDS", "value": ["core.model"] },
            { "name": "DISABLED", "value": false }
          ]
        }
      ]
    }"#;

    #[test]
    fn tests_and_properties_are_read() {
        let tests = parse_tests(JSON).unwrap();
        let names: Vec<&str> = tests.iter().map(|t| t.name.as_str()).collect();
        let expected = [
            "core.cargo_build",
            "core.model",
            "other.config",
            "app.ui-windows",
        ];
        assert_eq!(names, expected);
        let build = &tests[0];
        assert!(build.run_serial && build.available);
        assert_eq!(build.fixtures_setup, ["cargo_tests_debug"]);
        assert_eq!(build.timeout, Some(3600.0));
        let model = &tests[1];
        assert_eq!(model.fixtures_required, ["cargo_tests_debug"]);
        assert_eq!(model.resource_locks, ["cargo_build"]);
        // A lock and a fixture make no serial test.
        assert!(!model.run_serial);
        assert_eq!(model.labels, ["rust", "model"]);
        assert_eq!(model.processors, 4.0);
        assert!(!tests[2].available);
        assert_eq!(tests[3].depends, ["core.model"]);
        assert!(!tests[3].disabled);
        assert!(parse_tests("{}").is_err());
    }

    #[test]
    fn names_become_exact_expressions() {
        assert_eq!(exact_regex("core.model"), "^core\\.model$");
        assert_eq!(exact_regex("a+b(c)"), "^a\\+b\\(c\\)$");
        let args = run_args("build/dev", None, "x.y");
        assert_eq!(args[..4], ["--test-dir", "build/dev", "-R", "^x\\.y$"]);
        assert!(args.contains(&"-FA".to_owned()));
        let configured = run_args("b", Some("Debug"), "t");
        assert_eq!(configured[configured.len() - 2..], ["-C", "Debug"]);
    }

    #[test]
    fn result_lines_are_read() {
        let cases = [
            ("Test #1: t.x ...   Passed  3.2 sec", Status::Passed),
            ("Test #2: t.x ...***Failed  1.0 sec", Status::Failed),
            ("Test #3: t.x ...***Timeout 1500.0 sec", Status::Timeout),
            ("Test #4: t.x ...***Skipped 0.1 sec", Status::Skipped),
            ("Test #5: t.x ...***Exception: SegFault", Status::Failed),
            ("Test #6: t.x ...***Not Run 0.0 sec", Status::NotRun),
        ];
        for (line, status) in cases {
            assert_eq!(parse_status(line, "t.x"), Some(status), "{line}");
            // Another test's line.
            assert_eq!(parse_status(line, "t"), None, "{line}");
        }
        assert_eq!(parse_status("No tests were found!!!", "t.x"), None);
    }

    #[test]
    fn full_disks_are_seen() {
        assert!(says_disk_full("write: No space left on device"));
        assert!(!says_disk_full("all good"));
    }

    #[test]
    fn globs_match() {
        assert!(glob_match("app.ui-*", "app.ui-windows-timber"));
        assert!(glob_match("*corpus*", "f3d.corpus_occt"));
        assert!(glob_match("core.model", "core.model"));
        assert!(!glob_match("core.model", "core.model.api"));
        assert!(glob_match("a*b*c", "aXbYc"));
        assert!(!glob_match("a*b*c", "aXc"));
        assert!(!glob_match("ab*ba", "aba"));
    }
}
