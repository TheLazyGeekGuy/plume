//! Plume: the desktop app's way to reach GitHub, GitLab and Forgejo (an HTTPS client for
//! [`wordcraft_vcs::forge`]), and the system keychain that remembers access tokens.

use std::io::Read;
use std::time::Duration;

use wordcraft_vcs::forge::{Request, Response, Transport};

/// Largest answer read from a forge.
const MAX_ANSWER: u64 = 160 << 20;

/// HTTPS through `ureq`, with the system's proxy settings and the platform's certificate checks.
pub struct Https {
    agent: ureq::Agent,
}

impl Https {
    pub fn new() -> Https {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .user_agent(format!("{}/{}", wordcraft_brand::APP_NAME, env!("CARGO_PKG_VERSION")))
            .build();
        Https { agent: config.into() }
    }
}

impl Transport for Https {
    fn send(&self, req: &Request) -> Result<Response, String> {
        let mut b = ureq::http::Request::builder().method(req.method.as_str()).uri(&req.url);
        for (k, v) in &req.headers {
            b = b.header(k.as_str(), v.as_str());
        }
        let resp = match &req.body {
            Some(body) => self.agent.run(b.body(body.clone()).map_err(|e| e.to_string())?),
            None => self.agent.run(b.body(()).map_err(|e| e.to_string())?),
        };
        let mut resp = resp.map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let mut body = Vec::new();
        resp.body_mut().as_reader().take(MAX_ANSWER).read_to_end(&mut body).map_err(|e| e.to_string())?;
        Ok(Response { status, body })
    }
}

/// The keychain entries' service name.
#[cfg(target_os = "macos")]
fn service() -> String {
    format!("{} forge token", wordcraft_brand::APP_NAME)
}

/// The macOS Keychain, for access tokens (one generic password per forge API address).
#[cfg(target_os = "macos")]
pub fn secrets() -> Option<wordcraft_ui_egui::Secrets> {
    use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};
    Some(wordcraft_ui_egui::Secrets {
        get: Box::new(|account| get_generic_password(&service(), account).ok().and_then(|b| String::from_utf8(b).ok())),
        set: Box::new(|account, secret| set_generic_password(&service(), account, secret.as_bytes()).map_err(|e| e.to_string())),
        forget: Box::new(|account| {
            let _ = delete_generic_password(&service(), account);
        }),
    })
}

/// Elsewhere tokens last for the session (Plume is developed on macOS first).
#[cfg(not(target_os = "macos"))]
pub fn secrets() -> Option<wordcraft_ui_egui::Secrets> {
    None
}

#[cfg(test)]
mod tests {
    use wordcraft_vcs::forge::{Method, Request, Transport};

    /// A real request to GitHub's API (network): `cargo test -p wordcraft -- --ignored live`.
    #[test]
    #[ignore]
    fn live_github_api() {
        let req = Request {
            method: Method::Get,
            url: "https://api.github.com/repos/storytold/wordcraft".into(),
            headers: vec![("Accept".into(), "application/json".into())],
            body: None,
        };
        let resp = super::Https::new().send(&req).unwrap();
        assert_eq!(resp.status, 200, "{}", String::from_utf8_lossy(&resp.body));
        assert!(String::from_utf8_lossy(&resp.body).contains("\"full_name\""));
    }
}
