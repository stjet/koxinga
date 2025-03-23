use std::vec::Vec;
use std::vec;
use std::fmt;
use std::boxed::Box;

use ming_wm_lib::window_manager_types::{ DrawInstructions, WindowLike, WindowLikeType };
use ming_wm_lib::messages::{ WindowMessage, WindowMessageResponse };
use ming_wm_lib::utils::Substring;
use ming_wm_lib::framebuffer_types::Dimensions;
use ming_wm_lib::themes::ThemeInfo;
use ming_wm_lib::ipc::listen;

mod http;
use crate::http::get;
mod xml;
use crate::xml::{ parse, Node, OutputType };

const LINE_HEIGHT: usize = 18;

fn get_base_url(url: &str) -> String {
  let mut base_url = String::new();
  let mut slash = 0;
  for c in url.chars() {
    if c == '/' {
      slash += 1;
      if slash == 3 {
        break;
      }
    }
    base_url += &c.to_string();
  }
  base_url
}

#[derive(Default, PartialEq)]
enum Mode {
  #[default]
  Normal,
  Url,
  Link,
  Search,
}

impl fmt::Display for Mode {
  fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
    fmt.write_str(match self {
      Mode::Normal => "NORMAL",
      Mode::Url => "URL",
      Mode::Link => "LINK",
      Mode::Search => "SEARCH",
    })?;
    Ok(())
  }
}

#[derive(Default)]
struct KoxingaBrowser {
  dimensions: Dimensions,
  mode: Mode,
  max_lines: usize,
  top_line_no: usize,
  url: Option<String>,
  input: String,
  links: Vec<String>,
  top_level_nodes: Vec<Box<Node>>,
  page: Vec<(usize, usize, String, bool)>, //x, y, text, link colour or not
}

