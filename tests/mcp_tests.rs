//! Integration tests for the MCP server.
//!
//! These tests verify the JSON-RPC protocol implementation.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

mod integration;
use integration::create_test_repo;

/// Send a JSON-RPC request to the MCP server and get the response.
fn send_request(stdin: &mut impl Write, stdout: &mut impl BufRead, request: Value) -> Value {
    // Send request
    let request_str = serde_json::to_string(&request).unwrap();
    writeln!(stdin, "{}", request_str).unwrap();
    stdin.flush().unwrap();

    // Read response
    let mut response_line = String::new();
    let bytes_read = stdout.read_line(&mut response_line).unwrap();

    if bytes_read == 0 || response_line.trim().is_empty() {
        panic!("Empty response from server for request: {}", request_str);
    }

    serde_json::from_str(&response_line).unwrap_or_else(|e| {
        panic!(
            "Failed to parse response '{}' for request '{}': {}",
            response_line.trim(),
            request_str,
            e
        );
    })
}

fn setup_mcp_test() -> (TempDir, std::process::Child) {
    let temp = create_test_repo();

    // Initialize AGIT
    Command::new(env!("CARGO_BIN_EXE_agit"))
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("Failed to init agit");

    // Start the MCP server
    let child = Command::new(env!("CARGO_BIN_EXE_agit"))
        .arg("serve")
        .current_dir(temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start MCP server");

    (temp, child)
}

#[test]
fn test_mcp_initialize() {
    let (temp, mut child) = setup_mcp_test();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    let response = send_request(&mut stdin, &mut reader, request);

    // Verify response
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["protocolVersion"].is_string());
    assert!(response["result"]["serverInfo"]["name"].is_string());

    // Clean up
    drop(stdin);
    let _ = child.kill();
    drop(temp);
}

#[test]
fn test_mcp_tools_list() {
    let (temp, mut child) = setup_mcp_test();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize first
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request);

    // List tools
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });

    let response = send_request(&mut stdin, &mut reader, request);

    // Verify tools
    let tools = response["result"]["tools"].as_array().unwrap();
    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(tool_names.contains(&"agit_log_step"));
    assert!(tool_names.contains(&"agit_read_roadmap"));
    assert!(tool_names.contains(&"agit_get_context"));
    assert!(tool_names.contains(&"agit_get_recent_summaries"));

    // Clean up
    drop(stdin);
    let _ = child.kill();
    drop(temp);
}

#[test]
fn test_mcp_log_step() {
    let (temp, mut child) = setup_mcp_test();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request);

    // Call agit_log_step
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "agit_log_step",
            "arguments": {
                "role": "user",
                "category": "intent",
                "content": "Fix the authentication bug"
            }
        }
    });

    let response = send_request(&mut stdin, &mut reader, request);

    // Verify success
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Logged"));

    // Verify entry was added to index
    let index_content = fs::read_to_string(temp.path().join(".agit/index")).unwrap();
    assert!(index_content.contains("Fix the authentication bug"));

    // Clean up
    drop(stdin);
    let _ = child.kill();
    drop(temp);
}

#[test]
fn test_mcp_log_step_invalid_role() {
    let (temp, mut child) = setup_mcp_test();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request);

    // Call with invalid role
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "agit_log_step",
            "arguments": {
                "role": "invalid",
                "category": "intent",
                "content": "Test"
            }
        }
    });

    let response = send_request(&mut stdin, &mut reader, request);

    // Verify error
    assert_eq!(response["result"]["isError"], true);

    // Clean up
    drop(stdin);
    let _ = child.kill();
    drop(temp);
}

#[test]
fn test_mcp_read_roadmap() {
    let (temp, mut child) = setup_mcp_test();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request);

    // Read roadmap (no commits yet)
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "agit_read_roadmap",
            "arguments": {}
        }
    });

    let response = send_request(&mut stdin, &mut reader, request);

    // Should return helpful message, not error
    assert!(response["result"]["isError"].is_null());
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("No roadmap"));

    // Clean up
    drop(stdin);
    let _ = child.kill();
    drop(temp);
}

#[test]
fn test_mcp_unknown_method() {
    let (temp, mut child) = setup_mcp_test();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize first (required for proper MCP protocol)
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request);

    // Call unknown method
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "unknown/method"
    });

    let response = send_request(&mut stdin, &mut reader, request);

    // Verify error response
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32601); // Method not found

    // Clean up
    drop(stdin);
    let _ = child.kill();
    drop(temp);
}
