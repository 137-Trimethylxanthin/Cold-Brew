use serde_json::json;

pub struct Api {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
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
            api_key: api_response.unwrap(),
            base_url,
            user_name,
            password,
        }
    }

    pub async fn get_all_songs(&self) -> Result<serde_json::Value, ()> {
        let url = format!("{}/Items/{}", &self.base_url, "?includeItemTypes=Audio");
        let response = self
            .client
            .get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .expect("hi");

        if response.status().is_success() {
            let response_body: serde_json::Value = response.json().await.unwrap();
            println!("{}",&response_body);
            Ok(response_body)
        } else {
            Err(())
        }
    }
}

async fn authenticate(base_url: &str, user_name: &str, pw: &str) -> Result<String, ()> {
    let url = format!("{}/Users/AuthenticateByName", base_url);
    let body = json!({
        "Username": user_name,
        "Pw": pw,
    });

    let client = reqwest::Client::new();
    let response = client.post(&url).json(&body).send().await.unwrap();

    if response.status().is_success() {
        let response_body: serde_json::Value = response.json().await.expect("Hi");
        println!("{}",&response_body);
        Ok(response_body["AccessToken"].as_str().unwrap().to_string())
    } else {
        Err(())
    }
}
