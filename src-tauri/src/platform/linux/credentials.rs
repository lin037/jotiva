use std::{
    io::Write,
    process::{Command, Stdio},
};
fn command(action: &str, service: &str, url: &str) -> Command {
    let mut c = Command::new("secret-tool");
    c.arg(action);
    if action == "store" {
        c.arg(if service.ends_with(".images") {
            "--label=Jotiva Images"
        } else {
            "--label=Jotiva Jev"
        });
    }
    c.args(["application", service, "endpoint", url]);
    c
}
pub fn read(service: &str, url: &str) -> Result<Option<String>, String> {
    let out = command("lookup", service, url)
        .output()
        .map_err(|e| format!("无法调用 Secret Service 的 secret-tool：{e}"))?;
    if out.status.success() {
        return String::from_utf8(out.stdout)
            .map(|s| Some(s.trim_end().to_owned()))
            .map_err(|e| e.to_string());
    }
    if out.status.code() == Some(1) && out.stderr.is_empty() {
        Ok(None)
    } else {
        Err("无法读取系统密钥环".into())
    }
}
pub fn remove(service: &str, url: &str) -> Result<(), String> {
    if read(service, url)?.is_none() {
        return Ok(());
    }
    let status = command("clear", service, url)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("无法移除密钥".into())
    }
}
pub fn prompt(service: &str, url: &str) -> Result<(), String> {
    let out = Command::new("zenity")
        .args(["--password", "--title", &format!("Jotiva · {url}")])
        .output()
        .map_err(|e| format!("无法打开 zenity 密码输入框：{e}"))?;
    if !out.status.success() {
        return Err("已取消配置密钥".into());
    }
    let raw = String::from_utf8(out.stdout).map_err(|e| e.to_string())?;
    let key = raw.trim_end_matches('\n');
    if !crate::platform::filing::valid_key(key) {
        return Err("请输入有效的 API 密钥".into());
    }
    let mut child = command("store", service, url)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(key.as_bytes())
        .map_err(|e| e.to_string())?;
    if child.wait().map_err(|e| e.to_string())?.success() {
        Ok(())
    } else {
        Err("系统密钥环未能保存密钥".into())
    }
}
