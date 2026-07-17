use std::vec::Vec;
use std::fmt;

const VALID_SCHEMES: [&'static str; 2] = ["HTTP", "HTTPS"]; //more to come in future?? who knows

//for the moment, we don't care about query params or fragments and the like
#[derive(Clone)]
pub struct Url {
  scheme: String, //http or https, probably
  pub valid_scheme: bool,
  pub hostname: String,
  path: Vec<String>,
  query: Option<String>, //empty or somethign like ?value1=yes&value2=abcd
}

impl fmt::Display for Url {
  fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
    if self.scheme != "" {
      fmt.write_str(&(self.scheme.clone() + "://" + &self.hostname + "/" + &self.path.join("/") + &self.query.as_ref().map_or("", |v| v)))?;
    }
    Ok(())
  }
}

impl Url {
  pub fn new(url: String) -> Url {
    let mut queries = url.split("?");
    let mut p = queries.next().unwrap().split("://");
    let scheme = p.next().unwrap_or("").to_string();
    let valid_scheme = VALID_SCHEMES.contains(&scheme.to_uppercase().as_str());
    p = p.next().unwrap_or("").split("/");
    let hostname = p.next().unwrap_or("").to_string();
    let path = p.filter(|s| *s != "").map(|s| s.to_string()).collect();
    let query = match queries.next() {
      Some(q) => Some(format!("?{}", q)),
      None => None,
    };
    Self { scheme, valid_scheme, hostname, path, query }
  }

  pub fn new_maybe_relative(url: String, current_url: Url) -> Url {
    if url.split("://").count() < 2 {
      let mut use_url = current_url;
      if url.starts_with("/") {
        use_url.pop_to_root();
      };
      use_url.append(url);
      use_url
    } else {
      Url::new(url)
    }
  }

  pub fn pop(&mut self) {
    self.path.pop();
    self.query = None;
  }
  
  pub fn pop_to_root(&mut self) {
    self.path = Vec::new();
    self.query = None;
  }
  
  pub fn append(&mut self, path: String) {
    self.path.extend(path.split("/").filter(|s| *s != "").map(|s| s.to_string()));
  }
  
  pub fn append_query(&mut self, key: &str, value: &str) {
    if self.query.is_none() {
      self.query = Some(format!("?{}={}", key, value));
    } else {
      self.query = Some(format!("{}&{}={}", self.query.as_ref().unwrap(), key, value));
    }
  }
}
