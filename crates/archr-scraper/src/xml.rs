//! Minimal element tree for the small ScreenScraper XML replies.
use quick_xml::{events::Event, Reader};

#[derive(Default, Debug)]
pub struct El {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub kids: Vec<El>,
}

impl El {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    pub fn child(&self, name: &str) -> Option<&El> {
        self.kids.iter().find(|kid| kid.name == name)
    }
    /// Moves the first child called `name` out of the tree.
    pub fn take(mut self, name: &str) -> Option<El> {
        let at = self.kids.iter().position(|kid| kid.name == name)?;
        Some(self.kids.swap_remove(at))
    }
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a El> {
        self.kids.iter().filter(move |kid| kid.name == name)
    }
}

/// Root element of `source`, or `None` when it is not well-formed XML.
pub fn parse(source: &[u8]) -> Option<El> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().expand_empty_elements = true;
    let mut stack = vec![El::default()];
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(start) => {
                let mut el = El {
                    name: String::from_utf8_lossy(start.name().as_ref()).into_owned(),
                    ..El::default()
                };
                for attr in start.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                    el.attrs
                        .push((key, attr.unescape_value().ok()?.into_owned()));
                }
                stack.push(el);
            }
            Event::End(_) => {
                let el = stack.pop()?;
                stack.last_mut()?.kids.push(el);
            }
            Event::Text(text) => stack.last_mut()?.text.push_str(&text.unescape().ok()?),
            Event::CData(data) => stack
                .last_mut()?
                .text
                .push_str(&String::from_utf8_lossy(&data)),
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    let root = stack.pop().filter(|_| stack.is_empty())?;
    root.kids.into_iter().next()
}
