use std::vec::Vec;
use std::boxed::Box;
use std::collections::HashMap;

use ming_wm_lib::utils::Substring;

//use ming_wm_lib::logging::log;

//try to be xhtml compliant?

//fuck these mfers. self close with a FUCKING slash man.
//<meta> is bad, <meta/> is good!!
const SELF_CLOSING: [&'static str; 9] = ["link", "meta", "input", "img", "br", "hr", "source", "track", "!DOCTYPE"];

//not all of them, eg there is intentionally no div
const BLOCK_LEVEL: [&'static str; 13] = ["p", "br", "li", "tr", "header", "footer", "section", "h1", "h2", "h3", "h4", "h5", "h6"];

pub const REPLACE: [(&'static str, &'static str); 7] = [
  ("&nbsp;", " "),
  ("&#x27;", "'"),
  ("&quot;", "\""),
  ("&#x2F;", "/"),
  ("&gt;", ">"),
  ("&lt;", "<"),
  ("&amp;", "&"),
];

pub const URL_REPLACE: [(&'static str, &'static str); 12] = [
  ("%22", "\""),
  ("%2B", "+"),
  ("%2C", ","),
  ("%2D", "-"),
  ("%2F", "/"),
  ("%3A", ":"),
  ("%5C", "\\"),
  ("%5B", "["),
  ("%5D", "]"),
  ("%5F", "_"),
  ("%7B", "{"),
  ("%7D", "}"),
];

fn is_whitespace(c: char) -> bool {
  c == ' ' || c == '\x09'
}

pub fn handle_escaped(s: &str, replace_list: Vec<(&str, &str)>, inverse: bool) -> String {
  let mut s = s.to_string();
  for rp in replace_list {
    if !inverse {
      s = s.replace(rp.0, rp.1);
    } else {
      s = s.replace(rp.1, rp.0);
    }
  }
  s
}

pub fn remove_quotes(s: String) -> String {
  //todo: remove only if quotes
  let s_len = s.len();
  if s_len > 1 {
    s.substring(1, s.len() - 1).to_string()
  } else {
    s //length is 0 or 1, can't strip no quotes...
  }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FormSubmitMethod {
  Get,
  Post,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Form {
  pub action: Option<String>, //url, if None, defaults to same url
  pub method: FormSubmitMethod,
  pub input_names: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum OutputType {
  StartLink(String), //url
  EndLink,
  Text(String),
  Newline,
  //only support one per line, once indented, will keep being indented until overriden, for now
  Indent(usize),
  TextInput(String, String), //name, default value
  Form(Form),
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Node {
  //for text nodes, tag_name is the content
  pub tag_name: String,
  pub attributes: HashMap<String, String>,
  pub children: Vec<Box<Node>>,
  pub text_node: bool,
}

impl Node {
  pub fn to_output(&self) -> Vec<OutputType> {
    let mut output = Vec::new();
    let mut link = false;
    let mut form = None;
    let mut input_names = Vec::new();
    if Some(&"\"true\"".to_string()) == self.attributes.get("aria-hidden") {
      return output;
    } else if self.text_node {
      output.push(OutputType::Text(handle_escaped(&self.tag_name.clone(), REPLACE.to_vec(), false)));
      return output;
    } else if self.tag_name == "script" || self.tag_name == "style" {
      //ignore script and style tags
      return output;
    } else if self.tag_name == "li" {
      output.push(OutputType::Text("-".to_string()));
    } else if let Some(href) = self.attributes.get("href") {
      link = true;
      //check if href is ddg link that fucks us over in lite.duckduckgo.com
      // //duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.merriam%2Dwebster.com%2Fdictionary%2Ftest&amp;rut=f86942690bea49b300b8ae8d470dbbe18ad217aded1750804e3f33a95da21cf2
      let href = if href.starts_with("\"//duckduckgo.com/l/?uddg=") {
        //todo: only take from &amp onward
        "\"".to_string() + &handle_escaped(&href.chars().skip(26).collect::<String>().split("&amp;").next().unwrap(), URL_REPLACE.to_vec(), false) + "\""
      } else {
        href.to_string()
      };
      output.push(OutputType::StartLink(href));
    } else if let Some(indent) = self.attributes.get("indent") {
      //non-standard indent attribute, basically just to support HN
      let indent = remove_quotes(indent.to_string());
      if let Ok(indent) = indent.parse::<usize>() {
        output.push(OutputType::Indent(indent * 32));
      }
    } else if self.tag_name == "input" || self.tag_name == "textarea" {
      if let Some(name) = self.attributes.get("name") {
        //unwrap_or is painful so compiler suggested map_or
        let input_type = remove_quotes(self.attributes.get("type").map_or("\"text\"".to_string(), |v| v.to_string()));
        if input_type == "text" || input_type == "search" || input_type == "password" || input_type == "hidden" {
          let default_value = remove_quotes(self.attributes.get("value").map_or(String::new(), |v| v.to_string()));
          output.push(OutputType::TextInput(remove_quotes(name.to_string()), default_value));
        }
      }
    } else if self.tag_name == "button" {
      if Some(&"\"submit\"".to_string()) == self.attributes.get("type") {
        //we only care about submit buttons with names since we need to send that on form POST submit
        if let Some(name) = self.attributes.get("name") {
          let default_value = remove_quotes(self.attributes.get("value").map_or(String::new(), |v| v.to_string()));
          output.push(OutputType::TextInput(remove_quotes(name.to_string()), default_value));
        }
      }
    } else if self.tag_name == "form" {
      let action = self.attributes.get("action");
      let method = if let Some(m) = self.attributes.get("method") {
        let m = remove_quotes(m.to_string()).to_lowercase();
        match m.as_str() {
          "post" => Some(FormSubmitMethod::Post),
          "get" => Some(FormSubmitMethod::Get),
          _ => None,
        }
      } else {
        Some(FormSubmitMethod::Get)
      };
      if let Some(method) = method {
        form = Some(Form {
          //wikipedia puts &amp; in the action url??? is that how its supposed to be? do I need to worry about href?
          action: if let Some(action) = action { Some(handle_escaped(&remove_quotes(action.to_string()), REPLACE.to_vec(), false)) } else { None },
          method,
          input_names: Vec::new(),
        });
      }
    }
    for c in &self.children {
      let children_output = c.to_output();
      if form.is_some() {
        for cc in &children_output {
          if let OutputType::TextInput(name, _) = cc {
            input_names.push(name.to_string());
          }
        }
      }
      output.extend(children_output);
    }
    if BLOCK_LEVEL.contains(&self.tag_name.as_str()) {
      output.push(OutputType::Newline);
    } else if link {
      output.push(OutputType::EndLink);
    } else if let Some(form) = form {
      let form = Form {
        action: form.action,
        method: form.method,
        input_names,
      };
      output.push(OutputType::Form(form));
    }
    output
  }
}

fn add_to_parent(top_level_nodes: &mut Vec<Box<Node>>, parent_location: &[usize], node: Node) -> usize {
  if parent_location.len() == 0 {
    top_level_nodes.push(Box::new(node));
    top_level_nodes.len() - 1
  } else {
    let mut parent_children = &mut top_level_nodes[parent_location[0]].children;
    for i in &parent_location[1..] {
      parent_children = &mut parent_children[*i].children;
    }
    let loc = parent_children.len();
    parent_children.push(Box::new(node));
    loc
  }
}

pub fn parse(xml_string: &str) -> Vec<Box<Node>> {
  let mut top_level_nodes = Vec::new();
  let mut chars = xml_string.chars().peekable();
  let mut parent_location: Vec<usize> = Vec::new(); //vec of indexes
  let mut recording_tag_name = false;
  let mut whitespace_only = true; //ignore leading whitespace on each line
  let mut attribute_name = String::new();
  let mut recording_attribute_value = false;
  let mut in_string = false;
  let mut quote_type = None;
  let mut current_node: Option<Node> = None;
  loop {
    let c = chars.next();
    if c.is_none() {
      break;
    }
    let c = c.unwrap();
    if let Some(ref mut n) = current_node {
      if n.tag_name == "!--" {
        //this is a comment... skip!
        current_node = None;
        recording_tag_name = false;
        let mut dash_count = 0;
        loop {
          let c2 = chars.next();
          if c2.is_none() {
            break;
          }
          let c2 = c2.unwrap();
          if c2 == '>' && dash_count == 2 {
            break;
          } else if c2 == '-' {
            dash_count += 1;
          } else {
            dash_count = 0;
          }
        }
      } else if (n.tag_name == "script" || n.tag_name == "style") && !n.text_node {
        //need to handle this carefully since < and > could be present
        let mut so_far = String::new();
        let loc = add_to_parent(&mut top_level_nodes, &parent_location, n.clone());
        parent_location.push(loc);
        //won't handle if </script> appears in a string
        loop {
          let c2 = chars.next();
          if c2.is_none() {
            break;
          }
          let c2 = c2.unwrap();
          so_far += &c2.to_string();
          let end_len = n.tag_name.len() + 3;
          if so_far.len() >= end_len {
            let end = so_far.chars().count();
            if so_far.substring(end - end_len, end) == "</".to_string() + &n.tag_name + ">" {
              current_node = None;
              let n2: Node = Node { text_node: true, tag_name: so_far.substring(0, end - end_len).to_string(), ..Default::default() };
              add_to_parent(&mut top_level_nodes, &parent_location, n2);
              parent_location.pop();
              recording_tag_name = false;
              break;
            }
          }
        }
      } else if (c == ' ' || c == '\n') && recording_tag_name && !n.text_node {
        recording_tag_name = false;
      } else if (c == '>' || (c == '/' && chars.peek().unwrap_or(&' ') == &'>') || (n.text_node && chars.peek().unwrap_or(&' ') == &'<')) && (!in_string || quote_type == Some(c)) {
        if n.text_node {
          n.tag_name += &c.to_string();
        }
        let loc = add_to_parent(&mut top_level_nodes, &parent_location, n.clone());
        if c == '>' && !SELF_CLOSING.contains(&n.tag_name.as_str()) {
          parent_location.push(loc);
        } else if c == '/' {
          chars.next();
        }
        //catch attributes like disabled with no = or value
        if attribute_name.len() > 0 && !recording_attribute_value {
          n.attributes.entry(attribute_name.clone()).insert_entry(String::new());
        }
        recording_tag_name = false;
        recording_attribute_value = false;
        attribute_name = String::new();
        current_node = None;
      } else if recording_tag_name {
        n.tag_name += &c.to_string();
      } else if c == ' ' && !in_string {
        //catch attributes like disabled with no = or value
        if attribute_name.len() > 0 && !recording_attribute_value {
          n.attributes.entry(attribute_name.clone()).insert_entry(String::new());
        } else if recording_attribute_value {
          //^this can just be an "else", not an "else if", probably
          recording_attribute_value = false;
        }
        attribute_name = String::new();
      } else if recording_attribute_value {
        if (c == '"' || c == '\'') && (quote_type == Some(c) || quote_type.is_none()) {
          in_string = *n.attributes.get(&attribute_name).unwrap() == "";
          quote_type = Some(c);
          if !in_string {
            quote_type = None;
          }
        }
        n.attributes.entry(attribute_name.clone()).and_modify(|s| *s += &c.to_string());
      } else if c == '=' {
        n.attributes.entry(attribute_name.clone()).insert_entry(String::new());
        recording_attribute_value = true;
      } else {
        attribute_name += &c.to_string();
      }
      //todo: record attributes
    } else if c == '<' {
      whitespace_only = false;
      if chars.peek().unwrap_or(&' ') == &'/' {
        parent_location.pop();
        //skip the rest of the </ >
        loop {
          let c2 = chars.next();
          if c2.is_none() || c2.unwrap() == '>' {
            break;
          }
        }
      } else {
        current_node = Some(Default::default());
        recording_tag_name = true;
      }
    } else if c == '\n' {
      whitespace_only = true;
    } else if !is_whitespace(c) || !whitespace_only {
      if !is_whitespace(c) {
        whitespace_only = false;
      }
      //text node
      let n: Node = Node { tag_name: c.to_string(), text_node: true, ..Default::default() };
      if chars.peek().unwrap_or(&' ') == &'<' {
        add_to_parent(&mut top_level_nodes, &parent_location, n);
      } else {
        recording_tag_name = true;
        current_node = Some(n);
      }
    }
  }
  top_level_nodes
}

#[test]
fn test_xml_parse() {
  let nodes = parse("<p>Woah <span id=\"spanner\">lorem ipsum</span> !!! no way</p>
<input name=\"in put\" disabled/>
<div>
  <a href=\"https://wikipedia.org\" title=12>Wikipedia</a>
  <p>Nested woah <b>woah</b></p>
</div>");
  assert!(nodes.len() == 3);
  assert!(nodes[0].tag_name == "p");
  assert!(nodes[0].children.len() == 3);
  assert!(nodes[0].children[0].tag_name == "Woah ");
  assert!(nodes[0].children[1].tag_name == "span");
  assert!(nodes[0].children[1].children[0].tag_name == "lorem ipsum");
  assert!(nodes[0].children[2].tag_name == " !!! no way");
  assert!(nodes[1].tag_name == "input");
  println!("{}", nodes[1].attributes.get("name").unwrap());
  assert!(nodes[1].attributes.get("name").unwrap() == "\"in put\"");
  assert!(nodes[2].tag_name == "div");
  assert!(nodes[2].children.len() == 2);
  assert!(nodes[2].children[0].tag_name == "a");
  assert!(nodes[2].children[0].attributes.get("href").unwrap() == "\"https://wikipedia.org\"");
  assert!(nodes[2].children[0].attributes.get("title").unwrap() == "12");
  assert!(nodes[2].children[0].children[0].tag_name == "Wikipedia");
  assert!(nodes[2].children[1].tag_name == "p");
  assert!(nodes[2].children[1].children[0].tag_name == "Nested woah ");
  assert!(nodes[2].children[1].children[1].tag_name == "b");
  assert!(nodes[2].children[1].children[1].children[0].tag_name == "woah");
}

#[test]
fn test_close_xml_parse() {
  let nodes = parse("<span> (<a>Hey</a>Woah)</span>");
  assert!(nodes[0].children[1].tag_name == "a");
  let nodes = parse("<a>ab</a> <span>woah</span>");
  assert!(nodes[2].tag_name == "span");
}

#[test]
fn test_style_script_xml_parse() {
  let nodes = parse("<p>a b c</p><style>. p ></style><b>woah</b>");
  assert!(nodes.len() == 3);
  assert!(nodes[0].tag_name == "p");
  assert!(nodes[1].tag_name == "style");
  assert!(nodes[1].children[0].tag_name == ". p >");
  //
}

#[test]
fn test_comments_xml_parse() {
  let nodes = parse("<p>test</p><!--comment <a>stallman forced me to do this</a><p>--><b> afterwards</b>");
  assert!(nodes.len() == 2);
  assert!(nodes[1].tag_name == "b");
  assert!(nodes[1].children[0].tag_name == " afterwards");
}

#[test]
fn test_weird_attr() {
  //weird order
  let nodes = parse("<input type=\"text\" disabled name=\"one\">");
  assert!(nodes[0].attributes.get("type").unwrap() == "\"text\"");
  assert!(nodes[0].attributes.get("disabled").is_some());
  assert!(nodes[0].attributes.get("name").unwrap() == "\"one\"");
  //newlines in tag and shit
  let nodes = parse("<input
  title=\"invalid text\"

  name=\"one\">");
  assert!(nodes[0].tag_name == "input");
  //assert!(nodes[0].attributes.get("title").unwrap() == "\"invalid text\"\n"); //current has newline at end I think (TODO: fix)
  //
  assert!(nodes[0].attributes.get("name").unwrap() == "\"one\"");
}

#[test]
fn test_form_parse_and_output() {
  let nodes = parse("<form method=\"get\" action=\"/test\">
  <div><input type=\"search\" name=\"search1\"></div>
  <label>Field 1:</label> test <input type=\"text\" name=\"field1\">
  <label>Field 2:</label> yeah <input type=\"text\" name=\"field2\">
</form>");
  assert!(nodes.len() == 1);
  assert!(nodes[0].tag_name == "form");
  assert!(nodes[0].children[0].children[0].tag_name == "input");
  assert!(nodes[0].children[1].tag_name == "label");
  assert!(nodes[0].children[3].tag_name == "input");
  assert!(nodes[0].children[4].tag_name == "label");
  //check .to_output()
  //
}

#[test]
fn test_strings_again() {
  let nodes = parse("<span data-value='woah\"cheeseburgers\"'>Nice</span>");
  assert!(nodes[0].attributes.get("data-value").unwrap() == "'woah\"cheeseburgers\"'");
  let nodes = parse("<span data-value=\"woah! ' cheeseburgers'\">Nice</span>");
  assert!(nodes[0].attributes.get("data-value").unwrap() == "\"woah! ' cheeseburgers'\"");
}


#[test]
fn test_real() {
  use std::fs::read_to_string;
  let nodes = parse(&read_to_string("./real_tests/wikipedia.html").unwrap());
  //println!("{:#?}", nodes);
  println!("{:?}", nodes[1].children[1].to_output());
  //println!("{}", nodes[12323233].children[1].tag_name);
}
