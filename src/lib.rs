use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{as_folder, FoldWith};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

mod transform;
pub use transform::TransformVisitor;

#[plugin_transform]
pub fn process_transform(program: Program, meta: TransformPluginProgramMetadata) -> Program {
    let visitor: TransformVisitor = match meta.get_transform_plugin_config() {
        Some(config) => serde_json::from_str(&config).expect("Failed to parse config"),
        None => Default::default(),
    };
    program.fold_with(&mut as_folder(visitor))
}
