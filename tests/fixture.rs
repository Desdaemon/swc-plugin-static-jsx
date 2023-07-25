use std::path::PathBuf;

use swc_core::ecma::parser::{EsConfig, Syntax};
use swc_core::ecma::transforms::testing::test_fixture;
use swc_core::ecma::visit::as_folder;
use swc_plugin_static_jsx::TransformVisitor;
use testing::fixture;

fn syntax() -> Syntax {
    Syntax::Es(EsConfig {
        jsx: true,
        ..Default::default()
    })
}

#[fixture("tests/fixtures/**/input.js")]
fn tests(input: PathBuf) {
    let output = input.with_file_name("output.js");

    test_fixture(
        syntax(),
        &|_| as_folder(TransformVisitor::default()),
        &input,
        &output,
        Default::default(),
    );
}
