use super::*;

#[test]
fn browsers_and_tools_are_described() {
    for (agent, expected) in [
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
            "Safari on iPhone",
        ),
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
            "Edge on Mac",
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0",
            "Firefox on Linux",
        ),
        (
            "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/130.0 Mobile Safari/537.36",
            "Chrome on Android",
        ),
        (
            "exchequer-cli/1.4.0 python-httpx/0.28.1",
            "exchequer-cli 1.4.0",
        ),
        ("backup.py", "backup.py"),
        ("curl/8.7.1", "curl 8.7.1"),
    ] {
        assert_eq!(describe(agent).as_deref(), Some(expected), "{agent}");
    }
    assert_eq!(describe("   "), None);
}

#[test]
fn the_address_is_the_one_the_proxy_added() {
    let mut headers = HeaderMap::new();
    let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();
    assert_eq!(
        client(&headers, Some(peer)).ip.as_deref(),
        Some("127.0.0.1")
    );
    headers.insert("x-forwarded-for", "10.0.0.9, 192.168.1.20".parse().unwrap());
    headers.insert(HOST_HEADER, " laptop ".parse().unwrap());
    let c = client(&headers, Some(peer));
    assert_eq!(c.ip.as_deref(), Some("192.168.1.20"));
    assert_eq!(c.host.as_deref(), Some("laptop"));
    assert_eq!(c.agent, None);
}
