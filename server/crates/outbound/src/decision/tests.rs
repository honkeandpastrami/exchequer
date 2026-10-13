use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::thread;

use super::*;

/// Serves one response per `(status, headers, body)`, in order, and returns
/// each request's head and body.
fn serve(
    responses: Vec<(u16, &'static str, &'static str)>,
) -> (String, thread::JoinHandle<Vec<(String, String)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = thread::spawn(move || {
        let mut seen = Vec::new();
        for (status, headers, body) in responses {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
            }
            let mut buf = vec![0; length];
            reader.read_exact(&mut buf).unwrap();
            seen.push((head, String::from_utf8(buf).unwrap()));
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
        seen
    });
    (url, handle)
}

fn question() -> Question {
    Question {
        name: "team".into(),
        instructions: "Which team?".into(),
        options: vec![
            ("support".into(), "How-to questions".into()),
            ("billing".into(), "Charges".into()),
        ],
    }
}

const ANSWER: &str = r#"{"model": "microsoft-decision-1", "answers": {"team": {"type": "choice",
    "choice": "billing", "probabilities": {"billing": 0.9375, "support": 6.25e-2}}}}"#;

#[test]
fn a_decision_is_asked_and_read() {
    let (url, server) = serve(vec![(200, "", ANSWER)]);
    let c = Client::new(&url, "my-deployment", "k3y", Duration::from_secs(5)).unwrap();
    let d = c
        .decide(&serde_json::json!({ "message": "refund?" }), &[question()])
        .unwrap();
    assert_eq!(d.model, "microsoft-decision-1");
    let a = &d.answers["team"];
    assert_eq!((a.choice.as_str(), a.confidence()), ("billing", 938));
    assert_eq!(a.probabilities["support"], 63);

    let (head, body) = &server.join().unwrap()[0];
    assert!(
        head.starts_with("POST /providers/microsoft/v1/systemone "),
        "{head}"
    );
    assert!(head.contains("api-key: k3y\r\n"), "{head}");
    // Options keep their order.
    assert_eq!(
        body,
        r#"{"model":"my-deployment","state":{"message":"refund?"},"questions":{"team":{"type":"choice","instructions":"Which team?","criteria":{"support":"How-to questions","billing":"Charges"}}}}"#
    );
}

#[test]
fn busy_is_retried_and_refusals_are_not() {
    let (url, server) = serve(vec![
        (429, "retry-after: 0\r\n", "{}"),
        (200, "", ANSWER),
        (
            400,
            "",
            r#"{"error": {"code": "content_filter", "message": "the text was …"}}"#,
        ),
    ]);
    let c = Client::new(&url, "d", "k", Duration::from_secs(5)).unwrap();
    let state = serde_json::json!({});
    assert!(c.decide(&state, &[question()]).is_ok());
    let e = c.decide(&state, &[question()]).unwrap_err();
    assert!(
        matches!(&e, Error::Refused { status: 400, code: Some(c) } if c == "content_filter"),
        "{e}"
    );
    // The message, which may quote the request, is never kept.
    assert!(!e.to_string().contains("text"));
    assert_eq!(server.join().unwrap().len(), 3);
}

#[test]
fn a_bad_key_is_an_error() {
    let (url, server) = serve(vec![(401, "", r#"{"error": {"code": "401"}}"#)]);
    let c = Client::new(&url, "d", "wrong", Duration::from_secs(5)).unwrap();
    let e = c.decide(&serde_json::json!({}), &[question()]).unwrap_err();
    assert!(matches!(e, Error::Status { status: 401, .. }), "{e}");
    server.join().unwrap();
}

#[test]
fn only_https_leaves_the_machine() {
    let new = |e: &str| Client::new(e, "d", "k", Duration::from_secs(1));
    assert!(new("http://example.com").is_err());
    assert!(new("https://example.services.ai.azure.com/").is_ok());
    assert!(
        new("https://example.services.ai.azure.com")
            .unwrap()
            .url
            .ends_with(".com/providers/microsoft/v1/systemone")
    );
    assert!(new("http://127.0.0.1:9").is_ok());
}

#[test]
fn malformed_answers_are_errors() {
    assert!(parse("[]").is_err());
    let d = parse(r#"{"answers": {"a": {"choice": "x", "probabilities": {"x": 1}}, "b": {}}}"#)
        .unwrap();
    assert_eq!(d.answers.len(), 1);
    assert_eq!(d.answers["a"].confidence(), 1000);
}
