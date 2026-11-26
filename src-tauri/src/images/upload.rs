use super::Config;
use reqwest::blocking::{Client, multipart};
use std::{io::Read, time::Duration};

use crate::settings::filing::http_endpoint;

pub(super) fn credential(service: &str, address: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let token = crate::platform::filing::read_token(service, address)?;
    #[cfg(not(target_os = "macos"))]
    let token = crate::platform::credentials::read(service, address)?;
    token.ok_or_else(|| "请先为此上传地址设置密钥".into())
}

pub(super) fn send(
    config: &Config,
    name: &str,
    mime: &str,
    bytes: Vec<u8>,
    key: Option<&str>,
) -> Result<String, String> {
    let url = http_endpoint(&config.endpoint)?;
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none());
    if matches!(url.host_str(), Some("127.0.0.1" | "[::1]")) {
        builder = builder.no_proxy();
    }
    let part = multipart::Part::bytes(bytes)
        .file_name(name.to_owned())
        .mime_str(mime)
        .map_err(|e| e.to_string())?;
    let form = multipart::Form::new().part(config.field.clone(), part);
    let mut request = builder
        .build()
        .map_err(|e| e.to_string())?
        .post(url)
        .multipart(form);
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let reply = request.send().map_err(|e| e.without_url().to_string())?;
    if !reply.status().is_success() {
        return Err(format!(
            "{} (HTTP {})",
            crate::i18n::t("图片上传失败"),
            reply.status().as_u16()
        ));
    }
    const MAX_REPLY: usize = 64 * 1024;
    let mut body = Vec::new();
    reply
        .take(MAX_REPLY as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    if body.len() > MAX_REPLY {
        return Err("上传响应过大".into());
    }
    let json: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| "上传服务未返回 JSON")?;
    let link = json
        .pointer(&config.response_pointer)
        .and_then(|v| v.as_str())
        .ok_or("响应中找不到图片地址")?;
    let link = tauri::Url::parse(link).map_err(|_| "无效的图片地址")?;
    if !matches!(link.scheme(), "http" | "https")
        || !link.has_host()
        || !link.username().is_empty()
        || link.password().is_some()
    {
        return Err("上传服务须返回 HTTP 或 HTTPS 图片地址".into());
    }
    Ok(link.into())
}
