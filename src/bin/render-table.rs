use gosub_renderer::render_table::{Table, TableCell, TableRow};

fn main() {
    let mut t = Table::new()
        .with_summary("This is the summary of the table")
        .with_caption("This is the caption of the table")
        .with_bordered(true)
        .with_cell_spacing(1)
        .with_cell_padding(1)
        .with_width(100)
        .with_height(100);

    t.add_head_row(TableRow {
        cells: vec![
            TableCell {
                content: "Head 1".to_string(),
                colspan: 1,
                rowspan: 1,
            },
            TableCell {
                content: "Head 2".to_string(),
                colspan: 1,
                rowspan: 1,
            },
        ]
    });

    t.add_body_row(TableRow {
        cells: vec![
            TableCell {
                content: "Body 1".to_string(),
                colspan: 1,
                rowspan: 1,
            },
            TableCell {
                content: "Body 2".to_string(),
                colspan: 1,
                rowspan: 1,
            },
        ]
    });

    t.add_footer_row(TableRow {
        cells: vec![
            TableCell {
                content: "Footer 1".to_string(),
                colspan: 1,
                rowspan: 1,
            },
            TableCell {
                content: "Footer 2".to_string(),
                colspan: 1,
                rowspan: 1,
            },
        ]
    });


    println!("{}", t.render(80));
}


// fn main_old() -> Result<()> {
//     let url = std::env::args()
//         .nth(1)
//         .or_else(|| {
//             println!("Usage: render-table <url>");
//             exit(1);
//         })
//         .unwrap();
//
//     // Fetch the html from the url
//     let response = ureq::get(&url).call().map_err(Box::new)?;
//     if !response.status() == 200 {
//         println!("could not get url. Status code {}", response.status());
//         exit(1);
//     }
//     let html = response.into_string()?;
//
//     let mut stream = ByteStream::new();
//     stream.read_from_str(&html, Some(Encoding::UTF8));
//     stream.set_confidence(Confidence::Certain);
//     stream.close();
//
//     let document = DocumentBuilder::new_document(None);
//     let parse_errors = Html5Parser::parse_document(&mut stream, Document::clone(&document), None)?;
//     for e in parse_errors {
//         println!("Parse Error: {}", e.message);
//     }
//
//     let doc = document.get();
//     let body_node_id = find_node_id(document.clone(), document.get().get_root(), "body");
//     let body_node = doc.get_node_by_id(body_node_id.unwrap());
//
//     if body_node.is_none() {
//         println!("[No Body Found]");
//         return Ok(());
//     }
//
//     recursive_display_node(&document.get(), body_node.unwrap(), 0);
//
//     // for node_id in body_node.unwrap().children.iter() {
//     //     let doc = document.get();
//     //     let node = doc.get_node_by_id(*node_id).unwrap();
//     //     display_node(&document.get(), node);
//     // }
//
//     Ok(())
// }
//
// fn find_node_id(handle: DocumentHandle,node: &Node, name: &str) -> Option<NodeId> {
//     let doc = handle.get();
//     if let NodeData::Element(element) = &node.data {
//         println!("Want: {}, Found: {}", name, element.name);
//         if element.name.eq(name) {
//             return Some(node.id.clone());
//         }
//     }
//
//     for node_id in node.children.iter() {
//         match find_node_id(handle.clone(), doc.get_node_by_id(*node_id).unwrap(), name) {
//             None => {}
//             Some(id) => {
//                 return Some(id);
//             }
//         }
//     }
//
//     None
// }
//
// fn get_node<'a>(document: &'a Document, parent: &'a Node, name: &'a str) -> Option<&'a Node> {
//     for id in &parent.children {
//         match document.get_node_by_id(*id) {
//             None => {}
//             Some(node) => {
//                 if node.name.eq(name) {
//                     return Some(node);
//                 }
//             }
//         }
//     }
//     None
// }
//
// fn recursive_display_node(document: &Document, node: &Node, level: usize) {
//     let prefix = "   ".repeat (level);
//
//     if let NodeData::Text(text) = &node.data {
//         if !text.value().eq("\n") {
//             println!("{}{}", prefix, text.value());
//         }
//     }
//
//     if let NodeData::Element(element) = &node.data {
//         println!("{}<{}>", prefix, element.name);
//     }
//
//     for child_id in &node.children {
//         if let Some(child) = document.get_node_by_id(*child_id) {
//             recursive_display_node(document, child, level + 1);
//         }
//     }
//
//     if let NodeData::Element(element) = &node.data {
//         println!("{}</{}>", prefix, element.name);
//     }
// }
//
// fn display_node(document: &Document, node: &Node) {
//     if let NodeData::Text(text) = &node.data {
//         if !text.value().eq("\n") {
//             println!("{}", text.value());
//         }
//     }
//
//     if let NodeData::Element(element) = &node.data {
//         if element.name.eq("table") {
//             println!("Table found!");
//         }
//     }
//
//     for child_id in &node.children {
//         if let Some(child) = document.get_node_by_id(*child_id) {
//             display_node(document, child);
//         }
//     }
// }
//
//
