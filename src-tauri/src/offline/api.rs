use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tiny_http::Request;

use crate::{success, warn};

/// Shared response type used by handle_request and server.rs
pub struct HandlerResponse {
  pub status: u16,
  pub body: String,
  pub content_type: &'static str,
}

#[derive(Serialize, Deserialize)]
struct Options {
  body: Option<String>,
  method: Option<String>,
}

#[derive(Serialize)]
pub struct ApiResponse {
  pub status: u16,
  pub body: String,
}

/// proxy a fetch call from the webview to the local server
#[tauri::command]
pub fn api_request(url: String, options: String) -> ApiResponse {
  let client = reqwest::blocking::Client::new();
  let options: Options = serde_json::from_str(&options).unwrap_or(Options {
    body: None,
    method: None,
  });
  let body = options.body.clone().unwrap_or_default();
  let method = options.method.clone().unwrap_or_else(|| "GET".to_string());

  // Strip the host portion; the game sends e.g. http://localhost:8001/account/info
  let path = url
    .split_once("http://")
    .unwrap_or(("", ""))
    .1
    .split_once('/')
    .unwrap_or(("", ""))
    .1;
  let forward_url = format!("{}/api/{}", crate::LOCAL_URL, path);

  let response = match method.as_str() {
    "GET" => client.get(&forward_url).send(),
    "POST" => client.post(&forward_url).body(body).send(),
    "PUT" => client.put(&forward_url).body(body).send(),
    "DELETE" => client.delete(&forward_url).send(),
    _ => client.get(&forward_url).send(),
  };

  match response {
    Ok(resp) => {
      let status = resp.status().as_u16();
      let text = resp.text().unwrap_or_default();
      success!(
        "API [{}] {}: {}",
        status,
        forward_url,
        &text[..text.len().min(120)]
      );
      ApiResponse { status, body: text }
    }
    Err(e) => {
      warn!("API request error: {:?}", e);
      ApiResponse {
        status: 500,
        body: String::new(),
      }
    }
  }
}

/// Request handler called by the tiny_http server
pub fn handle_request(request: &mut Request) -> HandlerResponse {
  let mut body = String::new();
  let _ = request.as_reader().read_to_string(&mut body);

  let full_path = request.url();
  let after_api = full_path
    .strip_prefix("/api/")
    .or_else(|| full_path.strip_prefix("api/"))
    .unwrap_or(full_path);

  let (path, query_str) = match after_api.split_once('?') {
    Some((p, q)) => (p, q),
    None => (after_api, ""),
  };

  let params: HashMap<String, String> = query_str
    .split('&')
    .filter(|s| !s.is_empty())
    .filter_map(|kv| {
      let (k, v) = kv.split_once('=')?;
      Some((k.to_string(), v.to_string()))
    })
    .collect();

  let method = request.method().as_str();
  let saves_dir = crate::util::paths::get_saves_dir();

  route_request(method, path, &params, &body, &saves_dir)
}

