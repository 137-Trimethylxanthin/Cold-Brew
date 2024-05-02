use serde_json::json;

pub struct Api {
    client: reqwest::Client,
    base_url: String,
    token: String,
    user_name: String,
    password: String,
}

impl Api {
    pub async fn new(base_url: String, user_name: String, password: String) -> Self {
        let api_response = authenticate(&base_url, &user_name, &password).await;
        if api_response.is_err() {
            //TODO: Log error and return a Result instead of panicking
            panic!("Failed to authenticate with Jellyfin API");
        }
        Self {
            client: reqwest::Client::new(),
            token: api_response.unwrap(),
            base_url,
            user_name,
            password,
        }
    }

    pub async fn get_all_songs(&self) -> Result<serde_json::Value, ()> {

        
        let url = format!("{}/Users/{}/Items", &self.base_url,"697013bd2f9c41dc9e6e157941ef3bcf");
        println!("{}", &url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("MediaBrowser Token=\"{}\"", self.token))
            .header("Content-type", "application/json")
            .header("Token", &self.token)
            .header("X-Application", "Cold Brew")
            .header("x-emby-authorization", self.get_auth_string())
            .query(&[("Recursive", "true"), ("IncludeItemTypes", "Audio")])
            .send()
            .await
            .expect("hi");

        if response.status().is_success() {
            let response_body: serde_json::Value = response.json().await.unwrap();
            Ok(response_body)
        } else {
            println!("{}",response.status().to_string());
            Err(())
        }
    }

    fn get_auth_string(&self) -> String {
        let mut auth_str: String = format!("MediaBrowser Client={}, Device={}, DeviceId=-, Version={}", "Coffee", "PiCi", "0.1.1");
        if !self.token.is_empty() {
            auth_str = format!("{}, Token={}", auth_str, self.token);
        }
        auth_str
    }
}

async fn authenticate(base_url: &str, user_name: &str, pw: &str) -> Result<String, ()> {
    let url: String = format!("{}/Users/AuthenticateByName", base_url);
    let body = json!({
        "Username": user_name,
        "Pw": pw,
    });

    let auth_str: String = format!("MediaBrowser Client={}, Device={}, DeviceId=-, Version={}", "Coffee", "PiCi", "0.1.1");

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-type", "application/json")
        .header("X-Application", "Cold Brew")
        .header("x-emby-authorization", auth_str)
        .json(&body)
        .send()
        .await
        .unwrap();

    if response.status().is_success() {
        let response_body: serde_json::Value = response.json().await.expect("Hi");
        Ok(response_body["AccessToken"].as_str().unwrap().to_string())
    } else {
        Err(())
    }
}
