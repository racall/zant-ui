//! `fetch` (WHATWG Fetch) for generated TSX, on `reqwest`.
//!
//! Requests run on a framework-owned thread with a single-threaded tokio
//! runtime (started by the first request) and one shared `reqwest::Client`
//! (connection pool, platform TLS and system proxy settings, gzip, at most
//! 20 redirects like the Fetch standard). Results come back to the UI
//! thread as promises ([`super::promise`]).
//!
//! JavaScript's behavior, as Node / browsers have it:
//! - `fetch` resolves when the response headers arrive; HTTP errors
//!   (404, 500) resolve too, with `ok === false`;
//! - a network failure rejects with `TypeError: fetch failed` (the cause is
//!   appended, which Node keeps in `error.cause`); a bad URL with
//!   `TypeError: Failed to parse URL from …`;
//! - `AbortController.abort()` rejects with `AbortError: This operation was
//!   aborted`, `AbortSignal.timeout(ms)` with `TimeoutError`;
//! - the body is read once (`text()` / `json()`); a second read rejects
//!   with `TypeError: Body is unusable: Body has already been read`;
//! - `headers.get(name)` is case-insensitive and joins repeated headers
//!   with ", ".

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use super::error::{Error, Thrown};
use super::json::Json;
use super::promise::Promise;
use crate::Str;

/// The HTTP thread's runtime, the client using the system proxy and the
/// client for local addresses (no proxy).
struct Http {
    handle: tokio::runtime::Handle,
    client: reqwest::Client,
    direct: reqwest::Client,
}

/// Local hosts never go through the system proxy: loopback, `localhost`,
/// `*.local`, private and link-local networks (the platforms' default
/// proxy exceptions, which reqwest's system proxy lookup does not apply —
/// a local dev server would otherwise be sent to a proxy app).
fn is_local(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    match bare.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(std::net::IpAddr::V6(ip)) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
        Err(_) => {
            let name = bare.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local")
        }
    }
}

fn http() -> &'static Http {
    static HTTP: OnceLock<Http> = OnceLock::new();
    HTTP.get_or_init(|| {
        let (handle, client) = runtime_and_client();
        let direct = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(20))
            .no_proxy()
            .build()
            .expect("the HTTP client builds");
        Http {
            handle,
            client,
            direct,
        }
    })
}

/// The HTTP thread's runtime and client.
fn runtime_and_client() -> (tokio::runtime::Handle, reqwest::Client) {
    static RUNTIME: OnceLock<(tokio::runtime::Handle, reqwest::Client)> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("zant-fetch".into())
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("the fetch runtime starts");
                    sender
                        .send(runtime.handle().clone())
                        .expect("the UI thread waits for the runtime");
                    runtime.block_on(std::future::pending::<()>());
                })
                .expect("the fetch thread starts");
            let handle = receiver.recv().expect("the fetch runtime started");
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(20))
                .build()
                .expect("the HTTP client builds");
            (handle, client)
        })
        .clone()
}

/// An error with its causes (`reqwest`'s own message is only the outer
/// layer: "error sending request").
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn type_error(message: String) -> Thrown {
    Thrown::error(Error::new("TypeError", Str::from(message)))
}

/// Request and response headers: names are case-insensitive (kept in
/// lowercase, like `Headers` does); iteration is sorted by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Headers(Vec<(Str, Str)>);

impl Headers {
    pub fn new() -> Headers {
        Headers::default()
    }

    /// `headers.get(name)`: every value of that name joined with ", ".
    pub fn get(&self, name: &str) -> Option<Str> {
        let name = name.to_ascii_lowercase();
        let values: Vec<&str> = self
            .0
            .iter()
            .filter(|(key, _)| key.as_str() == name)
            .map(|(_, value)| value.as_str())
            .collect();
        (!values.is_empty()).then(|| Str::from(values.join(", ")))
    }

    pub fn has(&self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        self.0.iter().any(|(key, _)| key.as_str() == name)
    }

    pub fn append(&mut self, name: &str, value: &str) {
        self.0.push((
            Str::from(name.to_ascii_lowercase()),
            Str::from(value.trim().to_owned()),
        ));
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.delete(name);
        self.append(name, value);
    }

    pub fn delete(&mut self, name: &str) {
        let name = name.to_ascii_lowercase();
        self.0.retain(|(key, _)| key.as_str() != name);
    }

