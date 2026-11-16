//! `fetch` against public test APIs (needs the network; ignored by default):
//!
//! ```text
//! cargo test -p zant --lib -- --ignored tsx::fetch::real_tests
//! ```
//!
//! jsonplaceholder (fake REST data), httpbin.org (echo, status codes,
//! redirects, delays, gzip), postman-echo (echo) and dummyjson (paged
//! products).

use super::tests::wait;
use super::*;
use crate::tsx::json::{JsonPath, JsonValue, read_field};

/// `{ userId, id, title, completed }`, decoded like generated code does.
#[derive(Clone, Debug, Default, PartialEq)]
struct Todo {
    user_id: f64,
    id: f64,
    title: Str,
    completed: bool,
}

impl Json for Todo {
    fn to_json(&self) -> Option<JsonValue> {
        Some(
            super::super::json::object_builder(4)
                .field("userId", &self.user_id)
                .field("id", &self.id)
                .field("title", &self.title)
                .field("completed", &self.completed)
                .build(),
        )
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Ok(Todo {
            user_id: read_field(value, "userId", path)?,
            id: read_field(value, "id", path)?,
            title: read_field(value, "title", path)?,
            completed: read_field(value, "completed", path)?,
        })
    }
}

fn get(url: &str) -> Response {
    wait(&fetch(url, RequestInit::default())).unwrap_or_else(|error| panic!("{url}: {error}"))
}

#[test]
#[ignore]
fn jsonplaceholder_typed_json() {
    let response = get("https://jsonplaceholder.typicode.com/todos/1");
    assert!(response.ok);
    assert!(
        response
            .headers
            .get("content-type")
            .is_some_and(|value| value.contains("application/json"))
    );
    let todo: Todo = wait(&response.json()).unwrap();
    assert_eq!(todo.id, 1.0);
    assert_eq!(todo.user_id, 1.0);
    assert!(!todo.title.is_empty());
    // a list
    let todos: crate::tsx::List<Todo> =
        wait(&get("https://jsonplaceholder.typicode.com/todos?userId=1").json()).unwrap();
    assert_eq!(todos.len(), 20);
    // the wrong shape is a TypeError, bad JSON a SyntaxError
    let wrong =
        wait(&get("https://jsonplaceholder.typicode.com/todos/1").json::<crate::tsx::List<Todo>>())
            .unwrap_err();
    assert!(
        wrong
            .to_string()
            .starts_with("TypeError: JSON does not match"),
        "{wrong}"
    );
    let html = wait(&get("https://httpbin.org/html").json::<Todo>()).unwrap_err();
    assert!(html.to_string().starts_with("SyntaxError"), "{html}");
}

#[test]
#[ignore]
fn post_json_with_headers_is_echoed() {
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json");
    headers.set("X-Zant-Test", "你好");
    let body = r#"{"name":"Zant","tags":["ui","rust"]}"#;
    for url in ["https://httpbin.org/post", "https://postman-echo.com/post"] {
        let response = wait(&fetch(
            url,
            RequestInit {
                method: Str::Static("POST"),
                headers: headers.clone(),
                body: Some(Str::Static(body)),
                signal: None,
            },
        ))
        .unwrap_or_else(|error| panic!("{url}: {error}"));
        assert_eq!(response.status, 200.0, "{url}");
        let echo = wait(&response.text()).unwrap();
        assert!(
            echo.contains(r#""name":"Zant""#) || echo.contains(r#""name": "Zant""#),
            "{url}: {echo}"
        );
        assert!(
            echo.to_ascii_lowercase().contains("x-zant-test"),
            "{url}: {echo}"
        );
    }
    // PUT / DELETE / PATCH
    for method in ["PUT", "DELETE", "PATCH"] {
        let url = format!("https://httpbin.org/{}", method.to_ascii_lowercase());
        let response = wait(&fetch(
            &url,
            RequestInit {
                method: Str::Static(method),
                ..RequestInit::default()
            },
        ))
        .unwrap();
        assert_eq!(response.status, 200.0, "{method}");
    }
}

#[test]
#[ignore]
fn statuses_redirects_gzip_and_utf8() {
    for status in [404.0, 500.0, 201.0] {
        let response = get(&format!("https://httpbin.org/status/{status}"));
        assert_eq!(response.status, status);
        assert_eq!(response.ok, status < 300.0);
    }
    let redirected = get("https://httpbin.org/redirect/3");
    assert!(redirected.redirected);
    assert!(redirected.url.ends_with("/get"), "{}", redirected.url);
    let gzip = wait(&get("https://httpbin.org/gzip").text()).unwrap();
    assert!(gzip.contains(r#""gzipped": true"#), "{gzip}");
    let utf8 = wait(&get("https://httpbin.org/encoding/utf8").text()).unwrap();
    assert!(utf8.contains("∮ E⋅da = Q"), "decoded as UTF-8");
    let products = wait(&get("https://dummyjson.com/products?limit=5&skip=10").text()).unwrap();
    assert!(
        products.contains(r#""limit":5"#) && products.contains(r#""skip":10"#),
        "{products}"
    );
}

#[test]
#[ignore]
fn timeout_and_abort_on_a_slow_endpoint() {
    let error = wait(&fetch(
        "https://httpbin.org/delay/5",
        RequestInit {
            signal: Some(AbortSignal::timeout(1000.0)),
            ..RequestInit::default()
        },
    ))
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "TimeoutError: The operation was aborted due to timeout"
    );
    let controller = AbortController::new();
    let pending = fetch(
        "https://httpbin.org/delay/5",
        RequestInit {
            signal: Some(controller.signal.clone()),
            ..RequestInit::default()
        },
    );
    controller.abort();
    let error = wait(&pending).unwrap_err();
    assert_eq!(error.to_string(), "AbortError: This operation was aborted");
}
