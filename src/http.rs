use reqwest::blocking::Client;

//for now, just a thin wrapper
pub struct HttpClient {
  client: Client,
}

impl std::default::Default for HttpClient {
  fn default() -> Self {
    //for privacy can change to more common one
    let client = Client::builder().user_agent("Koxinga").build().unwrap();
    Self {
      client,
    }
  }
}

impl HttpClient {
  pub fn get(&self, url: &str) -> Option<String> {
    if let Ok(resp) = self.client.get(url).send() {
      if let Ok(text) = resp.text() {
        return Some(text);
      }
    }
    None
  }

  //todo: form submit (get/post)
}
