use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::*;
use crate::tsx::task;
use crate::ui::ActionCx;

/// A local HTTP/1.1 server answering by path (one thread, `Connection: close`).
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut buffer).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    request.extend_from_slice(&buffer[..n]);
                }
                let text = String::from_utf8_lossy(&request).to_string();
                let line = text.lines().next().unwrap_or("").to_owned();
                let path = line.split(' ').nth(1).unwrap_or("/").to_owned();
                let header = |name: &str| {
                    text.lines()
                        .find(|l| l.to_ascii_lowercase().starts_with(&format!("{name}:")))
                        .map(|l| l.split_once(':').unwrap().1.trim().to_owned())
                        .unwrap_or_default()
                };
                let (status, extra, body) = match path.as_str() {
                    "/text" => ("200 OK", String::new(), "hello 世界".to_owned()),
                    "/json" => ("200 OK", String::new(), r#"{"a":1,"b":[1,2]}"#.to_owned()),
                    "/missing" => ("404 Not Found", String::new(), "nope".to_owned()),
                    "/headers" => (
                        "200 OK",
                        "X-Many: a\r\nX-Many: b\r\n".to_owned(),
                        format!(
                            "{} {}",
                            line.split(' ').next().unwrap_or(""),
                            header("x-test")
                        ),
                    ),
                    "/redirect" => ("302 Found", "Location: /text\r\n".to_owned(), String::new()),
                    "/slow" => {
                        std::thread::sleep(Duration::from_secs(3));
                        ("200 OK", String::new(), "late".to_owned())
                    }
                    _ => ("404 Not Found", String::new(), String::new()),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    base
}

/// Runs the UI-thread queues until `done` (or 10 s).
pub(super) fn wait<T: Clone + 'static>(promise: &Promise<T>) -> Result<T, Thrown> {
    let slot: Rc<RefCell<Option<Result<T, Thrown>>>> = Rc::new(RefCell::new(None));
    let (a, b) = (slot.clone(), slot.clone());
    promise
        .then(move |_, value| {
            *a.borrow_mut() = Some(Ok(value));
            Ok(())
        })
        .catch(move |_, error| {
            *b.borrow_mut() = Some(Err(error));
            Ok(())
        });
    let start = Instant::now();
    loop {
        let mut root = ();
        task::run(&mut ActionCx::new(&mut root));
        if let Some(result) = slot.borrow_mut().take() {
            return result;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn message(error: Thrown) -> String {
    error.to_string()
}

#[test]
fn the_test_server_answers() {
    let base = serve();
    let address = base.trim_start_matches("http://");
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .write_all(b"GET /text HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 200 OK"), "{answer:?}");
}

#[test]
fn text_status_headers_and_json() {
    let base = serve();
    let response = wait(&fetch(&format!("{base}/text"), RequestInit::default())).unwrap();
    assert!(response.ok);
    assert_eq!(response.status, 200.0);
    assert_eq!(response.status_text.as_str(), "OK");
    assert_eq!(
        response.headers.get("CONTENT-TYPE").as_deref(),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(wait(&response.text()).unwrap().as_str(), "hello 世界");
    // the body is read once
    assert!(response.body_used());
    let again = wait(&response.text()).unwrap_err();
    assert_eq!(
        message(again),
        "TypeError: Body is unusable: Body has already been read"
    );

    let missing = wait(&fetch(&format!("{base}/missing"), RequestInit::default())).unwrap();
    assert!(!missing.ok, "HTTP errors resolve with ok = false");
    assert_eq!(missing.status, 404.0);

    let json = wait(&fetch(&format!("{base}/json"), RequestInit::default())).unwrap();
    assert_eq!(wait(&json.text()).unwrap().as_str(), r#"{"a":1,"b":[1,2]}"#);
}

#[test]
fn methods_request_headers_redirects_and_repeated_headers() {
    let base = serve();
    let mut headers = Headers::new();
    headers.set("X-Test", "  sent  ");
    let response = wait(&fetch(
        &format!("{base}/headers"),
        RequestInit {
            method: Str::Static("post"),
            headers,
            body: Some(Str::Static("payload")),
            signal: None,
        },
    ))
    .unwrap();
    assert_eq!(wait(&response.text()).unwrap().as_str(), "POST sent");
    assert_eq!(response.headers.get("x-many").as_deref(), Some("a, b"));
    let redirected = wait(&fetch(&format!("{base}/redirect"), RequestInit::default())).unwrap();
    assert!(redirected.redirected);
    assert!(redirected.url.ends_with("/text"));
    assert_eq!(wait(&redirected.text()).unwrap().as_str(), "hello 世界");
}

#[test]
fn failures_reject_like_node() {
    let error = wait(&fetch("not a url", RequestInit::default())).unwrap_err();
    assert_eq!(
        message(error),
        "TypeError: Failed to parse URL from not a url"
    );
    // nothing listens on this port
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let error = wait(&fetch(
        &format!("http://127.0.0.1:{port}/"),
        RequestInit::default(),
    ))
    .unwrap_err();
    assert!(message(error).starts_with("TypeError: fetch failed"));
}

#[test]
fn abort_and_timeout() {
    let base = serve();
    let controller = AbortController::new();
    let pending = fetch(
        &format!("{base}/slow"),
        RequestInit {
            signal: Some(controller.signal.clone()),
            ..RequestInit::default()
        },
    );
    controller.abort();
    let error = wait(&pending).unwrap_err();
    assert_eq!(message(error), "AbortError: This operation was aborted");
    let error = wait(&fetch(
        &format!("{base}/slow"),
        RequestInit {
            signal: Some(AbortSignal::timeout(100.0)),
            ..RequestInit::default()
        },
    ))
    .unwrap_err();
    assert_eq!(
        message(error),
        "TimeoutError: The operation was aborted due to timeout"
    );
}

/// Real HTTPS through the platform TLS stack and the system proxy (needs
/// the network): `cargo test -p zant -- --ignored real_https`.
#[test]
#[ignore]
fn real_https() {
    let response = wait(&fetch("https://example.com/", RequestInit::default())).unwrap();
    assert!(response.ok, "status {}", response.status);
    let text = wait(&response.text()).unwrap();
    assert!(text.contains("Example Domain"), "{text}");
}