pub fn route_request(
  method: &str,
  path: &str,
  params: &HashMap<String, String>,
  body: &str,
  saves_dir: &Path,
) -> HandlerResponse {
  match (method, path) {
    ("GET", "account/info") => HandlerResponse {
      status: 200,
      body: r#"{"username":"Guest","lastSessionSlot":-1,"discordId":"","googleId":"","hasAdminRole":false}"#.to_string(),
      content_type: "application/json",
    },

    ("POST", "account/login") => HandlerResponse {
      status: 200,
      body: r#"{"token":"offline"}"#.to_string(),
      content_type: "application/json",
    },

    ("GET", "account/logout") => ok_empty(),

    ("GET", "savedata/system/get") => {
      let file = saves_dir.join("system.json");
      match fs::read_to_string(&file) {
        Ok(contents) => HandlerResponse {
          status: 200,
          body: contents,
          content_type: "application/json",
        },
        // game treats 404 as a new account and auto-generates default data,
        // then calls system/update to persist it
        Err(_) => HandlerResponse {
          status: 404,
          body: String::new(),
          content_type: "text/plain",
        },
      }
    }

    ("POST", "savedata/system/update") => {
      match fs::write(saves_dir.join("system.json"), body) {
        Ok(_) => ok_empty(),
        Err(e) => {
          warn!("Failed to write system.json: {:?}", e);
          error_response("failed to write system save")
        }
      }
    }

    ("GET", "savedata/system/verify") => {
      let file = saves_dir.join("system.json");
      let system_value: serde_json::Value = fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

      let resp = serde_json::json!({ "valid": true, "systemData": system_value });
      HandlerResponse {
        status: 200,
        body: resp.to_string(),
        content_type: "application/json",
      }
    }

    ("GET", "savedata/session/get") => {
      let slot = match parse_slot(params) {
        Some(s) => s,
        None => return bad_request("invalid or missing slot"),
      };
      let file = saves_dir.join(format!("session_{}.json", slot));
      match fs::read_to_string(&file) {
        Ok(contents) => HandlerResponse {
          status: 200,
          body: contents,
          content_type: "application/json",
        },
        Err(_) => HandlerResponse {
          status: 404,
          body: String::new(),
          content_type: "text/plain",
        },
      }
    }

    ("POST", "savedata/session/update") => {
      let slot = match parse_slot(params) {
        Some(s) => s,
        None => return bad_request("invalid or missing slot"),
      };
      match fs::write(saves_dir.join(format!("session_{}.json", slot)), body) {
        Ok(_) => ok_empty(),
        Err(e) => {
          warn!("Failed to write session_{}.json: {:?}", slot, e);
          error_response("failed to write session save")
        }
      }
    }

    ("GET", "savedata/session/delete") => {
      let slot = match parse_slot(params) {
        Some(s) => s,
        None => return bad_request("invalid or missing slot"),
      };
      let _ = fs::remove_file(saves_dir.join(format!("session_{}.json", slot)));
      ok_empty()
    }

    ("POST", "savedata/session/clear") => {
      let slot = match parse_slot(params) {
        Some(s) => s,
        None => return bad_request("invalid or missing slot"),
      };
      let _ = fs::remove_file(saves_dir.join(format!("session_{}.json", slot)));
      HandlerResponse {
        status: 200,
        body: r#"{"success":true}"#.to_string(),
        content_type: "application/json",
      }
    }

    ("GET", "savedata/session/newclear") => HandlerResponse {
      status: 200,
      body: "true".to_string(),
      content_type: "application/json",
    },

    ("POST", "savedata/updateall") => {
      let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
          warn!("Failed to parse updateall body: {:?}", e);
          return bad_request("invalid JSON body");
        }
      };

      if let Some(system) = value.get("system") {
        let system_str = serde_json::to_string(system).unwrap_or_default();
        if let Err(e) = fs::write(saves_dir.join("system.json"), &system_str) {
          warn!("Failed to write system.json in updateall: {:?}", e);
          return error_response("failed to write system save");
        }
      }

      let slot = value
        .get("sessionSlotId")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;

      if slot > 4 {
        return bad_request("invalid sessionSlotId");
      }

      if let Some(session) = value.get("session") {
        if !session.is_null() {
          let session_str = serde_json::to_string(session).unwrap_or_default();
          if let Err(e) = fs::write(
            saves_dir.join(format!("session_{}.json", slot)),
            &session_str,
          ) {
            warn!("Failed to write session_{}.json in updateall: {:?}", slot, e);
            return error_response("failed to write session save");
          }
        }
      }

      ok_empty()
    }

    ("GET", "daily/seed") => {
      let date = Utc::now().format("%Y-%m-%d").to_string();
      HandlerResponse {
        status: 200,
        body: base64_encode(date.as_bytes()),
        content_type: "text/plain",
      }
    }

    ("GET", "game/titlestats") => HandlerResponse {
      status: 200,
      body: r#"{"playerCount":1,"battleCount":0}"#.to_string(),
      content_type: "application/json",
    },

    _ => {
      warn!("Unimplemented API: {} {}", method, path);
      HandlerResponse {
        status: 404,
        body: String::new(),
        content_type: "text/plain",
      }
    }
  }
}

fn ok_empty() -> HandlerResponse {
  HandlerResponse {
    status: 200,
    body: String::new(),
    content_type: "text/plain",
  }
}

fn bad_request(msg: &'static str) -> HandlerResponse {
  HandlerResponse {
    status: 400,
    body: msg.to_string(),
    content_type: "text/plain",
  }
}

fn error_response(msg: &'static str) -> HandlerResponse {
  HandlerResponse {
    status: 500,
    body: msg.to_string(),
    content_type: "text/plain",
  }
}

fn parse_slot(params: &HashMap<String, String>) -> Option<u8> {
  let slot: u8 = params.get("slot")?.parse().ok()?;
  if slot > 4 {
    return None;
  }
  Some(slot)
}

