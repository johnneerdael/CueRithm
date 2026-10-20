//! Real loopback HTTP servers exercise the shared transport, without live providers.
use onetagger_selected_providers::http::{CatalogHttp, MemoryChallengeStore, SolverConfig};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

fn server(
    responses: Vec<(u16, &'static str, String)>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let mut requests = vec![];
        for (status, headers, body) in responses {
            let started = std::time::Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && started.elapsed() < Duration::from_secs(5) =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("missing expected request: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = vec![];
            loop {
                let mut bytes = [0; 8192];
                let n = stream.read(&mut bytes).unwrap();
                request.extend_from_slice(&bytes[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some((head, payload)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if payload.len() >= length {
                        break;
                    }
                }
                assert!(n > 0);
            }
            requests.push(String::from_utf8(request).unwrap());
            stream.write_all(format!("HTTP/1.1 {status} Fixture\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
        }
        requests
    });
    (url, task)
}
fn transport() -> CatalogHttp {
    CatalogHttp::new(
        reqwest::blocking::Client::new(),
        "fixture",
        Arc::new(MemoryChallengeStore::default()),
    )
}
fn solved(url: &str, body: &str) -> String {
    serde_json::json!({"status":"ok","solution":{"url":url,"status":200,"headers":{},"response":body}}).to_string()
}
#[test]
fn challenge_is_remembered_and_next_lookup_does_not_contact_direct_source() {
    let (url, direct) = server(vec![(
        403,
        "cf-mitigated: challenge\r\n",
        "challenge".into(),
    )]);
    let http = transport();
    let cancel = Arc::new(AtomicBool::new(false));
    for _ in 0..2 {
        let error = http
            .get_text(&url, &SolverConfig::default(), &cancel)
            .unwrap_err();
        assert!(error.to_string().contains("solver"), "{error:#}");
    }
    assert_eq!(direct.join().unwrap().len(), 1);
}
#[test]
fn detected_challenge_falls_back_and_followup_uses_solver_without_another_direct_request() {
    let (url, direct) = server(vec![(403, "cf-mitigated: challenge\r\n", "blocked".into())]);
    let (api, solver) = server(vec![
        (200, "", solved(&url, "first page")),
        (200, "", solved(&url, "second page")),
    ]);
    let config = SolverConfig {
        endpoint: Some(api),
        ..Default::default()
    };
    let http = transport();
    let cancel = Arc::new(AtomicBool::new(false));
    assert_eq!(http.get_text(&url, &config, &cancel).unwrap(), "first page");
    assert_eq!(
        http.get_text(&url, &config, &cancel).unwrap(),
        "second page"
    );
    assert_eq!(direct.join().unwrap().len(), 1);
    let requests = solver.join().unwrap();
    for request in requests {
        assert!(request.starts_with("POST /v1 HTTP/1.1"));
        let payload: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(payload["cmd"], "request.get");
        assert_eq!(payload["url"], format!("{url}/"));
        assert_eq!(payload["maxTimeout"], 120000);
    }
}
#[test]
fn ordinary_forbidden_response_does_not_mark_source_or_call_solver() {
    let (url, direct) = server(vec![
        (403, "", "not licensed".into()),
        (200, "", "allowed".into()),
    ]);
    let http = transport();
    let cancel = Arc::new(AtomicBool::new(false));
    assert!(
        http.get_text(&url, &SolverConfig::default(), &cancel)
            .is_err()
    );
    assert_eq!(
        http.get_text(&url, &SolverConfig::default(), &cancel)
            .unwrap(),
        "allowed"
    );
    assert_eq!(direct.join().unwrap().len(), 2);
}
#[test]
fn solver_timeout_envelope_is_retried_once_but_unresolved_challenge_is_never_parsed() {
    let (url,direct)=server(vec![(200,"",r#"<title>Just a moment...</title><script src="/cdn-cgi/challenge-platform/a.js"></script>"#.into())]);
    let (api, solver) = server(vec![
        (
            500,
            "",
            r#"{"status":"error","message":"Scrape deadline exceeded","solution":{}}"#.into(),
        ),
        (200, "", solved(&url, "usable page")),
    ]);
    let config = SolverConfig {
        endpoint: Some(api),
        ..Default::default()
    };
    let http = transport();
    let cancel = Arc::new(AtomicBool::new(false));
    assert_eq!(
        http.get_text(&url, &config, &cancel).unwrap(),
        "usable page"
    );
    assert_eq!(direct.join().unwrap().len(), 1);
    assert_eq!(solver.join().unwrap().len(), 2);
}

#[test]
fn ordinary_cloudflare_pages_and_embedded_widgets_do_not_require_a_solver() {
    for body in [
        "Hosted by Cloudflare",
        r#"<title>Catalog</title><script src="https://challenges.cloudflare.com/turnstile/v0/api.js"></script>"#,
    ] {
        let (url, direct) = server(vec![(200, "server: cloudflare\r\n", body.into())]);
        assert_eq!(
            transport()
                .get_text(
                    &url,
                    &SolverConfig::default(),
                    &Arc::new(AtomicBool::new(false))
                )
                .unwrap(),
            body
        );
        direct.join().unwrap();
    }
}
#[test]
fn unresolved_or_malformed_solver_results_never_become_catalog_content() {
    for response in [
        r#"{"status":"ok","solution":{}}"#.to_owned(),
        r#"{"status":"ok","solution":{"url":"https://elsewhere.invalid/","status":200,"response":"not the catalog"}}"#.to_owned(),
        "not JSON".to_owned(),
    ] {
        let (url,direct)=server(vec![(403,"cf-mitigated: challenge\r\n","challenge".into())]);
        let (api,solver)=server(vec![(200,"",response)]);
        let config=SolverConfig{endpoint:Some(api),..Default::default()};
        assert!(transport().get_text(&url,&config,&Arc::new(AtomicBool::new(false))).is_err());
        direct.join().unwrap();assert_eq!(solver.join().unwrap().len(),1);
    }
    let (url, direct) = server(vec![(
        403,
        "cf-mitigated: challenge\r\n",
        "challenge".into(),
    )]);
    let (api, solver) = server(vec![(
        200,
        "",
        solved(
            &url,
            r#"<title>Just a moment...</title><script>var _cf_chl_opt={}</script>"#,
        ),
    )]);
    let error = transport()
        .get_text(
            &url,
            &SolverConfig {
                endpoint: Some(api),
                ..Default::default()
            },
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
    assert!(error.to_string().contains("unresolved"));
    direct.join().unwrap();
    solver.join().unwrap();
}
#[test]
fn exhausted_timeouts_are_bounded_and_never_fall_back_to_direct_access() {
    let (url, direct) = server(vec![(
        403,
        "cf-mitigated: challenge\r\n",
        "challenge".into(),
    )]);
    let timeout = r#"{"status":"error","message":"Timeout after 120 seconds"}"#.to_owned();
    let (api, solver) = server(vec![(500, "", timeout.clone()), (500, "", timeout)]);
    let error = transport()
        .get_text(
            &url,
            &SolverConfig {
                endpoint: Some(api),
                ..Default::default()
            },
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
    assert!(error.to_string().contains("timed out"));
    assert_eq!(direct.join().unwrap().len(), 1);
    assert_eq!(solver.join().unwrap().len(), 2);
}
#[test]
fn cancellation_interrupts_an_in_flight_solve_and_prevents_retries() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api = format!("http://{}", listener.local_addr().unwrap());
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let slow = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut data = [0; 4096];
        assert!(socket.read(&mut data).unwrap() > 0);
        signal.store(true, std::sync::atomic::Ordering::Relaxed);
        thread::sleep(Duration::from_millis(300));
    });
    let (url, direct) = server(vec![(
        403,
        "cf-mitigated: challenge\r\n",
        "challenge".into(),
    )]);
    let start = std::time::Instant::now();
    let error = transport()
        .get_text(
            &url,
            &SolverConfig {
                endpoint: Some(api),
                ..Default::default()
            },
            &cancel,
        )
        .unwrap_err();
    assert!(error.to_string().contains("stopping"));
    assert!(start.elapsed() < Duration::from_secs(2));
    direct.join().unwrap();
    slow.join().unwrap();
}

#[test]
fn solver_urls_are_normalized_and_credentials_are_rejected() {
    for (input, expected) in [
        ("http://localhost:8191/", "http://localhost:8191/v1"),
        (
            "https://solver.invalid/prefix/v1/",
            "https://solver.invalid/prefix/v1",
        ),
        (
            "https://solver.invalid/prefix",
            "https://solver.invalid/prefix/v1",
        ),
    ] {
        let config = SolverConfig {
            endpoint: Some(input.into()),
            ..Default::default()
        };
        assert_eq!(config.api_url().unwrap().unwrap().as_str(), expected);
    }
    for endpoint in [
        "file:///tmp/solver",
        "https://user:password@solver.invalid/v1",
        "https://solver.invalid/v1?key=secret",
        "https://solver.invalid/#key",
    ] {
        let config = SolverConfig {
            endpoint: Some(endpoint.into()),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}

#[test]
fn interrupted_solver_connection_is_retried_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api = format!("http://{}", listener.local_addr().unwrap());
    let (url, direct) = server(vec![(
        403,
        "cf-mitigated: challenge\r\n",
        "challenge".into(),
    )]);
    let body = solved(&url, "recovered page");
    let solver = thread::spawn(move || {
        for attempt in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 8192];
            assert!(socket.read(&mut request).unwrap() > 0);
            if attempt == 1 {
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
            }
        }
    });
    let result = transport().get_text(
        &url,
        &SolverConfig {
            endpoint: Some(api),
            ..Default::default()
        },
        &Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(result.unwrap(), "recovered page");
    direct.join().unwrap();
    solver.join().unwrap();
}

#[test]
fn interrupted_solver_response_body_is_retried_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api = format!("http://{}", listener.local_addr().unwrap());
    let (url, direct) = server(vec![(
        403,
        "cf-mitigated: challenge\r\n",
        "challenge".into(),
    )]);
    let body = solved(&url, "complete page");
    let solver = thread::spawn(move || {
        for attempt in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 8192];
            assert!(socket.read(&mut request).unwrap() > 0);
            if attempt == 0 {
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n{\"status\":").unwrap();
            } else {
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
            }
        }
    });
    let result = transport().get_text(
        &url,
        &SolverConfig {
            endpoint: Some(api),
            ..Default::default()
        },
        &Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(result.unwrap(), "complete page");
    direct.join().unwrap();
    solver.join().unwrap();
}
