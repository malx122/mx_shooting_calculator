//! MCP server for MX Shooting Calculator: lets an AI assistant (LM Studio,
//! Claude Desktop, Claude Code, Jan…) read the hunter's rifles and bullets
//! and work out holdovers with the app's own ballistic engine.
//!
//! The assistant starts this program and talks JSON-RPC to it over stdin
//! and stdout, one message per line (MCP's stdio transport). It opens the
//! same database as the desktop app (or `--db <file>` / `MXSC_DB`).

mod tools;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use ballistics_store::Store;
use serde_json::{Value, json};

const APP_NAME: &str = "MX Shooting Calculator";
/// MCP versions this server speaks, newest first.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const INSTRUCTIONS: &str = "Tools for MX Shooting Calculator, the hunter's ballistic calculator. \
    Use `holdover` or `dope_table` for any question about where to aim, dialling, drop, wind \
    drift, energy or ethical range: they run the app's engine on the hunter's own rifles, \
    bullets and measured velocities, so never estimate those numbers yourself. Use the list \
    tools to find names. Ranges are metres, wind m/s, wind direction a clock face (3 = from \
    the right).";

fn main() {
    let store = match database_path().and_then(|path| {
        Store::open(&path).map_err(|error| format!("could not open {}: {error}", path.display()))
    }) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("mxsc-mcp: {error}");
            std::process::exit(1);
        }
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(&store, &line) {
            let _ = writeln!(stdout, "{reply}");
            let _ = stdout.flush();
        }
    }
}

/// Answers one JSON-RPC message; notifications get no answer.
fn handle(store: &Store, line: &str) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(_) => return Some(error(Value::Null, -32700, "not JSON")),
    };
    // A notification (no id), such as notifications/initialized.
    let id = message.get("id")?.clone();
    let params = &message["params"];
    let result = match message["method"].as_str().unwrap_or("") {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or("");
            let version = PROTOCOL_VERSIONS
                .iter()
                .find(|&&known| known == asked)
                .unwrap_or(&PROTOCOL_VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": "mxsc",
                    "title": APP_NAME,
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools::list() }),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            match tools::call(store, name, &arguments) {
                Ok(answer) => {
                    let mut result = json!({
                        "content": [{ "type": "text", "text": answer.text }],
                        "isError": false,
                    });
                    if let Some(data) = answer.data {
                        result["structuredContent"] = data;
                    }
                    result
                }
                Err(message) => json!({
                    "content": [{ "type": "text", "text": message }],
                    "isError": true,
                }),
            }
        }
        method => return Some(error(id, -32601, &format!("unknown method {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The desktop app's database, unless `--db <file>` or `MXSC_DB` says
/// otherwise.
fn database_path() -> Result<PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--db" {
            return arguments
                .next()
                .map(PathBuf::from)
                .ok_or("--db needs a file path".to_owned());
        }
    }
    if let Some(path) = std::env::var_os("MXSC_DB") {
        return Ok(PathBuf::from(path));
    }
    let directories = directories::ProjectDirs::from("", "", APP_NAME)
        .ok_or("could not find the app's data folder")?;
    let path = directories.data_dir().join("mxsc.sqlite");
    if !path.exists() {
        return Err(format!(
            "no database at {}: open the desktop app once, or pass --db",
            path.display()
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_lists_and_calls_tools() {
        let mut store = Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        let reply = |line: &str| handle(&store, line);

        let init = reply(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(reply(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());

        let list = reply(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 5);

        let call = reply(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"holdover","arguments":{"rifle":"x-bolt","range_m":200}}}"#).unwrap();
        assert_eq!(call["id"], 3);
        assert_eq!(call["result"]["isError"], false);
        assert!(
            call["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("X-Bolt")
        );
        assert!(call["result"]["structuredContent"]["hold"].is_object());

        let bad = reply(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"holdover","arguments":{"rifle":"x-bolt"}}}"#).unwrap();
        assert_eq!(bad["result"]["isError"], true);

        let unknown = reply(r#"{"jsonrpc":"2.0","id":5,"method":"nope"}"#).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
}
