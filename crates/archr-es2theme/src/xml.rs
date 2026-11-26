//! The small part of XML an EmulationStation theme uses: elements,
//! attributes and text. Comments, declarations and a byte order mark are
//! skipped.
use quick_xml::events::Event;

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// The document's root element.
pub fn parse(source: &str) -> Result<Node, String> {
    let mut reader = quick_xml::Reader::from_str(source.trim_start_matches('\u{feff}'));
    let mut stack = vec![Node::default()];
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("XML at byte {}: {e}", reader.buffer_position()))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                let mut node = Node {
                    name: String::from_utf8_lossy(tag.name().as_ref()).into_owned(),
                    ..Node::default()
                };
                for attr in tag.attributes().flatten() {
                    let value = attr
                        .unescape_value()
                        .map(|v| v.into_owned())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&attr.value).into_owned());
                    node.attrs.push((
                        String::from_utf8_lossy(attr.key.as_ref()).into_owned(),
                        value,
                    ));
                }
                if empty {
                    stack.last_mut().expect("root").children.push(node);
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err("XML closes more elements than it opens".into());
                }
                let node = stack.pop().expect("open element");
                stack.last_mut().expect("root").children.push(node);
            }
            Event::Text(text) => {
                let text = text
                    .unescape()
                    .map(|t| t.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(&text).into_owned());
                stack.last_mut().expect("root").text.push_str(&text);
            }
            Event::CData(data) => {
                let text = String::from_utf8_lossy(&data).into_owned();
                stack.last_mut().expect("root").text.push_str(&text);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err("XML ends with elements still open".into());
    }
    let root = stack.pop().expect("root");
    root.children
        .into_iter()
        .next()
        .ok_or_else(|| "XML has no element".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_elements_attributes_and_text_past_comments() {
        let root = super::parse(
            "\u{feff}<!-- c --><?xml version=\"1.0\"?><theme a=\"1\"><x>t &amp; u</x><y/></theme>",
        )
        .unwrap();
        assert_eq!(root.name, "theme");
        assert_eq!(root.attr("a"), Some("1"));
        assert_eq!(root.children[0].text, "t & u");
        assert_eq!(root.children[1].name, "y");
    }
}
