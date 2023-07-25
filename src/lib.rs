use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{as_folder, FoldWith};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

mod transform;
pub use transform::TransformVisitor;

#[plugin_transform]
pub fn process_transform(program: Program, _meta: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::default()))
}
