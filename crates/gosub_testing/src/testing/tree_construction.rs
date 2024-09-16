pub mod fixture;
mod generator;
pub(crate) mod parser;
pub mod result;

use generator::TreeOutputGenerator;
use gosub_html5::node::{HTML_NAMESPACE, MATHML_NAMESPACE, SVG_NAMESPACE};
use gosub_html5::parser::Html5ParserOptions;
use gosub_shared::byte_stream::{ByteStream, Config, Encoding, Location};
use gosub_shared::document::DocumentHandle;
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::document::{Document, DocumentBuilder};
use gosub_shared::traits::html5::{Html5Parser, ParserOptions};
use gosub_shared::types::{ParseError, Result};
use parser::{ScriptMode, TestSpec};
use result::TestResult;
use result::{ResultStatus, TreeLineResult};
use std::collections::HashMap;

/// Holds a single parser test
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Test {
    /// Filename of the test
    pub file_path: String,
    /// Line number of the test
    pub line: usize,
    /// The specification of the test provided in the test file
    pub spec: TestSpec,
    /// The document tree as found in the spec converted to an array
    pub document: Vec<String>,
}

impl Test {
    /// Returns the script modes that should be tested as an array
    pub fn script_modes(&self) -> &[bool] {
        match self.spec.script_mode {
            ScriptMode::ScriptOff => &[false],
            ScriptMode::ScriptOn => &[true],
            ScriptMode::Both => &[false, true],
        }
    }

    pub fn document_as_str(&self) -> &str {
        self.spec.document.as_str()
    }

    pub fn spec_data(&self) -> &str {
        self.spec.data.as_str()
    }
}

/// Harness is a wrapper to run tree-construction tests
#[derive(Debug)]
pub struct Harness {
    // Test that is currently being run
    test: Test,
    /// Next line in the document array
    next_document_line: usize,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    /// Generated a new harness instance. It uses a dummy test that is replaced when run_test is called
    #[must_use]
    pub fn new() -> Self {
        Self {
            test: Test::default(),
            next_document_line: 0,
        }
    }

    /// Runs a single test and returns the test result of that run
    pub fn run_test<P: Html5Parser<C>, C: CssSystem>(
        &mut self,
        test: Test,
        scripting_enabled: bool,
    ) -> Result<TestResult> {
        self.test = test;
        self.next_document_line = 0;

        let (actual_document, actual_errors) = self.do_parse::<P, C>(scripting_enabled)?;
        let result = self.generate_test_result::<P, C>(actual_document.clone(), &actual_errors);

        Ok(result)
    }

    /// Run the html5 parser and return the document tree and errors
    fn do_parse<P: Html5Parser<C>, C: CssSystem>(
        &mut self,
        scripting_enabled: bool,
    ) -> Result<(DocumentHandle<P::Document, C>, Vec<ParseError>)> {
        let options = <P::Options as ParserOptions>::new(scripting_enabled);
        let mut stream = ByteStream::new(
            Encoding::UTF8,
            Some(Config {
                cr_lf_as_one: true,
                replace_cr_as_lf: true,
                replace_high_ascii: false,
            }),
        );
        stream.read_from_str(self.test.spec_data(), None);
        stream.close();

        let (document, parse_errors) =
            if let Some(fragment) = self.test.spec.document_fragment.clone() {
                self.parse_fragment::<P, C>(fragment, stream, options, Location::default())?
            } else {
                let document = <P::Document as Document<C>>::Builder::new_document(None);
                let parser_errors =
                    P::parse(&mut stream, DocumentHandle::clone(&document), Some(options))?;

                (document, parser_errors)
            };

        Ok((document, parse_errors))
    }

    fn parse_fragment<P: Html5Parser<C>, C: CssSystem>(
        &mut self,
        fragment: String,
        mut stream: ByteStream,
        options: P::Options,
        start_location: Location,
    ) -> Result<(DocumentHandle<P::Document, C>, Vec<ParseError>)> {
        // First, create a (fake) main document that contains only the fragment as node
        let main_document = <P::Document as Document<C>>::Builder::new_document(None);

        let mut main_document = main_document.clone();
        let (element, namespace) = if fragment.starts_with("svg ") {
            (
                fragment.strip_prefix("svg ").unwrap().to_string(),
                SVG_NAMESPACE,
            )
        } else if fragment.starts_with("math ") {
            (
                fragment.strip_prefix("math ").unwrap().to_string(),
                MATHML_NAMESPACE,
            )
        } else {
            (fragment, HTML_NAMESPACE)
        };

        let doc_clone = DocumentHandle::clone(&main_document);

        let mut doc = main_document.get_mut();

        let node = <P::Document as Document<C>>::new_element_node(
            doc_clone.clone(),
            element.as_str(),
            Some(namespace),
            HashMap::new(),
            start_location,
        );

        let context_node_id = doc.register_node(node);

        doc.attach_node(context_node_id, NodeId::root(), None);

        let context_node = doc.node_by_id(context_node_id).unwrap().clone();

        let document = <P::Document as Document<C>>::Builder::new_document_fragment(&context_node);

        let parser_errors = P::parse_fragment(
            &mut stream,
            DocumentHandle::clone(&document),
            &context_node,
            Some(options),
            start_location,
        )?;

        Ok((document, parser_errors))
    }

    /// Retrieves the next line from the spec document
    fn next_line(&mut self) -> Option<String> {
        let mut line = String::new();
        let mut is_multi_line_text = false;

        loop {
            // If we are at the end of the document, we return None
            if self.next_document_line >= self.test.document.len() {
                return None;
            }

            // Get the next line
            let tmp = self.test.document[self.next_document_line].clone();
            self.next_document_line += 1;

            // If we have a starting quote, but not an ending quote, we are a multi-line text
            if tmp.starts_with('\"') && !tmp.ends_with('\"') {
                is_multi_line_text = true;
            }

            // Add the line to the current line if we are a multiline
            if is_multi_line_text {
                line.push_str(&tmp);
            } else {
                line = tmp;
            }

            // Only break if we're in a multi-line text, and we found the ending double-quote
            if is_multi_line_text && line.ends_with('\"') {
                break;
            }

            // if we are not a multi-line, we can just break
            if !is_multi_line_text {
                break;
            }

            // Otherwise we continue with the next line (multi-line text)
        }

        Some(line.to_string())
    }

    fn generate_test_result<P: Html5Parser<C>, C: CssSystem>(
        &mut self,
        document: DocumentHandle<P::Document, C>,
        _parse_errors: &[ParseError],
    ) -> TestResult {
        let mut result = TestResult::default();

        let generator = TreeOutputGenerator::new(document);
        let actual = generator.generate();

        let mut line_idx = 1;
        for actual_line in actual {
            let mut status = ResultStatus::Success;

            let expected_line = self.next_line();
            match expected_line.clone() {
                Some(expected_line) => {
                    if actual_line != expected_line {
                        status = ResultStatus::Mismatch;
                    }
                }
                None => {
                    status = ResultStatus::Missing;
                }
            }

            result.tree_results.push(TreeLineResult {
                index: line_idx,
                result: status,
                expected: expected_line.unwrap_or_default().to_string(),
                actual: actual_line.to_string(),
            });
            line_idx += 1;
        }

        // Check if we have additional lines and if so, add as errors
        while let Some(expected_line) = self.next_line() {
            result.tree_results.push(TreeLineResult {
                index: line_idx,
                result: ResultStatus::Additional,
                expected: expected_line,
                actual: String::new(),
            });
            line_idx += 1;
        }

        result
    }
}
