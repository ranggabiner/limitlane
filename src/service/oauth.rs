use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use crate::error::LimitLaneError;

pub struct LocalOAuthServer {
    pub port: u16,
}

impl LocalOAuthServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn listen_for_code(&self) -> Result<String, LimitLaneError> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            LimitLaneError::Network {
                url: format!("http://localhost:{}", self.port),
                message: format!("Failed to bind local OAuth listener port {}: {}", self.port, e),
            }
        })?;

        println!("⚡ Local OAuth server running on http://localhost:{}/callback", self.port);
        println!("   Waiting for browser authorization callback...");

        let (mut socket, _) = listener.accept().await.map_err(|e| {
            LimitLaneError::Network {
                url: format!("http://localhost:{}", self.port),
                message: format!("Failed to accept OAuth connection: {}", e),
            }
        })?;

        let mut buffer = [0u8; 4096];
        let bytes_read = socket.read(&mut buffer).await.map_err(|e| {
            LimitLaneError::Network {
                url: format!("http://localhost:{}", self.port),
                message: format!("Failed to read HTTP request: {}", e),
            }
        })?;

        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

        let code = extract_query_param(&request_str, "code")
            .or_else(|| extract_query_param(&request_str, "token"))
            .or_else(|| extract_query_param(&request_str, "access_token"))
            .or_else(|| extract_query_param(&request_str, "session_token"))
            .ok_or_else(|| LimitLaneError::Authentication {
                provider: "oauth".into(),
                account_id: "local".into(),
                message: "No authorization code or token received in OAuth callback".into(),
            })?;

        let response_body = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>LimitLane Authorization</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #f8fafc; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }
    .card { background: #1e293b; border-radius: 12px; padding: 40px; text-align: center; box-shadow: 0 10px 25px rgba(0,0,0,0.5); max-width: 400px; border: 1px solid #334155; }
    h1 { color: #38bdf8; margin-top: 0; font-size: 24px; }
    p { color: #94a3b8; line-height: 1.5; }
    .badge { display: inline-block; background: #0369a1; color: #e0f2fe; padding: 6px 14px; border-radius: 20px; font-weight: bold; margin-top: 15px; }
  </style>
</head>
<body>
  <div class="card">
    <h1>✅ Authorization Successful!</h1>
    <p>LimitLane has captured your OAuth credentials automatically.</p>
    <div class="badge">You can close this tab</div>
  </div>
</body>
</html>"#;

        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );

        socket.write_all(http_response.as_bytes()).await.ok();
        socket.flush().await.ok();

        Ok(code)
    }
}

fn extract_query_param(req: &str, param: &str) -> Option<String> {
    let target = format!("{}=", param);
    if let Some(pos) = req.find(&target) {
        let rest = &req[pos + target.len()..];
        let end = rest.find('&').or_else(|| rest.find(' ')).unwrap_or(rest.len());
        let val = &rest[..end];
        return Some(val.to_string());
    }
    None
}
