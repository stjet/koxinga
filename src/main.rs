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

#[derive(Default, PartialEq)]
enum Mode {
  #[default]
  Normal,
  Url,
}

impl fmt::Display for Mode {
  fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
    fmt.write_str(match self {
      Mode::Normal => "NORMAL",
      Mode::Url => "URL",
    })?;
    Ok(())
  }
}

#[derive(Default)]
struct KoxingaBrowser {
  dimensions: Dimensions,
  mode: Mode,
  top_line_no: usize,
  url_input: String,
  top_level_nodes: Vec<Box<Node>>,
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
        WindowMessageResponse::JustRedraw
      },
      WindowMessage::KeyPress(key_press) => {
        match self.mode {
          Mode::Normal => {
            if key_press.key == 'u' {
              self.mode = Mode::Url;
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'k' {
              if self.top_line_no > 0 {
                self.top_line_no -= 1;
              }
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'j' {
              self.top_line_no += 1;
              WindowMessageResponse::JustRedraw
            } else {
              WindowMessageResponse::DoNothing
            }
          },
          Mode::Url => {
            if key_press.key == '𐘂' { //the enter key
              if let Some(text) = get(&self.url_input) {
                self.top_line_no = 0;
                self.top_level_nodes = parse(&text);
                self.mode = Mode::Normal;
              }
            } else if key_press.key == '𐘃' { //escape key
              self.mode = Mode::Normal;
            } else if key_press.key == '𐘁' { //backspace
              if self.url_input.len() > 0 {
                self.url_input = self.url_input.remove_last();
              } else {
                return WindowMessageResponse::DoNothing;
              }
            } else {
              self.url_input += &key_press.key.to_string();
            }
            WindowMessageResponse::JustRedraw
          },
        }
      },
      _ => WindowMessageResponse::DoNothing,
    }
  }

  fn draw(&self, theme_info: &ThemeInfo) -> Vec<DrawInstructions> {
    let mut instructions = Vec::new();
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
    let mut colour = theme_info.text;
    for o in outputs {
      //each char is width of 13
      match o {
        OutputType::Text(s) => {
          //leading and trailing whitespace is probably a mistake
          let s = s.trim();
          let mut line = String::new();
          let mut start_x = x;
          for c in s.chars() {
            if x + 14 > self.dimensions[0] {
              //full line, add draw instruction
              let line_no = (y - 2) / LINE_HEIGHT;
              if line_no >= self.top_line_no {
                instructions.push(DrawInstructions::Text([start_x, y - LINE_HEIGHT * self.top_line_no], vec!["nimbus-roman".to_string()], line, colour, theme_info.background, Some(1), Some(14)));
              }
              line = String::new();
              x = 2;
              start_x = x;
              y += LINE_HEIGHT;
            }
            line += &c.to_string();
            x += 14;
          }
          let line_no = (y - 2) / LINE_HEIGHT;
          if line.len() > 0 && line_no >= self.top_line_no {
            instructions.push(DrawInstructions::Text([start_x, y - LINE_HEIGHT * self.top_line_no], vec!["nimbus-roman".to_string()], line, colour, theme_info.background, Some(1), Some(14)));
          }
        },
        OutputType::Newline => {
          x = 2;
          y += LINE_HEIGHT;
        },
        OutputType::StartLink(_) => {
          colour = theme_info.top_text;
        },
        OutputType::EndLink => {
          colour = theme_info.text;
        },
      };
    }
    //mode
    let mut bottom_text = self.mode.to_string() + ": ";
    if self.mode == Mode::Url {
      bottom_text += &self.url_input;
    }
    instructions.push(DrawInstructions::Text([0, self.dimensions[1] - LINE_HEIGHT], vec!["nimbus-roman".to_string()], bottom_text, theme_info.text, theme_info.background, Some(1), Some(12)));
    instructions
  }

  fn title(&self) -> String {
    "Koxinga Browser".to_string()
  }

  fn subtype(&self) -> WindowLikeType {
    WindowLikeType::Window
  }

  fn ideal_dimensions(&self, _dimensions: Dimensions) -> Dimensions {
    [410, 410]
  }

  fn resizable(&self) -> bool {
    true
  }
}

impl KoxingaBrowser {
  pub fn new() -> Self {
    Default::default()
  }
}

pub fn main() {
  listen(KoxingaBrowser::new());
}
