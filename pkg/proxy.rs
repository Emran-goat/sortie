use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioIo;
use http_body_util::Full;
use http_body_util::BodyExt;
use tokio::net::TcpListener;

type BackendMap = HashMap<String, Vec<(String, u16)>>;

async fn load_backends(deploy_path: &str) -> BackendMap {
    let state_path = format!("{}/.sortie/state.json", deploy_path);
    let content = match tokio::fs::read_to_string(&state_path).await {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let state: crate::types::HostState = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let mut map: BackendMap = HashMap::new();
    for (name, ep) in &state.services {
        map.insert(name.clone(), ep.hosts.iter().map(|h| (h.clone(), ep.port)).collect());
    }
    map
}

pub async fn run_proxy(deploy_path: &str, port: u16) -> Result<(), String> {
    let backends: Arc<tokio::sync::RwLock<BackendMap>> = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
    let path = deploy_path.to_string();

    let b = backends.clone();
    tokio::spawn(async move {
        loop {
            let map = load_backends(&path).await;
            *b.write().await = map;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    *backends.write().await = load_backends(deploy_path).await;

    let connector = hyper_util::client::legacy::connect::HttpConnector::new();
    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(connector);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.map_err(|e| format!("Can't bind port {}: {}", port, e))?;

    println!("Sortie proxy listening on :{}", port);

    loop {
        let (stream, _) = listener.accept().await.map_err(|e| format!("Accept: {}", e))?;
        let b = backends.clone();
        let cl = client.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| proxy_request(req, b.clone(), cl.clone()));
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, svc)
                .await
            {
                eprintln!("Proxy error: {}", e);
            }
        });
    }
}

async fn proxy_request(
    req: Request<Incoming>,
    backends: Arc<tokio::sync::RwLock<BackendMap>>,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let host = req.headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();

    let backends = backends.read().await;
    let mut backend: Option<(String, u16)> = None;

    for (svc_name, addrs) in backends.iter() {
        if host.eq_ignore_ascii_case(svc_name) || host.starts_with(svc_name) {
            if let Some(addr) = addrs.first() {
                backend = Some(addr.clone());
                break;
            }
        }
    }

    match backend {
        Some((backend_host, backend_port)) => {
            let uri = format!("http://{}:{}{}", backend_host, backend_port, req.uri());
            let (parts, body) = req.into_parts();
            let body_bytes = body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();

            let proxy_req = Request::builder()
                .method(parts.method)
                .uri(&uri)
                .body(Full::new(body_bytes))
                .unwrap();

            match client.request(proxy_req).await {
                Ok(resp) => {
                    let (resp_parts, resp_body) = resp.into_parts();
                    let resp_bytes = resp_body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();
                    let mut response = Response::new(Full::new(resp_bytes));
                    *response.status_mut() = resp_parts.status;
                    for (k, v) in resp_parts.headers.iter() {
                        response.headers_mut().insert(k, v.clone());
                    }
                    Ok(response)
                }
                Err(_) => Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Full::new(Bytes::from("502 Bad Gateway")))
                    .unwrap()),
            }
        }
        None => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from(format!("404 no backend for '{}'", host))))
            .unwrap()),
    }
}
