//! A throwaway HTTP server on 127.0.0.1. No test in this crate ever reaches the
//! real GitHub or GameBanana.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tiny_http::{Header, Response, Server};

pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

impl Reply {
    pub fn ok(body: impl Into<Vec<u8>>) -> Reply {
        Reply {
            status: 200,
            body: body.into(),
            headers: Vec::new(),
        }
    }

    pub fn status(status: u16) -> Reply {
        Reply {
            status,
            body: Vec::new(),
            headers: Vec::new(),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Reply {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

/// `(path, hit)` in, one reply out. `hit` counts from 1 for each path.
pub type Handler = Arc<dyn Fn(&str, usize) -> Reply + Send + Sync>;

pub struct TestServer {
    pub base: String,
    server: Arc<Server>,
    hits: Arc<Mutex<HashMap<String, usize>>>,
    worker: Option<JoinHandle<()>>,
    stopped: Arc<AtomicBool>,
}

impl TestServer {
    pub fn start(handler: Handler) -> TestServer {
        let server = Arc::new(Server::http("127.0.0.1:0").expect("bind loopback"));
        let base = format!("http://{}", server.server_addr());
        let hits: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        let worker = {
            let server = Arc::clone(&server);
            let hits = Arc::clone(&hits);
            let stopped = Arc::clone(&stopped);
            std::thread::spawn(move || {
                while let Ok(request) = server.recv() {
                    if stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    let path = request.url().to_string();
                    let hit = {
                        let mut hits = hits.lock().unwrap();
                        let slot = hits.entry(path.clone()).or_insert(0);
                        *slot += 1;
                        *slot
                    };
                    let reply = handler(&path, hit);
                    let headers: Vec<Header> = reply
                        .headers
                        .iter()
                        .filter_map(|(name, value)| {
                            Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
                        })
                        .collect();
                    let length = reply.body.len();
                    let response = Response::new(
                        tiny_http::StatusCode(reply.status),
                        headers,
                        Cursor::new(reply.body),
                        Some(length),
                        None,
                    );
                    let _ = request.respond(response);
                }
            })
        };

        TestServer {
            base,
            server,
            hits,
            worker: Some(worker),
            stopped,
        }
    }

    /// Serves a fixed map of path to body, 404 for anything else.
    pub fn fixtures(routes: Vec<(String, String)>) -> TestServer {
        let table: HashMap<String, String> = routes.into_iter().collect();
        TestServer::start(Arc::new(move |path: &str, _hit: usize| {
            match table.get(path) {
                Some(body) => Reply::ok(body.clone()),
                None => Reply::status(404),
            }
        }))
    }

    /// Like `start`, but the handler is also told the server's own base URL so
    /// a fixture can name a download that points back at itself.
    pub fn start_based(
        handler: impl Fn(&str, usize, &str) -> Reply + Send + Sync + 'static,
    ) -> TestServer {
        let base: Arc<std::sync::OnceLock<String>> = Arc::new(std::sync::OnceLock::new());
        let seen = Arc::clone(&base);
        let server = TestServer::start(Arc::new(move |path: &str, hit: usize| {
            handler(path, hit, seen.get().map(String::as_str).unwrap_or(""))
        }));
        let _ = base.set(server.base.clone());
        server
    }

    pub fn hits(&self, path: &str) -> usize {
        self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.server.unblock();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// A client wired to this server, with every wait scaled down.
pub fn client(server: &TestServer) -> resolve::Client {
    let mut client = resolve::Client::new(Some("test-token"));
    client.set_api_base(&server.base);
    client.set_gamebanana_base(&server.base);
    client.set_wait_scale(0.0);
    client
}
