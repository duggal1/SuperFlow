use crate::error::IntegrationError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use url::Url;

pub struct LoopbackCallback {
    listener: TcpListener,
    redirect_uri: String,
}

impl LoopbackCallback {
    pub async fn bind(redirect_host: &str) -> Result<Self, IntegrationError> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            listener,
            redirect_uri: format!("http://{redirect_host}:{port}/oauth/callback"),
        })
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub async fn wait(self, expected_state: &str) -> Result<String, IntegrationError> {
        let (mut stream, _) = timeout(Duration::from_secs(300), self.listener.accept())
            .await
            .map_err(|_| IntegrationError::OAuthTimeout)??;
        let request_bytes = timeout(Duration::from_secs(10), async {
            let mut collected = Vec::with_capacity(4096);
            let mut chunk = [0u8; 2048];
            loop {
                let size = stream.read(&mut chunk).await?;
                if size == 0 {
                    break;
                }
                collected.extend_from_slice(&chunk[..size]);
                if collected.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if collected.len() >= 16 * 1024 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "oauth callback request too large",
                    ));
                }
            }
            Ok::<_, std::io::Error>(collected)
        })
        .await
        .map_err(|_| IntegrationError::OAuthTimeout)??;
        let request = String::from_utf8_lossy(&request_bytes);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or_else(|| IntegrationError::OAuth("invalid callback request".into()))?;
        let callback_url = Url::parse(&format!("http://127.0.0.1{target}"))?;
        if callback_url.path() != "/oauth/callback" {
            return Err(IntegrationError::OAuth("invalid callback path".into()));
        }
        let params = callback_url
            .query_pairs()
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>();
        let result = if let Some(error) = params.get("error") {
            Err(IntegrationError::OAuth(error.clone()))
        } else if params.get("state").map(String::as_str) != Some(expected_state) {
            Err(IntegrationError::OAuthStateMismatch)
        } else {
            params
                .get("code")
                .cloned()
                .ok_or_else(|| IntegrationError::OAuth("authorization code missing".into()))
        };
        let (status, body) = match &result {
            Ok(_) => (
                "200 OK",
                "Authentication complete. You can close this tab and return to Tori.",
            ),
            Err(_) => (
                "400 Bad Request",
                "Authentication failed. You can close this tab and return to Tori.",
            ),
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await?;
        result
    }
}
