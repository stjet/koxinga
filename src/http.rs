use reqwest::blocking;

pub fn get(url: &str) -> Option<String> {
  if let Ok(resp) = blocking::get(url) {
    if let Ok(text) = resp.text() {
      return Some(text);
    }
  }
  None
}