    /// `[...headers]` / `headers.forEach`: `[name, value]` sorted by name,
    /// repeated names combined (`set-cookie` excepted, as in the standard).
    pub fn entries(&self) -> Vec<(Str, Str)> {
        let mut names: Vec<&Str> = self.0.iter().map(|(key, _)| key).collect();
        names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        names.dedup();
        let mut out = Vec::new();
        for name in names {
            if name.as_str() == "set-cookie" {
                for (key, value) in &self.0 {
                    if key == name {
                        out.push((key.clone(), value.clone()));
                    }
                }
            } else if let Some(value) = self.get(name) {
                out.push((name.clone(), value));
            }
        }
        out
    }

    fn from_reqwest(map: &reqwest::header::HeaderMap) -> Headers {
        let mut headers = Headers::new();
        for (name, value) in map {
            headers.append(name.as_str(), &String::from_utf8_lossy(value.as_bytes()));
        }
        headers
    }
}

/// `AbortController` / `AbortSignal` (a signal shares the controller's
/// state; `AbortSignal.timeout(ms)` aborts by itself).
#[derive(Clone, Debug, Default)]
pub struct AbortSignal(Rc<RefCell<SignalState>>);

#[derive(Debug, Default)]
struct SignalState {
    reason: Option<Thrown>,
    timeout_ms: Option<f64>,
    /// Abort the request in flight (set by `fetch`).
    abort_request: Vec<tokio::task::AbortHandle>,
}

impl PartialEq for AbortSignal {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl AbortSignal {
    /// `AbortSignal.timeout(ms)`: aborts with a `TimeoutError` after `ms`.
    pub fn timeout(ms: f64) -> AbortSignal {
        let signal = AbortSignal::default();
        signal.0.borrow_mut().timeout_ms = Some(ms.max(0.0));
        signal
    }

    pub fn aborted(&self) -> bool {
        self.0.borrow().reason.is_some()
    }

    fn abort(&self, reason: Thrown) {
        let handles = {
            let mut state = self.0.borrow_mut();
            if state.reason.is_some() {
                return;
            }
            state.reason = Some(reason);
            std::mem::take(&mut state.abort_request)
        };
        for handle in handles {
            handle.abort();
        }
    }
}

/// `new AbortController()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AbortController {
    pub signal: AbortSignal,
}

impl AbortController {
    pub fn new() -> AbortController {
        AbortController::default()
    }

    /// `controller.abort()`.
    pub fn abort(&self) {
        self.signal.abort(Thrown::error(Error::new(
            "AbortError",
            Str::Static("This operation was aborted"),
        )));
    }
}

/// `fetch(url, init)` options.
#[derive(Clone, Debug, Default)]
pub struct RequestInit {
    /// `GET` when empty; standard method names are upper-cased.
    pub method: Str,
    pub headers: Headers,
    pub body: Option<Str>,
    pub signal: Option<AbortSignal>,
}

/// A response: its head is known, the body is read once.
#[derive(Clone)]
pub struct Response {
    pub status: f64,
    pub status_text: Str,
    pub ok: bool,
    pub url: Str,
    pub redirected: bool,
    pub headers: Headers,
    body: Rc<RefCell<Option<reqwest::Response>>>,
    signal: Option<AbortSignal>,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Response {{ status: {}, url: {:?} }}",
            self.status, self.url
        )
    }
}

impl PartialEq for Response {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.body, &other.body)
    }
}

impl Default for Response {
    fn default() -> Self {
        Response {
            status: 0.0,
            status_text: Str::Static(""),
            ok: false,
            url: Str::Static(""),
            redirected: false,
            headers: Headers::new(),
            body: Rc::new(RefCell::new(None)),
            signal: None,
        }
    }
}

impl Response {
    /// `response.bodyUsed`.
    pub fn body_used(&self) -> bool {
        self.body.borrow().is_none()
    }

    /// `response.text()` (UTF-8, invalid sequences replaced).
    pub fn text(&self) -> Promise<Str> {
        let Some(body) = self.body.borrow_mut().take() else {
            return Promise::reject(type_error(
                "Body is unusable: Body has already been read".into(),
            ));
        };
        let signal = self.signal.clone();
        run_on_http_thread(signal, async move {
            body.bytes()
                .await
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .map_err(|error| describe(&error))
        })
        .then(|_, text| Ok(Str::from(text.borrow_mut().take().unwrap_or_default())))
    }

    /// `response.json()`, decoded into `T` (`SyntaxError` for bad JSON).
    pub fn json<T: Json + Clone + 'static>(&self) -> Promise<T> {
        self.text()
            .then(|_, text| super::json::parse_as::<T>(text.as_str()))
    }
}

