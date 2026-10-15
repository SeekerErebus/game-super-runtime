use crate::{
    config::{self, Config, MAX_CONFIG_BYTES},
    editor, execute, storage,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    thread,
    time::{Duration, Instant},
};

const MAX_REQUEST: usize = MAX_CONFIG_BYTES * 4;

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}
fn request(stream: &mut TcpStream) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut data = vec![];
    let mut buf = [0u8; 4096];
    let end = loop {
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        ensure!(data.len() < 16384, "request headers too large");
        let n = stream.read(&mut buf)?;
        ensure!(n > 0, "incomplete request");
        data.extend_from_slice(&buf[..n]);
    };
    let head = std::str::from_utf8(&data[..end])?;
    let mut lines = head.split("\r\n");
    let parts: Vec<_> = lines
        .next()
        .context("missing request line")?
        .split_whitespace()
        .collect();
    ensure!(
        parts.len() == 3 && parts[2] == "HTTP/1.1",
        "expected HTTP/1.1 request"
    );
    let method = parts[0].to_owned();
    let path = parts[1].to_owned();
    let mut headers = vec![];
    for line in lines.filter(|l| !l.is_empty()) {
        let (name, value) = line.split_once(':').context("invalid header")?;
        let name = name.to_ascii_lowercase();
        ensure!(
            !headers.iter().any(|(k, _)| k == &name),
            "duplicate HTTP header"
        );
        headers.push((name, value.trim().to_owned()));
    }
    ensure!(
        !headers.iter().any(|(k, _)| k == "transfer-encoding"),
        "chunked requests are not supported"
    );
    let length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .map(|(_, v)| v.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    ensure!(length <= MAX_REQUEST, "request is too large");
    while data.len() - end < length {
        let n = stream.read(&mut buf)?;
        ensure!(n > 0, "incomplete body");
        data.extend_from_slice(&buf[..n]);
    }
    Ok(Request {
        method,
        path,
        headers,
        body: data[end..end + length].to_vec(),
    })
}
fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    token: &str,
) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; script-src 'nonce-{token}'; style-src 'nonce-{token}'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}
fn payload(path: &Path, title: &str) -> Result<Value> {
    let yaml = storage::read_text(path)?;
    let (config, error) = match config::parse(&yaml) {
        Ok(c) => (serde_json::to_value(c)?, Value::Null),
        Err(e) => (Value::Null, json!(format!("{e:#}"))),
    };
    Ok(json!({"title":title,"path":path,"yaml":yaml,"config":config,"error":error}))
}
fn api(route: &str, body: &[u8], path: &Path, title: &str) -> Result<Value> {
    let value: Value = serde_json::from_slice(body).context("invalid JSON request")?;
    match route {
        "validate" => {
            let yaml = value
                .get("yaml")
                .and_then(Value::as_str)
                .context("yaml is required")?;
            let config = config::parse(yaml)?;
            Ok(
                json!({"config":config,"message":"Configuration is valid. No programs were launched."}),
            )
        }
        "format" => {
            let config: Config =
                serde_json::from_value(value.get("config").cloned().context("config is required")?)
                    .context("invalid form values")?;
            config.validate()?;
            let yaml = serde_yaml_ng::to_string(&config)?;
            config::parse(&yaml)?;
            Ok(json!({"yaml":yaml,"config":config}))
        }
        "save" => {
            let yaml = value
                .get("yaml")
                .and_then(Value::as_str)
                .context("yaml is required")?;
            let original = value
                .get("original")
                .and_then(Value::as_str)
                .context("original is required for conflict detection")?;
            storage::save_config(path, yaml, original)?;
            payload(path, title)
        }
        _ => bail!("unknown action"),
    }
}

pub fn serve(path: &Path, title: &str, no_open: bool) -> Result<i32> {
    let listener = TcpListener::bind("127.0.0.1:0").context("cannot start local editor")?;
    listener.set_nonblocking(true)?;
    let authority = listener.local_addr()?.to_string();
    let mut random = [0u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let base = format!("/{token}/");
    let origin = format!("http://{authority}");
    let url = format!("{origin}{base}");
    println!(
        "Game Super Runtime editor: {url}\nEditing: {}\nUse Close editor to stop the server, or press Ctrl+C. Idle timeout: 30 minutes.",
        path.display()
    );
    std::io::stdout().flush()?;
    if !no_open {
        if let Err(e) = editor::open_browser(&url) {
            eprintln!("game-super-runtime: {e:#}");
        }
    }
    let mut active = Instant::now();
    while execute::received_signal() == 0 && active.elapsed() < Duration::from_secs(1800) {
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let req = match request(&mut stream) {
            Ok(r) => r,
            Err(_) => {
                let _ = respond(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    b"Invalid request",
                    &token,
                );
                continue;
            }
        };
        // Host checking blocks DNS rebinding; an unguessable path and custom header
        // block cross-origin writes even when Origin is omitted by a client.
        if req.header("host") != Some(authority.as_str())
            || req.header("origin").is_some_and(|s| s != origin)
            || !req.path.starts_with(&base)
        {
            let _ = respond(
                &mut stream,
                "403 Forbidden",
                "text/plain",
                b"Forbidden",
                &token,
            );
            continue;
        }
        active = Instant::now();
        let route = &req.path[base.len()..];
        if req.method == "GET" && route.is_empty() {
            let html = include_str!("../assets/editor.html").replace("__GSR_NONCE__", &token);
            let _ = respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                html.as_bytes(),
                &token,
            );
            continue;
        }
        let result = if req.method == "GET" && route == "config" {
            payload(path, title)
        } else if req.method == "POST"
            && req.header("x-gsr-token") == Some(token.as_str())
            && req
                .header("content-type")
                .is_some_and(|s| s.starts_with("application/json"))
        {
            if route == "close" {
                let _ = respond(&mut stream, "200 OK", "application/json", b"{}", &token);
                return Ok(0);
            }
            api(route, &req.body, path, title)
        } else {
            bail_response(&mut stream, &token);
            continue;
        };
        let (status, value) = match result {
            Ok(v) => ("200 OK", v),
            Err(e) => ("400 Bad Request", json!({"error":format!("{e:#}")})),
        };
        let _ = respond(
            &mut stream,
            status,
            "application/json",
            serde_json::to_string(&value)?.as_bytes(),
            &token,
        );
    }
    Ok(if execute::received_signal() == 0 {
        0
    } else {
        128 + execute::received_signal()
    })
}
fn bail_response(stream: &mut TcpStream, token: &str) {
    let _ = respond(
        stream,
        "403 Forbidden",
        "application/json",
        br#"{"error":"Unsupported or unauthenticated request"}"#,
        token,
    );
}
