use std::vec::Vec;
use std::vec;
use std::fmt;
use std::boxed::Box;
use std::collections::HashMap;

//use ming_wm_lib::logging::log;
use ming_wm_lib::window_manager_types::{ DrawInstructions, WindowLike, WindowLikeType };
use ming_wm_lib::messages::{ WindowMessage, WindowMessageResponse };
use ming_wm_lib::utils::{ get_rest_of_split, Substring };
use ming_wm_lib::framebuffer_types::{ Dimensions, RGBColor };
use ming_wm_lib::themes::ThemeInfo;
use ming_wm_lib::fonts::{ CachedFontCharGetter, measure_text, measure_text_with_cache };
use ming_wm_lib::ipc::listen;

mod http;
use crate::http::HttpClient;
mod xml;
use crate::xml::{ parse, remove_quotes, Form, FormSubmitMethod, Node, OutputType };
mod url;
use crate::url::Url;

const LINE_HEIGHT: usize = 18;
const BAND_HEIGHT: usize = 19;

#[derive(Default, PartialEq)]
enum State {
  #[default]
  None,
  Maybeg,
}

#[derive(Default, PartialEq)]
enum Mode {
  #[default]
  Normal,
  Url,
  Link,
  Search,
  FormSubmit,
  FormInput, //(input elements)
}

impl fmt::Display for Mode {
  fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
    fmt.write_str(match self {
      Mode::Normal => "NORMAL",
      Mode::Url => "URL",
      Mode::Link => "LINK",
      Mode::Search => "SEARCH",
      Mode::FormSubmit => "FORM SUBMIT",
      Mode::FormInput => "FORM INPUT",
    })?;
    Ok(())
  }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Subtype {
  Text,
  Link,
  TextInput,
  Button,
  //
}

impl Subtype {
  pub fn to_rgb(&self, theme_info: &ThemeInfo) -> RGBColor {
    match self {
      Self::Text => theme_info.text,
      Self::Link => theme_info.alt_text,
      Self::TextInput => theme_info.alt_secondary,
      Self::Button => theme_info.alt_secondary,
    }
  }

  //
  pub fn is_one_off(&self) -> bool {
    //button, text input, stuff that we don't expect other subtypes to be in (well, buttons might, but whatever)
    self == &Subtype::TextInput || self == &Subtype::Button
  }
}

