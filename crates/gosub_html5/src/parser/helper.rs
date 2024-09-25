use crate::node::HTML_NAMESPACE;
use crate::parser::{ActiveElement, Html5Parser, Scope};
use crate::tokenizer::token::Token;
use gosub_shared::document::DocumentHandle;
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::document::Document;
use gosub_shared::traits::document::DocumentFragment;
use gosub_shared::traits::node::{ElementDataType, Node, TextDataType};
use crate::node::nodeimpl::NodeImpl;

const ADOPTION_AGENCY_OUTER_LOOP_DEPTH: usize = 8;
const ADOPTION_AGENCY_INNER_LOOP_DEPTH: usize = 3;

#[derive(Debug)]
pub enum InsertionPositionMode<D: Document<C>, C: CssSystem, NodeId> {
    LastChild {
        handle: DocumentHandle<D, C>,
        parent: NodeId,
    },
    Sibling {
        handle: DocumentHandle<D, C>,
        parent: NodeId,
        before: NodeId,
    },
}

pub enum BookMark<NodeId> {
    Replace(NodeId),
    InsertAfter(NodeId),
}

impl<'chars, D, C> Html5Parser<'chars, D, C>
where
    D: Document<C>,
    C: CssSystem,
    <<D as Document<C>>::Node as Node<C>>::ElementData: ElementDataType<C, Document = D>,
    <<<D as Document<C>>::Node as Node<C>>::ElementData as ElementDataType<C>>::DocumentFragment:
        DocumentFragment<C, Document = D>,
{
    fn find_position_in_active_format(&self, node_id: NodeId) -> Option<usize> {
        self.active_formatting_elements
            .iter()
            .position(|&x| x == ActiveElement::Node(node_id))
    }

    fn find_position_in_open_element(&self, node_id: NodeId) -> Option<usize> {
        self.open_elements.iter().position(|&x| x == node_id)
    }

    fn find_format_element_index(&self, subject: &str) -> Option<(usize, NodeId)> {
        self.active_formatting_elements
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, &node_id)| {
                if let ActiveElement::Node(node_id) = node_id {
                    let node = get_node_by_id!(self.document, node_id);
                    if get_element_data!(node).name() == subject {
                        Some((i, node_id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    }

    fn find_further_block_index(&self, format_ele_position: usize) -> Option<(usize, NodeId)> {
        self.open_elements
            .iter()
            .enumerate()
            .skip(format_ele_position)
            .find_map(|(i, &node_id)| {
                let node = get_node_by_id!(self.document, node_id);
                if get_element_data!(node).is_special() {
                    Some((i, node_id))
                } else {
                    None
                }
            })
    }

    pub fn insert_element_helper(&mut self, node: D::Node, position: InsertionPositionMode<D, C, NodeId>) -> NodeId {
        match position {
            InsertionPositionMode::Sibling { handle, parent, before } => {
                let parent_node = get_node_by_id!(handle, parent);
                let position = parent_node.children().iter().position(|&x| x == before);
                let mut_handle = &mut handle.clone();
                let id = mut_handle.get_mut().register_node_at(node, parent, position).clone();
                
                
                id
            }
            InsertionPositionMode::LastChild { handle, parent } => {
                let mut_handle = &mut handle.clone();
                let id = mut_handle.get_mut().register_node_at(node, parent, None);
                
                id
            }
        }
    }

    pub fn insert_text_helper(&mut self, position: InsertionPositionMode<D, C, NodeId>, token: &Token) {
        match position {
            InsertionPositionMode::Sibling { handle, parent, before } => {
                let parent_node = get_node_by_id!(handle, parent);
                let position = parent_node.children().iter().position(|&x| x == before);
                match position {
                    None | Some(0) => {
                        let node = self.create_node(token, HTML_NAMESPACE);
                        let mut_handle = &mut handle.clone();
                        mut_handle.get_mut().register_node_at(node, parent, position);
                    }
                    Some(index) => {
                        let last_node_id = parent_node.children()[index - 1];
                        let mut_handle = &mut handle.clone();
                        let mut last_node = get_node_by_id!(mut_handle, last_node_id);

                        if last_node.is_text_node() {
                            let data = get_text_data_mut!(&mut last_node);
                            data.value_mut().push_str(&token.to_string());
                            return;
                        }

                        let node = self.create_node(token, HTML_NAMESPACE);
                        let mut_handle = &mut handle.clone();
                        mut_handle.get_mut().register_node_at(node, parent, Some(index));
                    }
                }
            }
            InsertionPositionMode::LastChild { handle, parent } => {
                let parent_node = get_node_by_id!(handle, parent);
                if let Some(&last_node_id) = parent_node.children().last() {
                    let mut_handle = &mut handle.clone();
                    let mut last_node = get_node_by_id!(mut_handle, last_node_id);

                    if last_node.is_text_node() {
                        let data = get_text_data_mut!(&mut last_node);
                        data.value_mut().push_str(&token.to_string());
                        return;
                    };

                    let node = self.create_node(token, HTML_NAMESPACE);
                    let mut_handle = &mut handle.clone();
                    mut_handle.get_mut().register_node_at(node, parent, None);
                    return;
                }

                let node = self.create_node(token, HTML_NAMESPACE);
                let mut_handle = &mut handle.clone();
                mut_handle.get_mut().register_node_at(node, parent, None);
            }
        }
    }

    pub fn insert_html_element(&mut self, token: &Token) -> NodeId {
        self.insert_element_from_token(token, None, Some(HTML_NAMESPACE))
    }

    pub fn insert_foreign_element(&mut self, token: &Token, namespace: &str) -> NodeId {
        self.insert_element_from_token(token, None, Some(namespace))
    }

    pub fn insert_element_from_token(
        &mut self,
        token: &Token,
        override_node: Option<NodeId>,
        namespace: Option<&str>,
    ) -> NodeId {
        let mut node = self.create_node(token, namespace.unwrap_or(HTML_NAMESPACE));
        // add CSS classes from class attribute in element
        // e.g., <div class="one two three">
        // TODO: this will be refactored later in ElementAttributes to do this
        // when inserting a "class" attribute. Similar to "id" to attach it to the DOM
        // named_id_list. Although this will require some shared pointers

        
        
        
        
        if node.is_element_node() {
            // let mut_handle = &mut self.document.clone();
            // let mut node = get_node_by_id!(mut_handle, node.id());

            let data = get_element_data_mut!(&mut node);
            if let Some(class_string) = data.attributes().get("class") {
                let class_string = class_string.clone();

                data.add_class(&class_string.clone());
            }
        }

        self.insert_element(node, override_node)
    }

    pub fn insert_element_from_node(&mut self, org_node: &D::Node, override_node: Option<NodeId>) -> NodeId {
        // Create a node, but without children and push it onto the open elements stack (if needed)
        let new_node = org_node.clone();

        if new_node.is_element_node() {
            let mut_handle = &mut self.document.clone();
            let mut new_node = get_node_by_id!(mut_handle, new_node.id());

            let data = get_element_data_mut!(&mut new_node);
            if let Some(class_string) = data.attributes().get("class") {
                let class_string = class_string.clone();
                data.add_class(&class_string);
            }
        }
        self.insert_element(new_node, override_node)
    }

    pub fn insert_element(&mut self, node: D::Node, override_node: Option<NodeId>) -> NodeId {

        let insert_position = self.appropriate_place_insert(override_node);
        let node_id = self.insert_element_helper(node, insert_position);

        //     if parser not created as part of html fragment parsing algorithm
        //       pop the top element queue from the relevant agent custom element reactions stack (???)

        self.open_elements.push(node_id);
        node_id
    }

    pub fn insert_doctype_element(&mut self, token: &Token) {
        let node = self.create_node(token, HTML_NAMESPACE);
        self.document.get_mut().register_node_at(node, NodeId::root(), None);
    }

    pub fn insert_document_element(&mut self, token: &Token) {
        let node = self.create_node(token, HTML_NAMESPACE);
        let node_id = self.document.get_mut().register_node_at(node, NodeId::root(), None);

        self.open_elements.push(node_id);
    }

    pub fn insert_comment_element(&mut self, token: &Token, insert_position: Option<NodeId>) {
        let node = self.create_node(token, HTML_NAMESPACE);
        if let Some(position) = insert_position {
            self.document.get_mut().register_node_at(node, position, None);
            return;
        }

        // let node_id = self.document.get_mut().register_node(node);
        let insert_position = self.appropriate_place_insert(None);
        self.insert_element_helper(node, insert_position);
    }

    pub fn insert_text_element(&mut self, token: &Token) {
        // Skip empty text nodes
        if let Token::Text { text, .. } = token {
            if text.is_empty() {
                return;
            }
        }

        let insertion_position = self.appropriate_place_insert(None);
        // TODO, for text element, if the insertion_position is Document, should not do next step.
        self.insert_text_helper(insertion_position, token);
    }

    // @todo: where is the fragment case handled? (sub step 4: https://html.spec.whatwg.org/multipage/parsing.html#appropriate-place-for-inserting-a-node)
    pub fn appropriate_place_insert(&self, override_node: Option<NodeId>) -> InsertionPositionMode<D, C, NodeId> {
        let current_node = current_node!(self);
        let element_data = get_element_data!(current_node);
        let target_id = override_node.unwrap_or(current_node.id());

        if !(self.foster_parenting && ["table", "tbody", "thead", "tfoot", "tr"].contains(&element_data.name())) {
            if element_data.name() == "template" && element_data.is_namespace(HTML_NAMESPACE) {
                if let Some(template_fragment) = element_data.template_contents() {
                    return InsertionPositionMode::LastChild {
                        handle: template_fragment.handle(),
                        parent: target_id,
                    };
                }
            } else {
                return InsertionPositionMode::LastChild {
                    handle: self.document.clone(),
                    parent: target_id,
                };
            }
        }
        let mut iter = self.open_elements.iter().rev().peekable();
        while let Some(node_id) = iter.next() {
            let node = get_node_by_id!(self.document, *node_id);
            let element_data = get_element_data!(node);

            if element_data.name() == "template" {
                if let Some(template_fragment) = element_data.template_contents() {
                    return InsertionPositionMode::LastChild {
                        handle: template_fragment.handle(),
                        parent: *node_id,
                    };
                }
            } else if element_data.name() == "table" {
                if let Some(parent_id) = node.parent_id() {
                    return InsertionPositionMode::Sibling {
                        handle: self.document.clone(),
                        parent: parent_id,
                        before: *node_id,
                    };
                }
                // TODO has some question? can reached?
                return InsertionPositionMode::LastChild {
                    handle: self.document.clone(),
                    parent: *(*iter.peek().unwrap()),
                };
            }
        }
        InsertionPositionMode::LastChild {
            handle: self.document.clone(),
            parent: *self.open_elements.first().unwrap(),
        }
    }

    pub fn adoption_agency_algorithm(&mut self, token: &Token) {
        // step 1
        let subject = match token {
            Token::StartTag { name, .. } | Token::EndTag { name, .. } => name,
            _ => panic!("un reached"),
        };
        let current_node = current_node!(self);
        // let current_node_id = current_node.id();
        let current_data = get_element_data!(current_node);

        // step 2
        if current_data.name() == *subject
            && current_data.is_namespace(HTML_NAMESPACE)
            && self.find_position_in_active_format(current_node.id()).is_none()
        {
            self.open_elements.pop();
            return;
        }

        // step 3
        let mut outer_loop_counter = 0;

        // step 4
        loop {
            // step 4.1
            if outer_loop_counter >= ADOPTION_AGENCY_OUTER_LOOP_DEPTH {
                return;
            }

            // step 4.2
            outer_loop_counter += 1;

            // step 4.3
            let (format_elem_idx, format_elem_node_id) = match self.find_format_element_index(subject) {
                None => {
                    return self.handle_in_body_any_other_end_tag(subject);
                }
                Some((idx, node_id)) => (idx, node_id),
            };

            let format_node = get_node_by_id!(self.document, format_elem_node_id);
            let format_element_data = get_element_data!(format_node);
            let format_ele_stack_position = match self.open_elements.iter().rposition(|&x| x == format_elem_node_id) {
                // step 4.4
                None => {
                    self.parse_error("not found format_element_node in open_elements");
                    self.active_formatting_elements.remove(format_elem_idx);
                    return;
                }
                Some(idx) => idx,
            };

            // step 4.5
            if !self.is_in_scope(format_element_data.name(), HTML_NAMESPACE, Scope::Regular) {
                self.parse_error("format_element_node not in regular scope");
                return;
            }

            // step 4.6
            if format_elem_node_id != current_node.id() {
                self.parse_error("format_element_node not current_node");
            }

            // step 4.7
            let (further_block_idx, further_block_node_id) =
                match self.find_further_block_index(format_ele_stack_position) {
                    // step 4.8
                    None => {
                        self.open_elements.truncate(format_ele_stack_position);
                        self.active_formatting_elements.remove(format_elem_idx);
                        return;
                    }
                    Some((idx, node_id)) => (idx, node_id),
                };

            // step 4.9
            let common_ancestor = self.open_elements[format_ele_stack_position - 1];

            // step 4.10
            let mut bookmark_node_id = BookMark::Replace(format_elem_node_id);

            // step 4.11
            let mut node_id;
            let mut last_node_id = further_block_node_id;
            let mut node_idx = further_block_idx;

            // step 4.12
            let mut inner_loop_counter = 0;

            // step 4.13
            loop {
                // step 4.13.1
                inner_loop_counter += 1;

                // step 4.13.2
                node_idx -= 1;
                node_id = self.open_elements[node_idx];

                // step 4.13.3
                if node_id == format_elem_node_id {
                    break;
                }

                // step 4.13.4
                if inner_loop_counter > ADOPTION_AGENCY_INNER_LOOP_DEPTH {
                    self.find_position_in_active_format(node_id)
                        .map(|position| self.active_formatting_elements.remove(position));
                    self.open_elements.remove(node_idx);
                    continue;
                }
                // step 4.13.5
                let Some(node_active_position) = self.find_position_in_active_format(node_id) else {
                    self.open_elements.remove(node_idx);
                    continue;
                };

                // step 4.13.6
                let element_node = get_node_by_id!(self.document, node_id);
                let element_data = get_element_data!(element_node);

                let replacement_node = D::new_element_node(
                    self.document.clone(),
                    element_data.name(),
                    Some(element_data.namespace()),
                    element_data.attributes().clone(),
                    element_node.location(),
                );
                let replace_node_id = self.document.get_mut().register_node(replacement_node);

                self.active_formatting_elements[node_active_position] = ActiveElement::Node(replace_node_id);

                self.open_elements[node_idx] = replace_node_id;

                node_id = replace_node_id;

                // step 4.13.7
                if last_node_id == further_block_node_id {
                    bookmark_node_id = BookMark::InsertAfter(node_id);
                }

                // step 4.13.8
                self.document.get_mut().detach_node(last_node_id);
                self.document.get_mut().attach_node(last_node_id, replace_node_id, None);

                // step 4.13.9
                last_node_id = node_id;
            }

            // step 4.14
            self.document.get_mut().detach_node(last_node_id);
            let insert_position = self.appropriate_place_insert(Some(common_ancestor));
            
            
            let node = get_node_by_id!(self.document, last_node_id);
            
            
            self.insert_element_helper(todo!(), insert_position);

            // step 4.15
            let new_format_node = D::new_element_node(
                self.document.clone(),
                format_element_data.name(),
                Some(format_element_data.namespace()),
                format_element_data.attributes().clone(),
                format_node.location(),
            );

            // step 4.16
            let new_node_id = self.document.get_mut().register_node(new_format_node);

            let further_block_node = get_node_by_id!(self.document, further_block_node_id);
            for child in further_block_node.children() {
                self.document.get_mut().relocate_node(*child, new_node_id);
            }

            // step 4.17
            self.document
                .get_mut()
                .attach_node(new_node_id, further_block_node_id, None);

            // step 4.18
            match bookmark_node_id {
                BookMark::Replace(current) => {
                    let index = self.find_position_in_active_format(current).expect("node not found");
                    self.active_formatting_elements[index] = ActiveElement::Node(new_node_id);
                }
                BookMark::InsertAfter(previous) => {
                    let index = self.find_position_in_active_format(previous).expect("node not found") + 1;
                    self.active_formatting_elements
                        .insert(index, ActiveElement::Node(new_node_id));
                    let position = self.find_position_in_active_format(format_elem_node_id);
                    self.active_formatting_elements.remove(position.unwrap());
                }
            }

            // step 4.19
            self.open_elements.retain(|x| x != &format_elem_node_id);
            let position = self.find_position_in_open_element(further_block_node_id).unwrap();
            self.open_elements.insert(position + 1, new_node_id);
        }
    }
}
