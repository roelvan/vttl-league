//! Minimal XML tree for SOAP responses. Namespaces are dropped (local names only).
//! A repeated element is just several children with the same name, so "one or many"
//! is handled by `children(name)` everywhere.

use anyhow::{Result, anyhow, bail};
use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// Text of a direct child, or "" when missing.
    pub fn s(&self, name: &str) -> String {
        self.child(name).map(|c| c.text.trim().to_string()).unwrap_or_default()
    }
    pub fn i(&self, name: &str) -> i64 {
        self.s(name).parse().unwrap_or(0)
    }
    pub fn b(&self, name: &str) -> bool {
        self.s(name) == "true"
    }
}

pub fn parse(xml: &str) -> Result<Node> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Node> = vec![Node::default()];
    loop {
        match reader.read_event()? {
            Event::Start(e) => stack.push(Node {
                name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                ..Default::default()
            }),
            Event::Empty(e) => {
                let n = Node {
                    name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                    ..Default::default()
                };
                stack.last_mut().unwrap().children.push(n);
            }
            Event::Text(t) => stack.last_mut().unwrap().text.push_str(&t.unescape()?),
            Event::CData(t) => stack
                .last_mut()
                .unwrap()
                .text
                .push_str(&String::from_utf8_lossy(&t)),
            Event::End(_) => {
                let n = stack.pop().ok_or_else(|| anyhow!("unbalanced xml"))?;
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow!("unbalanced xml"))?
                    .children
                    .push(n);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        bail!("unbalanced xml");
    }
    Ok(stack.pop().unwrap())
}

/// Returns the first element inside SOAP Body. A SOAP fault becomes an error.
pub fn soap_body(xml: &str) -> Result<Node> {
    let root = parse(xml)?;
    let body = root
        .child("Envelope")
        .and_then(|e| e.child("Body"))
        .ok_or_else(|| anyhow!("no SOAP body"))?;
    let first = body
        .children
        .first()
        .ok_or_else(|| anyhow!("empty SOAP body"))?;
    if first.name == "Fault" {
        return Err(Fault { code: first.s("faultcode"), message: first.s("faultstring") }.into());
    }
    Ok(first.clone())
}

/// A SOAP fault from TabT, e.g. code "22" = no permission for this upload.
#[derive(Debug)]
pub struct Fault {
    pub code: String,
    pub message: String,
}
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "VTTL fault {}: {}", self.code, self.message)
    }
}
impl std::error::Error for Fault {}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_and_repeated() {
        let x = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="a" xmlns:ns1="b"><SOAP-ENV:Body><ns1:R><ns1:E><ns1:V>1</ns1:V></ns1:E></ns1:R></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
        let r = soap_body(x).unwrap();
        assert_eq!(r.children("E").count(), 1);
        assert_eq!(r.child("E").unwrap().i("V"), 1);
    }

    #[test]
    fn fault() {
        let x = r#"<Envelope><Body><Fault><faultcode>22</faultcode><faultstring>bad &amp; wrong</faultstring></Fault></Body></Envelope>"#;
        let e = soap_body(x).unwrap_err();
        assert!(e.to_string().contains("bad & wrong"));
        assert_eq!(e.downcast_ref::<Fault>().unwrap().code, "22");
    }
}
