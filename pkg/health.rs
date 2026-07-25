use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub fn check_health_on_host(url: &str, host: &str, timeout_secs: u64) -> Result<bool, String> {
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("Bad health check URL '{}': {}", url, e))?;

    if parsed.scheme() != "http" {
        return Err("Only HTTP is supported for health checks right now.".to_string());
    }

    let path = parsed.path().to_string();
    let port = parsed.port().unwrap_or(80);
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        let addr_str = format!("{}:{}", host, port);
        if let Ok(mut addrs) = addr_str.to_socket_addrs() {
            if let Some(sockaddr) = addrs.next() {
                if let Ok(mut stream) = TcpStream::connect_timeout(&sockaddr, Duration::from_secs(5)) {
                    let request = format!(
                        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                        path, host
                    );
                    if stream.write_all(request.as_bytes()).is_ok() {
                        let mut response = String::new();
                        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                        if stream.read_to_string(&mut response).is_ok() {
                            let first_line = response.lines().next().unwrap_or("");
                            if first_line.contains(" 2") {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_health_check_url() {
        let result = check_health_on_host("not a url", "10.0.0.1", 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Bad health check URL"));
    }

    #[test]
    fn test_https_not_supported() {
        let result = check_health_on_host("https://example.com/health", "10.0.0.1", 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Only HTTP"));
    }

    #[test]
    fn test_timeout_returns_false() {
        // 10.255.255.1 is a reserved/test address that should time out
        let result = check_health_on_host("http://10.255.255.1:9999/health", "10.255.255.1", 2);
        // Should return false (timeout) or error
        if let Ok(healthy) = result {
            assert!(!healthy);
        }
    }

    #[test]
    fn test_url_with_path() {
        let url = url::Url::parse("http://localhost:8080/healthz").unwrap();
        assert_eq!(url.path(), "/healthz");
        assert_eq!(url.port(), Some(8080));
    }
}