/// `fetch(url, init)`.
pub fn fetch(url: &str, init: RequestInit) -> Promise<Response> {
    let parsed = match reqwest::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return Promise::reject(type_error(format!("Failed to parse URL from {url}"))),
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Promise::reject(type_error(format!(
            "Request cannot be constructed from a URL that includes credentials: {url}"
        )));
    }
    let method = match init.method.to_ascii_uppercase().as_str() {
        "" => reqwest::Method::GET,
        // the standard upper-cases these; any other name is sent as written
        upper @ ("DELETE" | "GET" | "HEAD" | "OPTIONS" | "POST" | "PUT") => {
            reqwest::Method::from_bytes(upper.as_bytes()).expect("a standard method")
        }
        _ => match reqwest::Method::from_bytes(init.method.as_bytes()) {
            Ok(method) => method,
            Err(_) => {
                return Promise::reject(type_error(format!(
                    "'{}' is not a valid HTTP method.",
                    init.method
                )));
            }
        },
    };
    let http = http();
    let client = if is_local(&parsed) {
        &http.direct
    } else {
        &http.client
    };
    let mut request = client.request(method, parsed.clone());
    for (name, value) in &init.headers.0 {
        request = request.header(name.as_str(), value.as_str());
    }
    if let Some(body) = &init.body {
        request = request.body(body.to_string());
    }
    let signal = init.signal.clone();
    let requested = parsed.to_string();
    run_on_http_thread(signal.clone(), async move {
        request.send().await.map_err(|error| describe(&error))
    })
    .then(move |_, response: Rc<RefCell<Option<reqwest::Response>>>| {
        let taken = response.borrow_mut().take();
        let Some(response) = taken else {
            return Err(type_error("fetch failed".into()));
        };
        let status = response.status();
        let url = response.url().to_string();
        Ok(Response {
            status: f64::from(status.as_u16()),
            status_text: Str::from(status.canonical_reason().unwrap_or("").to_owned()),
            ok: status.is_success(),
            redirected: url != requested,
            url: Str::from(url),
            headers: Headers::from_reqwest(response.headers()),
            body: Rc::new(RefCell::new(Some(response))),
            signal,
        })
    })
}

/// Runs `work` on the HTTP thread and settles a UI-thread promise with its
/// result, honoring `signal` (abort / timeout).
fn run_on_http_thread<T: Send + 'static>(
    signal: Option<AbortSignal>,
    work: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
) -> Promise<Rc<RefCell<Option<T>>>> {
    if let Some(reason) = signal
        .as_ref()
        .and_then(|signal| signal.0.borrow().reason.clone())
    {
        return Promise::reject(reason);
    }
    let handle = &http().handle;
    let timeout = signal
        .as_ref()
        .and_then(|signal| signal.0.borrow().timeout_ms);
    let (sender, receiver) = tokio::sync::oneshot::channel::<Result<T, String>>();
    let job = handle.spawn(async move {
        let result = match timeout {
            Some(ms) => {
                match tokio::time::timeout(Duration::from_secs_f64(ms / 1000.0), work).await {
                    Ok(result) => result,
                    Err(_) => Err("\u{0}timeout".into()),
                }
            }
            None => work.await,
        };
        let _ = sender.send(result);
    });
    if let Some(signal) = &signal {
        signal.0.borrow_mut().abort_request.push(job.abort_handle());
    }
    Promise::from_future(async move {
        let result = receiver.await;
        if let Some(reason) = signal
            .as_ref()
            .and_then(|signal| signal.0.borrow().reason.clone())
        {
            return Err(reason);
        }
        match result {
            Ok(Ok(value)) => Ok(Rc::new(RefCell::new(Some(value)))),
            Ok(Err(cause)) if cause == "\u{0}timeout" => {
                let error = Thrown::error(Error::new(
                    "TimeoutError",
                    Str::Static("The operation was aborted due to timeout"),
                ));
                if let Some(signal) = &signal {
                    signal.0.borrow_mut().reason = Some(error.clone());
                }
                Err(error)
            }
            Ok(Err(cause)) => Err(type_error(format!("fetch failed ({cause})"))),
            // the job was aborted (the signal holds the reason) or dropped
            Err(_) => Err(type_error("fetch failed".into())),
        }
    })
}

#[cfg(test)]
mod real_tests;
#[cfg(test)]
mod tests;
