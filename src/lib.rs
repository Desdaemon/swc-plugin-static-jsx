use swc_core::ecma::ast::*;
use swc_core::ecma::visit::VisitMutWith;
use swc_core::plugin::errors::HANDLER;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

mod transform;
pub use transform::TransformVisitor;

#[plugin_transform]
pub fn process_transform(mut program: Program, meta: TransformPluginProgramMetadata) -> Program {
	let mut visitor: TransformVisitor = match meta.get_transform_plugin_config() {
		Some(config) => match serde_json::from_str(&config) {
			Ok(visitor) => visitor,
			Err(err) => {
				HANDLER.with(|handler| {
					handler
						.struct_err("[swc-plugin-static-jsx] Failed to parse config")
						.note(&err.to_string())
						.emit()
				});
				return program;
			}
		},
		None => Default::default(),
	};
	program.visit_mut_with(&mut visitor);
	program
}
