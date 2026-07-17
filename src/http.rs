use std::collections::HashMap;

//use ming_wm_lib::logging::log;
use ming_wm_lib::utils::get_rest_of_split;

use crate::url::Url;

use reqwest::blocking::Client;

//for now, just a thin wrapper
pub struct HttpClient {
  client: Client,
  no_redirect_client: Client,
}

impl std::default::Default for HttpClient {
  fn default() -> Self {
    //we lie cause otherwise people block us. can't be honest no more
    let client = Client::builder().user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.3").build().unwrap();
    let no_redirect_client = Client::builder().user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.3").redirect(reqwest::redirect::Policy::none()).build().unwrap();
    Self {
      client,
      no_redirect_client,
    }
  }
}

fn serialise_cookies(cookies: &HashMap<String, String>) -> String {
  let mut c_header = String::new();
  for (name, value) in cookies {
    c_header += &format!("{}{}={}", if !c_header.is_empty() {
      "; "
    } else {
      ""
    }, name, value);
  }
  c_header
}

impl HttpClient {
  //the second return value, the final url, may differ from the input url, because of redirects
  pub fn get(&self, url: &str, cookies: Option<&HashMap<String, String>>) -> Option<(String, String)> {
    let mut req = self.client.get(url);
    //nom nom nom
    if let Some(cookies) = cookies {
      let c_header = serialise_cookies(cookies);
      //set cookie header
      if !c_header.is_empty() {
        req = req.header("Cookie", c_header);
      }
    }
    if let Ok(resp) = req.send() {
      let final_url = resp.url().as_str().to_string();
      if let Ok(text) = resp.text() {
        return Some((text, final_url));
      }
    }
    None
  }

  //todo: POST for form submit for cookies
  pub fn post(&self, url: Url, body: String, from_url: Url, cookies: Option<&HashMap<String, String>>) -> Option<(Url, Vec<(String, String)>)> {
    let mut url = url;
    let mut req = self.no_redirect_client.post(url.to_string()).body(body).header("Content-Type", "application/x-www-form-urlencoded").header("Origin", format!("https://{}", from_url.hostname));
    if let Some(cookies) = cookies {
      let c_header = serialise_cookies(cookies);
      if !c_header.is_empty() {
        req = req.header("Cookie", c_header);
      }
    }
    let mut cookies: Vec<(String, String)> = Vec::new();
    let mut redirect_count = 0;
    loop {
      if redirect_count > 5 {
        break;
      }
      if let Ok(resp) = req.send() {
        let c_headers = resp.headers().get_all("Set-Cookie");
        for header in c_headers {
          if let Ok(value) = header.to_str() {
            let mut parts = value.split(";").next().unwrap().split("=");
            let name = parts.next().unwrap_or_default().to_string();
            let value = get_rest_of_split(&mut parts, Some("="));
            cookies.push((name, value));
          }
        }
        if resp.status().is_redirection() {
          redirect_count += 1;
          //follow location resp header
          if let Some(location) = resp.headers().get("Location") {
            url = Url::new_maybe_relative(location.to_str().unwrap_or_default().to_string(), url);
            req = self.no_redirect_client.get(url.to_string());
            continue;
          }
        }
        return Some((url, cookies)); //break out
      } else {
        break;
      }
    }
    None
  }
}
