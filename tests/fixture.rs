use std::path::PathBuf;

use swc_core::common::{chain, Mark};
use swc_core::ecma::parser::{EsConfig, Syntax};
use swc_core::ecma::transforms::base::resolver;
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
		&|_| {
			let visitor = if let Ok(file) = std::fs::read(input.with_file_name("config.json")) {
				serde_json::from_slice(&file).expect("Failed to read config")
			} else {
				TransformVisitor::default()
			};
			chain!(resolver(Mark::new(), Mark::new(), false), as_folder(visitor))
		},
		&input,
		&output,
		Default::default(),
	);
}