#[derive(Default)]
struct KoxingaBrowser {
  client: HttpClient,
  dimensions: Dimensions,
  fonts: Vec<String>,
  mode: Mode,
  state: State,
  max_lines: usize,
  top_line_no: usize,
  url: Option<Url>,
  input: String,
  maybe_num: Option<usize>,
  links: Vec<String>,
  forms: Vec<Form>,
  form_inputs: HashMap<(usize, String), String>, //form #+input name, input value
  title: Option<String>,
  top_level_nodes: Vec<Box<Node>>,
  page: Vec<(usize, usize, String, Subtype)>, //x, y, text, subtype
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
        self.calc_page(false);
        WindowMessageResponse::JustRedraw
      },
      WindowMessage::KeyPress(key_press) => {
        match self.mode {
          Mode::Normal => {
            let max_lines_screen = (self.dimensions[1] - 2) / LINE_HEIGHT - 2;
            if self.state == State::Maybeg && key_press.key != 'g' {
              self.state = State::None;
            }
            if key_press.key == 'u' {
              self.mode = Mode::Url;
              self.input = self.url.clone().unwrap_or(Url::new(String::new())).to_string();
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'l' && self.url.is_some() {
              self.mode = Mode::Link;
              self.calc_page(false);
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'f' {
              self.mode = Mode::FormSubmit;
              self.calc_page(false);
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 's' {
              self.mode = Mode::Search;
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'f' && self.url.is_some() {
              self.mode = Mode::FormSubmit;
              self.calc_page(false);
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'i' && self.url.is_some() {
              self.mode = Mode::FormInput;
              self.calc_page(false);
              WindowMessageResponse::JustRedraw
            } else if key_press.key == 'j' || key_press.key == 'k' {
              let num = self.maybe_num.unwrap_or(1);
              self.maybe_num = None;
              if key_press.key == 'j' {
                let max_top = self.max_lines - max_lines_screen + 1;
                if self.top_line_no + num < max_top {
                  self.top_line_no += num;
                  WindowMessageResponse::JustRedraw
                } else if self.top_line_no != max_top {
                  self.top_line_no = max_top;
                  WindowMessageResponse::JustRedraw
                } else {
                  WindowMessageResponse::DoNothing
                }
              } else {
                if self.top_line_no > num {
                  self.top_line_no -= num;
                  WindowMessageResponse::JustRedraw
                } else if self.top_line_no > 0 {
                  self.top_line_no = 0;
                  WindowMessageResponse::JustRedraw
                } else {
                  WindowMessageResponse::DoNothing
                }
              }
            } else if key_press.key == 'g' {
              if self.state == State::Maybeg {
                self.top_line_no = 0;
                WindowMessageResponse::JustRedraw
              } else {
                self.state = State::Maybeg;
                WindowMessageResponse::DoNothing
              }
            } else if key_press.key == 'G' {
              self.top_line_no = self.max_lines - max_lines_screen + 1;
              WindowMessageResponse::JustRedraw
            } else if key_press.key.is_ascii_digit() {
              self.maybe_num = Some(self.maybe_num.unwrap_or(0) * 10 + key_press.key.to_digit(10).unwrap() as usize);
              WindowMessageResponse::DoNothing
            } else if self.maybe_num.is_some() {
              self.maybe_num = None;
              WindowMessageResponse::DoNothing
            } else {
              WindowMessageResponse::DoNothing
            }
          },
          //all modes besides normal, which use the bottom input
          _ => {
            if key_press.is_enter() && self.input.len() > 0 {
              if self.mode == Mode::Url || self.mode == Mode::Link {
                let new_url = if self.mode == Mode::Link {
                  self.mode = Mode::Normal;
                  let link_index = self.input.parse::<usize>().unwrap();
                  let mut url = self.url.as_ref().unwrap().clone();
                  if link_index < self.links.len() {
                    let mut link = self.links[link_index].clone();
                    if link.chars().count() >= 2 {
                      link = remove_quotes(link);
                    }
                    if link.starts_with("/") {
                      url.pop_to_root();
                      url.append(link);
                    } else if !link.starts_with("http://") && !link.starts_with("https://") {
                      if !link.starts_with("?") && !link.starts_with("#") {
                        url.pop();
                      }
                      url.append(link);
                    } else {
                      url = Url::new(link);
                    }
                  } else {
                    return WindowMessageResponse::DoNothing
                  }
                  url
                } else {
                  //if Mode::Url
                  Url::new(self.input.clone())
                };
                if let Some(text) = self.client.get(&new_url.to_string()) {
                  self.change_url(new_url, text);
                  WindowMessageResponse::JustRedraw
                } else {
                  WindowMessageResponse::DoNothing
                }
              } else if self.mode == Mode::FormSubmit || self.mode == Mode::FormInput {
                if self.mode == Mode::FormInput {
                  //this shouldn't be able to panic I hope
                  //get_rest_of_split may return an empty string, but it won't panic
                  let mut splitted = self.input.split("=");
                  let first = splitted.next().unwrap();
                  let input_value = get_rest_of_split(&mut splitted, Some("="));
                  let mut first_splitted = first.split(",");
                  let form_count = first_splitted.next().unwrap().parse::<usize>();
                  //form count is not a number
                  if form_count.is_err() {
                    self.input = String::new();
                    return WindowMessageResponse::JustRedraw;
                  }
                  let form_count = form_count.unwrap();
                  let input_name = get_rest_of_split(&mut first_splitted, None); //I mean, there shouldn't be a comma in the input name, right?
                  //insert overwrites
                  //todo: check if exists first
                  self.form_inputs.insert((form_count, input_name), input_value);
                  self.input = String::new();
                  self.calc_page(false);
                  WindowMessageResponse::JustRedraw
                } else {
                  //form submit
                  let form_index = self.input.parse::<usize>().unwrap();
                  if form_index < self.forms.len() {
                    let form_info = &self.forms[form_index];
                    match form_info.method {
                      FormSubmitMethod::Get => {
                        //construct url to redirect to
                        let mut form_url = Url::new_maybe_relative(form_info.action.clone(), self.url.clone().unwrap());
                        //key aka name attr
                        for key in &form_info.input_names {
                          if let Some(value) = self.form_inputs.get(&(form_index, key.clone())) {
                            form_url.append_query(&key, value);
                          }
                        }
                        //log(&format!("{}", form_url.clone()));
                        if let Some(text) = self.client.get(&form_url.to_string()) {
                          self.change_url(form_url, text);
                          WindowMessageResponse::JustRedraw
                        } else {
                          WindowMessageResponse::DoNothing
                        }
                      },
                      FormSubmitMethod::Post => {
                        //todo. maybe later
                        //
                        WindowMessageResponse::DoNothing
                      },
                    }
                  } else {
                    WindowMessageResponse::DoNothing
                  }
                }
              } else {
                //Mode::Search
                for p in &self.page {
                  let line_no = (p.1 - 2) / LINE_HEIGHT;
                  if line_no > self.top_line_no {
                    //p.2 is the text
                    if p.2.contains(&self.input) {
                      self.top_line_no = line_no;
                      return WindowMessageResponse::JustRedraw;
                    }
                  }
                }
                WindowMessageResponse::DoNothing
              }
            } else if key_press.is_escape() {
              self.mode = Mode::Normal;
              self.input = String::new();
              if self.mode == Mode::Link || self.mode == Mode::FormSubmit || self.mode == Mode::FormInput {
                self.calc_page(false);
              }
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
    let max_lines_screen = (self.dimensions[1] - 2) / LINE_HEIGHT - 2;
    for p in &self.page {
      let line_no = (p.1 - 2) / LINE_HEIGHT;
      if line_no >= self.top_line_no + max_lines_screen {
        break;
      } else if line_no >= self.top_line_no && line_no < self.top_line_no + max_lines_screen {
        let subtype = p.3;
        let top_left = [p.0, p.1 - LINE_HEIGHT * self.top_line_no];
        let bg_colour = if subtype == Subtype::TextInput || subtype == Subtype::Button {
          Some(theme_info.alt_background)
        } else {
          None
        };
        if let Some(bg_colour) = bg_colour {
          let width = measure_text(&self.fonts, &p.2, Some(1)).width;
          instructions.push(DrawInstructions::Rect([top_left[0] - 2, top_left[1] - 2], [width, LINE_HEIGHT], bg_colour));
        }
        instructions.push(DrawInstructions::Text(top_left, self.fonts.clone(), p.2.clone(), subtype.to_rgb(&theme_info), bg_colour.unwrap_or(theme_info.background), Some(1), None));
      }
    }
    //mode, in a blue band
    instructions.push(DrawInstructions::Rect([0, self.dimensions[1] - BAND_HEIGHT * 2], [self.dimensions[0], BAND_HEIGHT], theme_info.top));
    let mut bottom_text = self.mode.to_string() + ": ";
    if self.mode == Mode::Normal && self.dimensions[0] >= 300 {
      bottom_text += "u(rl)";
      if self.url.is_some() && self.dimensions[0] >= 640 {
        bottom_text += ", s(earch), l(ink), i(nput), f(orm), j, k";
      }
    } else if self.mode == Mode::FormInput && self.dimensions[0] > 500 {
      bottom_text += "syntax is eg \"0,inputname=input value\"";
    }
    instructions.push(DrawInstructions::Text([0, self.dimensions[1] - LINE_HEIGHT * 2], vec!["nimbus-romono".to_string()], bottom_text, theme_info.top_text, theme_info.top, Some(1), Some(11)));
    instructions.push(DrawInstructions::Text([0, self.dimensions[1] - LINE_HEIGHT], vec!["nimbus-romono".to_string()], self.input.clone(), theme_info.text, theme_info.background, Some(1), Some(11)));
    instructions
  }

  fn title(&self) -> String {
    let t = if let Some(title) = &self.title {
      format!(": {}", title)
    } else {
      " Browser".to_string()
    };
    "Koxinga".to_string() + &t
  }

  fn subtype(&self) -> WindowLikeType {
    WindowLikeType::Window
  }

  fn ideal_dimensions(&self, _dimensions: Dimensions) -> Dimensions {
    [650, 410]
  }

  fn resizable(&self) -> bool {
    true
  }
}

impl KoxingaBrowser {
  pub fn new(fonts: Vec<String>) -> Self {
    let mut s: Self = Default::default();
    s.fonts = fonts;
    s
  }

  pub fn change_url(&mut self, new_url: Url, text: String) {
    self.url = Some(new_url);
    self.top_line_no = 0;
    self.top_level_nodes = parse(&text);
    self.input = String::new();
    self.calc_page(true);
    self.mode = Mode::Normal;
  }

  pub fn calc_page(&mut self, new_page: bool) {
    self.title = None;
    self.page = Vec::new();
    self.links = Vec::new();
    self.forms = Vec::new();
    if new_page {
      self.form_inputs = HashMap::new();
    }
    let mut outputs = Vec::new();
    if self.top_level_nodes.len() > 0 {
      let html_index = self.top_level_nodes.iter().position(|n| n.tag_name == "html");
      if let Some(html_index) = html_index {
        for n in &self.top_level_nodes[html_index].children {
          if n.tag_name == "head" {
            //look for title, if any
            for hn in &n.children {
              if hn.tag_name == "title" && hn.children.len() > 0 && hn.children[0].text_node {
                self.title = Some(hn.children[0].tag_name.clone());
              }
            }
          } else if n.tag_name == "body" {
            outputs = n.to_output();
            break;
          }
        }
      }
    }
    let mut y = 2;
    let mut x = 2;
    let mut indent = 0;
    let mut line_count = 0;
    let mut link_counter = 0;
    let mut form_counter = 0;
    let mut subtype = Subtype::Text;
    let mut fc_getter = CachedFontCharGetter::new(81); //all eng alpha + numbers + 19
    for o in outputs {
      //each char is width of 13
      let output_string = if let OutputType::Text(ref s) = o {
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
        subtype = Subtype::Link;
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
      } else if let OutputType::Form(form) = &o {
        //yeah, in future properly render the submit button
        subtype = Subtype::Button;
        self.forms.push(form.clone());
        let t = if self.mode == Mode::FormSubmit {
          form_counter.to_string() + ":"
        } else {
          String::new()
        } + "Submit Form";
        form_counter += 1;
        Some(t)
      } else if let OutputType::TextInput(name) = &o {
        subtype = Subtype::TextInput;
        if new_page {
          self.form_inputs.insert((form_counter, name.to_string()), String::new());
        }
        let t = if self.mode == Mode::FormInput || self.mode == Mode::FormSubmit {
          format!("{},{}={}", form_counter.to_string(), name, self.form_inputs.get(&(form_counter, name.to_owned())).unwrap())
        } else {
          name.to_owned()
        };
        Some(t)
      } else {
        None
      };
      if let Some(s) = output_string {
        //leading and trailing whitespace is probably a mistake
        let mut line = String::new();
        if x == 2 {
          x += indent;
        }
        let mut start_x = x;
        for c in s.chars() {
          let c_width = measure_text_with_cache(&mut fc_getter, &self.fonts, &c.to_string(), None).width + 1; //+1 for horiz spacing
          if x + c_width > self.dimensions[0] {
            //full line, add draw instruction
            self.page.push((start_x, y, line, subtype));
            line = String::new();
            x = 2 + indent;
            start_x = x;
            y += LINE_HEIGHT;
            line_count += 1;
          }
          line += &c.to_string();
          x += c_width;
        }
        if line.len() > 0 {
          self.page.push((start_x, y, line, subtype));
        }
        if subtype.is_one_off() {
          //so button and textinput subtypes don't persist
          //really we should allow multiple subtypes at once or something, idk
          //but this is fine for now
          subtype = Subtype::Text;
        }
      }
      if let OutputType::Indent(space) = o {
        indent = space;
        if x == 2 {
          x += indent;
        }
      }
      if o == OutputType::Newline {
        x = 2;
        y += LINE_HEIGHT;
        line_count += 1;
      } else if o == OutputType::EndLink {
        subtype = Subtype::Text;
      }
    }
    self.max_lines = line_count;
  }
}

pub fn main() {
  listen(KoxingaBrowser::new(vec!["nimbus-roman".to_string(), "shippori-mincho".to_string()]));
}
