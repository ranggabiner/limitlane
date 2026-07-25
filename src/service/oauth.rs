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

pub async fn perform_oauth_flow(provider: &str) -> Result<String, LimitLaneError> {
    let port = 1455;
    let redirect_uri = format!("http://localhost:{}/callback", port);

    let (auth_url, client_id, token_url) = match provider.to_lowercase().as_str() {
        "claude" => (
            format!("https://claude.ai/oauth/authorize?client_id=anthropic-cli&response_type=code&redirect_uri={}&scope=user:inference", redirect_uri),
            "anthropic-cli",
            "https://claude.ai/oauth/token"
        ),
        _ => (
            format!("https://auth.openai.com/authorize?client_id=app-355525547631-openai&response_type=code&redirect_uri={}&scope=openid%20profile%20email%20offline_access", redirect_uri),
            "app-355525547631-openai",
            "https://auth.openai.com/oauth/token"
        ),
    };

    println!("\n================================================================================");
    println!("                        OAuth 2.0 Browser Authorization                         ");
    println!("================================================================================");
    println!("Opening your default browser to authorize LimitLane:\n");
    println!("  {}", auth_url);

    open_browser(&auth_url);

    let server = LocalOAuthServer::new(port);
    let auth_code = server.listen_for_code().await?;

    println!("\n[OK] Received OAuth Authorization Code!");
    println!("     Exchanging code for access token...");

    let client = reqwest::Client::new();
    let mut params = std::collections::HashMap::new();
    params.insert("grant_type", "authorization_code");
    params.insert("client_id", client_id);
    params.insert("code", &auth_code);
    params.insert("redirect_uri", &redirect_uri);

    match client.post(token_url).form(&params).send().await {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = resp.json().await.map_err(|e| LimitLaneError::Parsing {
                provider: provider.to_string(),
                message: format!("Failed to parse token response: {}", e),
            })?;

            let token = json.get("access_token")
                .or_else(|| json.get("token"))
                .and_then(|v| v.as_str())
                .unwrap_or(&auth_code)
                .to_string();

            Ok(token)
        }
        _ => Ok(auth_code),
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(&["/C", "start", url]).spawn();
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