impl WindowLike for KoxingaBrowser {
  fn handle_message(&mut self, message: WindowMessage) -> WindowMessageResponse {
    match message {
      WindowMessage::Init(dimensions) => {
        self.dimensions = dimensions;
        WindowMessageResponse::JustRedraw
      },
      WindowMessage::ChangeDimensions(dimensions) => {
        self.dimensions = dimensions;
        self.calc_page();
        WindowMessageResponse::JustRedraw
      },
      WindowMessage::KeyPress(key_press) => {
        match self.mode {
          Mode::Normal => {
            let max_lines_screen = (self.dimensions[1] - 4) / LINE_HEIGHT - 1;
            if key_press.key == 'u' {
              self.mode = Mode::Url;
              self.input = self.url.clone().unwrap_or(String::new());
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'l' && self.url.is_some() {
              self.mode = Mode::Link;
              self.calc_page();
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 's' {
              self.mode = Mode::Search;
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'k' {
              if self.top_line_no > 0 {
                self.top_line_no -= 1;
              }
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'j' {
              if self.top_line_no < self.max_lines - max_lines_screen {
                self.top_line_no += 1;
                WindowMessageResponse::JustRedraw
              } else {
                WindowMessageResponse::DoNothing
              }
            } else if key_press.key == '0' {
              self.top_line_no = 0;
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'G' {
              self.top_line_no = self.max_lines - max_lines_screen;
              WindowMessageResponse::JustRedraw
            } else {
              WindowMessageResponse::DoNothing
            }
          },
          Mode::Url | Mode::Search | Mode::Link => {
            if key_press.is_enter() && self.input.len() > 0 {
              let new_url = if self.mode == Mode::Search {
                "http://frogfind.de/?q=".to_string() + &self.input
              } else if self.mode == Mode::Link {
                let link_index = self.input.parse::<usize>().unwrap();
                let url = self.url.as_ref().unwrap();
                let mut link;
                if link_index < self.links.len() {
                  link = self.links[link_index].clone();
                  if link.chars().count() >= 2 {
                    //remove the quotes
                    link = link.substring(1, link.len() - 1).to_string();
                  }
                  if link.starts_with("/") {
                    link = get_base_url(&url) + &link;
                  } else if !link.starts_with("http://") && !link.starts_with("https://") {
                    link = url.clone() + if url.ends_with("/") { "" } else { "/" } + &link;
                  }
                } else {
                  return WindowMessageResponse::DoNothing
                }
                link
              } else {
                //if Mode::Url
                self.input.clone()
              };
              if let Some(text) = get(&new_url) {
                self.url = Some(new_url.clone());
                self.top_line_no = 0;
                self.top_level_nodes = parse(&text);
                self.input = String::new();
                self.calc_page();
                self.mode = Mode::Normal;
                WindowMessageResponse::JustRedraw
              } else {
                WindowMessageResponse::DoNothing
              }
            } else if key_press.is_escape() {
              let is_link_mode = self.mode == Mode::Link;
              self.mode = Mode::Normal;
              if is_link_mode {
                self.calc_page();
              }
              self.input = String::new();
              WindowMessageResponse::JustRedraw
            } else if key_press.is_backspace() && self.input.len() > 0 {
              self.input = self.input.remove_last();
              WindowMessageResponse::JustRedraw
            } else if (self.mode == Mode::Link && key_press.key.is_ascii_digit() && self.input.len() < 10) || (self.mode != Mode::Link && key_press.is_regular()) {
              self.input += &key_press.key.to_string();
              WindowMessageResponse::JustRedraw
            } else {
              WindowMessageResponse::DoNothing
            }
          },
        }
      },
      _ => WindowMessageResponse::DoNothing,
    }
  }

  fn draw(&self, theme_info: &ThemeInfo) -> Vec<DrawInstructions> {
    let mut instructions = Vec::new();
    let max_lines_screen = (self.dimensions[1] - 4) / LINE_HEIGHT - 1;
    for p in &self.page {
      let line_no = (p.1 - 2) / LINE_HEIGHT;
      if line_no >= self.top_line_no && line_no < self.top_line_no + max_lines_screen {
        instructions.push(DrawInstructions::Text([p.0, p.1 - LINE_HEIGHT * self.top_line_no], vec!["nimbus-roman".to_string()], p.2.clone(), if p.3 { theme_info.top_text } else { theme_info.text }, theme_info.background, Some(1), Some(11)));
      }
    }
    //mode
    let mut bottom_text = self.mode.to_string() + ": ";
    if self.mode == Mode::Normal && self.dimensions[0] >= 500 {
      bottom_text += "u (url), s (search)";
      if self.url.is_some() {
        bottom_text += ", l (link), j (down), k (up)";
      }
    } else {
      bottom_text += &self.input;
    }
    instructions.push(DrawInstructions::Text([0, self.dimensions[1] - LINE_HEIGHT], vec!["nimbus-roman".to_string()], bottom_text, theme_info.text, theme_info.background, Some(1), Some(11)));
    instructions
  }

  fn title(&self) -> String {
    "Koxinga Browser".to_string()
  }

  fn subtype(&self) -> WindowLikeType {
    WindowLikeType::Window
  }

  fn ideal_dimensions(&self, _dimensions: Dimensions) -> Dimensions {
    [500, 410]
  }

  fn resizable(&self) -> bool {
    true
  }
}

impl KoxingaBrowser {
  pub fn new() -> Self {
    Default::default()
  }

  pub fn calc_page(&mut self) {
    self.page = Vec::new();
    self.links = Vec::new();
    let mut outputs = Vec::new();
    if self.top_level_nodes.len() > 0 {
      let html_index = self.top_level_nodes.iter().position(|n| n.tag_name == "html");
      if let Some(html_index) = html_index {
        for n in &self.top_level_nodes[html_index].children {
          if n.tag_name == "body" {
            outputs = n.to_output();
          }
        }
      }
    }
    let mut y = 2;
    let mut x = 2;
    let mut colour = false;
    let mut link_counter = 0;
    for o in outputs {
      //each char is width of 13
      let os = if let OutputType::Text(ref s) = o {
        let s = if s.starts_with(" ") {
          " ".to_string()
        } else {
          "".to_string()
        } + s.trim() + if s.ends_with(" ") {
          " "
        } else {
          ""
        };
        Some(s)
      } else if let OutputType::StartLink(link) = &o {
        colour = true;
        if self.mode == Mode::Link {
          self.links.push(link.to_string());
          let s = link_counter.to_string() + ":";
          link_counter += 1;
          if self.mode == Mode::Link {
            Some(s)
          } else {
            None
          }
        } else {
          None
        }
      } else {
        None
      };
      if let Some(s) = os {
        //leading and trailing whitespace is probably a mistake
        let mut line = String::new();
        let mut start_x = x;
        for c in s.chars() {
          if x + 12 > self.dimensions[0] {
            //full line, add draw instruction
            self.page.push((start_x, y, line, colour));
            line = String::new();
            x = 2;
            start_x = x;
            y += LINE_HEIGHT;
          }
          line += &c.to_string();
          x += 12;
        }
        if line.len() > 0 {
          self.page.push((start_x, y, line, colour));
        }
      }
      if o == OutputType::Newline {
        x = 2;
        y += LINE_HEIGHT;
      } else if o == OutputType::EndLink {
        colour = false;
      }
    }
    self.max_lines = (y - 2) / LINE_HEIGHT;
  }
}

pub fn main() {
  listen(KoxingaBrowser::new());
}