/// Standard base64 encoding (matches JavaScript's `btoa`)
fn base64_encode(input: &[u8]) -> String {
  const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
  let mut i = 0;
  while i < input.len() {
    let b0 = input[i] as u32;
    let b1 = if i + 1 < input.len() {
      input[i + 1] as u32
    } else {
      0
    };
    let b2 = if i + 2 < input.len() {
      input[i + 2] as u32
    } else {
      0
    };
    result.push(CHARS[((b0 >> 2) & 0x3F) as usize] as char);
    result.push(CHARS[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize] as char);
    result.push(if i + 1 < input.len() {
      CHARS[(((b1 << 2) | (b2 >> 6)) & 0x3F) as usize] as char
    } else {
      '='
    });
    result.push(if i + 2 < input.len() {
      CHARS[(b2 & 0x3F) as usize] as char
    } else {
      '='
    });
    i += 3;
  }
  result
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  fn empty_params() -> HashMap<String, String> {
    HashMap::new()
  }

  fn slot_params(slot: u8) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("slot".to_string(), slot.to_string());
    m
  }

  #[test]
  fn test_base64_encode() {
    // Matches JS: btoa("2026-03-23") === "MjAyNi0wMy0yMw=="
    assert_eq!(base64_encode(b"2026-03-23"), "MjAyNi0wMy0yMw==");
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"Man"), "TWFu");
  }

  #[test]
  fn test_account_info() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request("GET", "account/info", &empty_params(), "", dir.path());
    assert_eq!(r.status, 200);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(v["username"], "Guest");
    assert_eq!(v["lastSessionSlot"], -1);
  }

  #[test]
  fn test_account_login() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request("POST", "account/login", &empty_params(), "", dir.path());
    assert_eq!(r.status, 200);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(v["token"], "offline");
  }

  #[test]
  fn test_account_logout() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request("GET", "account/logout", &empty_params(), "", dir.path());
    assert_eq!(r.status, 200);
    assert!(r.body.is_empty());
  }

  #[test]
  fn test_system_get_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "GET",
      "savedata/system/get",
      &empty_params(),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 404);
  }

  #[test]
  fn test_system_update_and_get() {
    let dir = tempfile::tempdir().unwrap();
    let data = r#"{"dexData":{},"timestamp":12345}"#;
    let w = route_request(
      "POST",
      "savedata/system/update",
      &empty_params(),
      data,
      dir.path(),
    );
    assert_eq!(w.status, 200);

    let r = route_request(
      "GET",
      "savedata/system/get",
      &empty_params(),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.body, data);
  }

  #[test]
  fn test_system_verify_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "GET",
      "savedata/system/verify",
      &empty_params(),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(v["valid"], true);
  }

  #[test]
  fn test_system_verify_with_file() {
    let dir = tempfile::tempdir().unwrap();
    let data = r#"{"dexData":{},"timestamp":999}"#;
    fs::write(dir.path().join("system.json"), data).unwrap();

    let r = route_request(
      "GET",
      "savedata/system/verify",
      &empty_params(),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(v["valid"], true);
    assert_eq!(v["systemData"]["timestamp"], 999);
  }

  #[test]
  fn test_session_get_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "GET",
      "savedata/session/get",
      &slot_params(0),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 404);
  }

  #[test]
  fn test_session_missing_slot_is_400() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "GET",
      "savedata/session/get",
      &empty_params(),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 400);
  }

  #[test]
  fn test_session_invalid_slot_is_400() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "GET",
      "savedata/session/get",
      &slot_params(5),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 400);
  }

  #[test]
  fn test_session_update_and_get() {
    let dir = tempfile::tempdir().unwrap();
    let data = r#"{"party":[],"enemyParty":[],"timestamp":1}"#;
    let w = route_request(
      "POST",
      "savedata/session/update",
      &slot_params(2),
      data,
      dir.path(),
    );
    assert_eq!(w.status, 200);

    let r = route_request(
      "GET",
      "savedata/session/get",
      &slot_params(2),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.body, data);
  }

  #[test]
  fn test_session_delete() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("session_1.json"), "data").unwrap();

    let r = route_request(
      "GET",
      "savedata/session/delete",
      &slot_params(1),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    assert!(!dir.path().join("session_1.json").exists());
  }

  #[test]
  fn test_session_clear() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("session_0.json"), "data").unwrap();

    let r = route_request(
      "POST",
      "savedata/session/clear",
      &slot_params(0),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(v["success"], true);
    assert!(!dir.path().join("session_0.json").exists());
  }

  #[test]
  fn test_newclear() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "GET",
      "savedata/session/newclear",
      &empty_params(),
      "",
      dir.path(),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.body, "true");
  }

  #[test]
  fn test_updateall() {
    let dir = tempfile::tempdir().unwrap();
    let body = r#"{"system":{"dexData":{}},"session":{"party":[]},"sessionSlotId":3,"clientSessionId":"abc"}"#;
    let r = route_request(
      "POST",
      "savedata/updateall",
      &empty_params(),
      body,
      dir.path(),
    );
    assert_eq!(r.status, 200);
    assert!(dir.path().join("system.json").exists());
    assert!(dir.path().join("session_3.json").exists());
  }

  #[test]
  fn test_updateall_null_session() {
    let dir = tempfile::tempdir().unwrap();
    let body =
      r#"{"system":{"dexData":{}},"session":null,"sessionSlotId":0,"clientSessionId":"abc"}"#;
    let r = route_request(
      "POST",
      "savedata/updateall",
      &empty_params(),
      body,
      dir.path(),
    );
    assert_eq!(r.status, 200);
    assert!(dir.path().join("system.json").exists());
    assert!(!dir.path().join("session_0.json").exists());
  }

  #[test]
  fn test_updateall_invalid_body() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request(
      "POST",
      "savedata/updateall",
      &empty_params(),
      "not json",
      dir.path(),
    );
    assert_eq!(r.status, 400);
  }

  #[test]
  fn test_titlestats() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request("GET", "game/titlestats", &empty_params(), "", dir.path());
    assert_eq!(r.status, 200);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert!(v["playerCount"].is_number());
  }

  #[test]
  fn test_unknown_endpoint_is_404() {
    let dir = tempfile::tempdir().unwrap();
    let r = route_request("GET", "whatever/unknown", &empty_params(), "", dir.path());
    assert_eq!(r.status, 404);
  }
}
