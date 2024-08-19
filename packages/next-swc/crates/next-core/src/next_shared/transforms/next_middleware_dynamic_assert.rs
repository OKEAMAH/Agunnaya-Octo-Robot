use anyhow::Result;
use async_trait::async_trait;
use next_custom_transforms::transforms::middleware_dynamic::next_middleware_dynamic;
use turbo_tasks::Vc;
use turbopack_binding::{
    swc::core::ecma::{ast::*, visit::VisitMutWith},
    turbopack::{
        ecmascript::{CustomTransformer, EcmascriptInputTransform, TransformContext},
        turbopack::module_options::{ModuleRule, ModuleRuleEffect},
    },
};

use super::module_rule_match_js_no_url;

pub fn get_middleware_dynamic_assert_rule(enable_mdx_rs: bool) -> ModuleRule {
    let transformer =
        EcmascriptInputTransform::Plugin(Vc::cell(Box::new(NextMiddlewareDynamicAssert {}) as _));
    ModuleRule::new(
        module_rule_match_js_no_url(enable_mdx_rs),
        vec![ModuleRuleEffect::ExtendEcmascriptTransforms {
            prepend: Vc::cell(vec![]),
            append: Vc::cell(vec![transformer]),
        }],
    )
}

#[derive(Debug)]
struct NextMiddlewareDynamicAssert {}

#[async_trait]
impl CustomTransformer for NextMiddlewareDynamicAssert {
    async fn transform(&self, program: &mut Program, _ctx: &TransformContext<'_>) -> Result<()> {
        let mut visitor = next_middleware_dynamic();
        program.visit_mut_with(&mut visitor);
        Ok(())
    }
}
