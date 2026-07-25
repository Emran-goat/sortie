use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;

pub(crate) fn sh_quote(s: &str) -> String {
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

pub fn connect(host: &str, port: u16, user: &str, key_path: Option<&str>) -> Result<Session, String> {
    let addr = format!("{}:{}", host, port);
    let tcp = TcpStream::connect(&addr)
        .map_err(|e| format!("Can't reach {} on port {}: {}", host, port, e))?;

    let mut session = Session::new()
        .map_err(|e| format!("Couldn't create an SSH session: {}", e))?;
    session.set_tcp_stream(tcp);
    session.handshake()
        .map_err(|e| format!("SSH handshake with {} failed: {}", host, e))?;

    let key = match key_path {
        Some(k) => k.to_string(),
        None => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            format!("{}/.ssh/id_rsa", home)
        }
    };

    session.userauth_pubkey_file(user, None, Path::new(&key), None)
        .map_err(|e| format!("SSH key auth didn't work for {}@{}: {}", user, host, e))?;

    session.set_timeout(30000);

    Ok(session)
}

pub fn upload_file(session: &Session, local: &Path, remote: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(local)
        .map_err(|e| format!("Can't read local file '{}': {}", local.display(), e))?;

    let mut channel = session.scp_send(remote, 0o755_i32, meta.len(), None)
        .map_err(|e| format!("Couldn't start upload to '{}': {}", remote.display(), e))?;

    let mut file = std::fs::File::open(local)
        .map_err(|e| format!("Can't open '{}' for reading: {}", local.display(), e))?;

    std::io::copy(&mut file, &mut channel)
        .map_err(|e| format!("Upload interrupted: {}", e))?;

    channel.send_eof().ok();
    channel.wait_eof().ok();
    channel.close().ok();
    channel.wait_close().ok();

    Ok(())
}

pub fn run_command(session: &Session, cmd: &str) -> Result<(String, String, i32), String> {
    let mut channel = session.channel_session()
        .map_err(|e| format!("Couldn't open a command channel: {}", e))?;

    channel.exec(cmd)
        .map_err(|e| format!("Failed to run command on remote: {}", e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    channel.read_to_string(&mut stdout).ok();
    channel.stderr().read_to_string(&mut stderr).ok();

    channel.wait_close()
        .map_err(|e| format!("Command didn't finish cleanly: {}", e))?;
    let code = channel.exit_status().unwrap_or(-1);

    Ok((stdout, stderr, code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sh_quote_simple() {
        assert_eq!(sh_quote("hello"), "'hello'");
    }

    #[test]
    fn test_sh_quote_with_single_quote() {
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_sh_quote_with_spaces() {
        assert_eq!(sh_quote("my app"), "'my app'");
    }

    #[test]
    fn test_sh_quote_empty() {
        assert_eq!(sh_quote(""), "''");
    }

    #[test]
    fn test_sh_quote_multiple_quotes() {
        assert_eq!(sh_quote("a'b'c"), "'a'\\''b'\\''c'");
    }

    #[test]
    fn test_sh_quote_special_chars() {
        assert_eq!(sh_quote("$HOME"), "'$HOME'");
    }

    #[test]
    fn test_sh_quote_path() {
        assert_eq!(sh_quote("/opt/my app/bin"), "'/opt/my app/bin'");
    }
}
