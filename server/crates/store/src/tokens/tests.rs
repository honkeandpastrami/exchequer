use super::*;

#[test]
fn a_scripts_sessions_are_its_tools_and_hosts() {
    let client = |agent: &str, host: &str| Client {
        agent: Some(agent.into()),
        host: Some(host.into()),
        ip: None,
    };
    assert_eq!(
        session_key(Kind::Script, &client("Backup/1.2 python-httpx/0.28", "nas")),
        "backup@nas"
    );
    assert_eq!(
        session_key(Kind::Script, &client("backup/1.3", "nas")),
        session_key(Kind::Script, &client("Backup/1.2", "nas")),
        "a new version is the same session"
    );
    assert_eq!(session_key(Kind::Script, &Client::default()), "@");
    assert_eq!(
        session_key(Kind::Cli, &client("exchequer-cli/1.0", "laptop")),
        ""
    );
}
