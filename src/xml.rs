use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use xmlparser::Error as XmlError;

#[derive(Debug)]
pub enum XmlParseError {
    /// **Note**: Sadly, the error type can only be a string because xmlparser
    /// returns all errors as strings. There is an open PR to fix
    /// this deficiency, but since the XML parsing is only needed for
    /// hot-reloading and compiling, it doesn't matter that much.
    ParseError(XmlError),
    /// Invalid hierarchy close tags, i.e `<app></p></app>`
    MalformedHierarchy(String, String),
}

/// Represents one XML node tag
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct XmlNode {
    /// Type of the node
    pub node_type: String,
    /// Attributes of an XML node (note: not yet filtered and / or broken into function arguments!)
    pub attributes: BTreeMap<String, String>,
    /// Direct children of this node
    pub children: Vec<XmlNode>,
    /// String content of the node, i.e the "Hello" in `<p>Hello</p>`
    pub text: Option<String>,
}

pub fn get_all_nodes_in_subtree<'a>(xml: &'a [XmlNode], node_type_searched: &'static str) -> Vec<&'a XmlNode> {
    get_all_nodes_in_subtree_comparator(
        xml, 
        |node| node.node_type.as_str() == node_type_searched,
    )
}


pub fn get_all_nodes_in_tree<'a>(xml: &'a [XmlNode]) -> Vec<&'a XmlNode> {
    get_all_nodes_in_subtree_comparator(xml, |_| true)
}

pub fn get_all_nodes_in_subtree_comparator<'a, F: Fn(&XmlNode) -> bool>(
    xml: &'a [XmlNode], 
    search_fn: F,
) -> Vec<&'a XmlNode> {
    let mut nodes = Vec::new();
    get_all_nodes_in_subtree_comparator_internal(xml, &search_fn, &mut nodes);
    nodes
}

pub fn get_all_nodes_in_subtree_comparator_internal<'a, F: Fn(&XmlNode) -> bool>(
    xml: &'a [XmlNode],
    search_fn: &F,
    target: &mut Vec<&'a XmlNode>,
) {
    let mut found_nodes = xml
        .iter()
        .filter(|node| (search_fn)(node))
        .collect::<Vec<_>>();

    for xml_node in xml.iter() {
        get_all_nodes_in_subtree_comparator_internal(&xml_node.children, search_fn, &mut found_nodes);
    }

    target.extend(found_nodes.into_iter());
}

impl XmlNode {
    pub fn get_all_children_of_type<'a>(&'a self, node_type: &str) -> Vec<&'a XmlNode> {
        self.children
            .iter()
            .filter(|n| n.node_type.as_str() == node_type)
            .collect()
    }
}

pub fn parse_xml_string(xml: &str, log: &mut Vec<String>) -> Result<Vec<XmlNode>, XmlParseError> {
    use xmlparser::ElementEnd::*;
    use xmlparser::Token::*;
    use xmlparser::Tokenizer;

    use self::XmlParseError::*;

    let mut root_node = XmlNode::default();

    // Search for "<?xml" and "?>" tags and delete them from the XML
    let mut xml = xml.trim();
    if xml.starts_with("<?") {
        let pos = xml
            .find("?>")
            .ok_or(MalformedHierarchy("<?xml".into(), "?>".into()))?;
        xml = &xml[(pos + 2)..];
    }

    // Delete <!doctype if necessary
    let mut xml = xml.trim();
    if xml.starts_with("<!") {
        let pos = xml
            .find(">")
            .ok_or(MalformedHierarchy("<!doctype".into(), ">".into()))?;
        xml = &xml[(pos + 1)..];
    }

    let tokenizer = Tokenizer::from_fragment(xml, 0..xml.len());

    // In order to insert where the item is, let's say
    // [0 -> 1st element, 5th-element -> node]
    // we need to trach the index of the item in the parent.
    let mut current_hierarchy: Vec<usize> = Vec::new();

    for token in tokenizer {
        let token = token.map_err(|e| ParseError(e))?;
        match token {
            ElementStart { local, .. } => {
                if let Some(current_parent) = get_item(&current_hierarchy, &mut root_node) {
                    let children_len = current_parent.children.len();
                    current_parent.children.push(XmlNode {
                        node_type: local.to_string(),
                        attributes: BTreeMap::new(),
                        children: Vec::new(),
                        text: None,
                    });
                    current_hierarchy.push(children_len);
                }
            }
            ElementEnd { end: Empty, .. } => {
                current_hierarchy.pop();
            }
            ElementEnd {
                end: Close(_, close_value),
                ..
            } => {
                let i = get_item(&current_hierarchy, &mut root_node);
                if let Some(last) = i {
                    if last.node_type != close_value.as_str() {
                        return Err(MalformedHierarchy(
                            close_value.to_string(),
                            last.node_type.clone(),
                        ));
                    }
                }
                current_hierarchy.pop();
            }
            Attribute { local, value, .. } => {
                if let Some(last) = get_item(&current_hierarchy, &mut root_node) {
                    // NOTE: Only lowercase the key ("local"), not the value!
                    last.attributes
                        .insert(local.to_string(), value.as_str().to_string());
                }
            }
            Text { text } => {
                let text = text.trim();
                if !text.is_empty() {
                    if let Some(last) = get_item(&current_hierarchy, &mut root_node) {
                        if let Some(s) = last.text.as_mut() {
                            s.push_str(text);
                        }
                        if last.text.is_none() {
                            last.text = Some(text.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(root_node.children)
}

/// Given a root node, traverses along the hierarchy, and returns a
/// mutable reference to the last child node of the root node
#[allow(trivial_casts)]
fn get_item<'a>(hierarchy: &[usize], root_node: &'a mut XmlNode) -> Option<&'a mut XmlNode> {
    let mut hierarchy = hierarchy.to_vec();
    hierarchy.reverse();
    let item = match hierarchy.pop() {
        Some(s) => s,
        None => return Some(root_node),
    };
    let node = root_node.children.get_mut(item)?;
    get_item_internal(&mut hierarchy, node)
}

fn get_item_internal<'a>(hierarchy: &mut Vec<usize>, root_node: &'a mut XmlNode) -> Option<&'a mut XmlNode> {
    if hierarchy.is_empty() {
        return Some(root_node);
    }
    let cur_item = match hierarchy.pop() {
        Some(s) => s,
        None => return Some(root_node),
    };
    let node = root_node.children.get_mut(cur_item)?;
    get_item_internal(hierarchy, node)
}