use crate::platform::credentials;
use crate::settings::filing::KeyAction;
use serde_json::Value;
use std::{io::Read, time::Duration};
pub fn credential(service: &str, url: &str, action: KeyAction) -> Result<bool, String> {
    match action {
        KeyAction::Check => credentials::read(service, url).map(|v| v.is_some()),
        KeyAction::Set => credentials::prompt(service, url).map(|()| true),
        KeyAction::Remove => credentials::remove(service, url).map(|()| false),
    }
}
pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| b.is_ascii_graphic())
}
pub fn post(service: &str, url: &str, payload: &Value) -> (i64, Vec<u8>) {
    match credentials::read(service, url) {
        Ok(Some(key)) => send(url, &key, payload),
        _ => (-1, Vec::new()),
    }
}
pub fn send(url: &str, key: &str, payload: &Value) -> (i64, Vec<u8>) {
    let Ok(url) = crate::settings::filing::endpoint(url) else {
        return (-4, vec![]);
    };
    if !valid_key(key) {
        return (-3, vec![]);
    }
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none());
    if matches!(url.host_str(), Some("127.0.0.1" | "[::1]")) {
        builder = builder.no_proxy()
    }
    let response = builder.build().and_then(|c| {
        c.post(url)
            .bearer_auth(key)
            .header("Content-Type", "application/json")
            .body(payload.to_string())
            .send()
    });
    let response = match response {
        Ok(r) => r,
        Err(e) if e.is_timeout() => return (-2, vec![]),
        Err(e) if e.is_connect() => return (0, vec![]),
        Err(_) => return (-4, vec![]),
    };
    let status = i64::from(response.status().as_u16());
    if status != 200 {
        return (status, vec![]);
    }
    let mut body = Vec::new();
    if response.take(131073).read_to_end(&mut body).is_err()
        || body.len() > 131072
        || body.contains(&0)
    {
        return (-3, vec![]);
    }
    (status, body)
}
#[cfg(test)]
pub use send as post_fixture;
