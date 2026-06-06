use serde_json::{Value, json};

pub struct Api {
    client: reqwest::Client,
    base_url: String,
    token: String,
    user_id: String,
}

struct AuthSession {
    token: String,
    user_id: String,
}

impl Api {
    pub async fn new(
        base_url: String,
        user_name: String,
        password: String,
    ) -> Result<Self, String> {
        let auth_session = authenticate(&base_url, &user_name, &password).await?;
        Ok(Self {
            client: reqwest::Client::new(),
            token: auth_session.token,
            user_id: auth_session.user_id,
            base_url: normalize_base_url(&base_url),
        })
    }

    pub async fn get_all_songs(&self) -> Result<Value, String> {
        let url = format!("{}/Users/{}/Items", &self.base_url, self.user_id);
        let response = self
            .client
            .get(&url)
            .header(
                "Authorization",
                format!("MediaBrowser Token=\"{}\"", self.token),
            )
            .header("Content-type", "application/json")
            .header("Token", &self.token)
            .header("X-Application", "Cold Brew")
            .header("x-emby-authorization", self.get_auth_string())
            .query(&[("Recursive", "true"), ("IncludeItemTypes", "Audio")])
            .send()
            .await
            .map_err(|error| format!("Failed to request Jellyfin songs: {error}"))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("Jellyfin song request failed with HTTP {status}."));
        }

        response
            .json()
            .await
            .map_err(|error| format!("Failed to parse Jellyfin song response: {error}"))
    }

    fn get_auth_string(&self) -> String {
        let mut auth_str: String = format!(
            "MediaBrowser Client={}, Device={}, DeviceId=-, Version={}",
            "Cold-Brew", "Desktop", "0.1.0"
        );
        if !self.token.is_empty() {
            auth_str = format!("{}, Token={}", auth_str, self.token);
        }
        auth_str
    }
}

async fn authenticate(base_url: &str, user_name: &str, pw: &str) -> Result<AuthSession, String> {
    if user_name.trim().is_empty() || pw.trim().is_empty() {
        return Err("Jellyfin username and password must not be empty.".to_string());
    }

    let url: String = format!("{}/Users/AuthenticateByName", normalize_base_url(base_url));
    let body = json!({
        "Username": user_name,
        "Pw": pw,
    });

    let auth_str: String = format!(
        "MediaBrowser Client={}, Device={}, DeviceId=-, Version={}",
        "Cold-Brew", "Desktop", "0.1.0"
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-type", "application/json")
        .header("X-Application", "Cold Brew")
        .header("x-emby-authorization", auth_str)
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("Failed to authenticate with Jellyfin: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Jellyfin authentication failed with HTTP {status}."
        ));
    }

    let response_body: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse Jellyfin auth response: {error}"))?;
    let token = response_body
        .get("AccessToken")
        .and_then(Value::as_str)
        .ok_or_else(|| "Jellyfin auth response did not include an access token.".to_string())?;
    let user_id = response_body
        .get("User")
        .and_then(|user| user.get("Id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Jellyfin auth response did not include a user id.".to_string())?;

    Ok(AuthSession {
        token: token.to_string(),
        user_id: user_id.to_string(),
    })
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}
