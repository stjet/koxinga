use std::vec::Vec;
use std::boxed::Box;
use std::collections::HashMap;

use ming_wm_lib::utils::Substring;

//try to be xhtml compliant?

//fuck these mfers. self close with a FUCKING slash man.
//<meta> is bad, <meta/> is good!!
const SELF_CLOSING: [&'static str; 9] = ["link", "meta", "input", "img", "br", "hr", "source", "track", "!DOCTYPE"];

fn is_whitespace(c: char) -> bool {
  c == ' ' || c == '\x09'
}

fn handle_escaped(s: &str) -> String {
  s.replace("&nbsp;", " ").replace("&#x27;", "'").to_string()
}

#[derive(PartialEq)]
pub enum OutputType {
  StartLink(String),
  EndLink,
  Text(String),
  Newline,
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
    if self.text_node {
      output.push(OutputType::Text(handle_escaped(&self.tag_name.clone())));
      return output;
    } else if self.tag_name == "script" || self.tag_name == "style" {
      //ignore script and style tags
      return output;
    } else if let Some(href) = self.attributes.get("href") {
      link = true;
      output.push(OutputType::StartLink(href.to_string()));
    }
    for c in &self.children {
      output.extend(c.to_output());
    }
    if self.tag_name == "p" || self.tag_name == "br" || self.tag_name == "li" || self.tag_name == "tr" {
      output.push(OutputType::Newline);
    } else if link {
      output.push(OutputType::EndLink);
    }
    output
  }
}

fn add_to_parent(top_level_nodes: &mut Vec<Box<Node>>, parent_location: &Vec<usize>, node: Node) -> usize {
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
              let mut n2: Node = Default::default();
              n2.text_node = true;
              n2.tag_name = so_far.substring(0, end - end_len).to_string();
              add_to_parent(&mut top_level_nodes, &parent_location, n2);
              parent_location.pop();
              recording_tag_name = false;
              break;
            }
          }
        }
      } else if c == ' ' && recording_tag_name && !n.text_node {
        recording_tag_name = false;
      } else if c == '>' || (c == '/' && chars.peek().unwrap_or(&' ') == &'>') || (n.text_node && chars.peek().unwrap_or(&' ') == &'<') {
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
      } else if c == ' ' && !in_string && recording_attribute_value {
        //catch attributes like disabled with no = or value
        if attribute_name.len() > 0 && !recording_attribute_value {
          n.attributes.entry(attribute_name.clone()).insert_entry(String::new());
        }
        recording_attribute_value = false;
        attribute_name = String::new();
      } else if recording_attribute_value {
        if c == '"' {
          in_string = *n.attributes.get(&attribute_name).unwrap() == "";
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
          if c2.is_none() {
            break;
          } else if c2.unwrap() == '>' {
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
      let mut n: Node = Default::default();
      n.tag_name = c.to_string();
      n.text_node = true;
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

//more tests 100% needed. yoink from news.ycombinator.com and en.wikipedia.org
