use anyhow::Result;
use next_custom_transforms::transforms::strip_page_exports::ExportFilter;
use turbo_tasks::Vc;
use turbopack_binding::turbopack::turbopack::module_options::ModuleRule;

use crate::{
    mode::NextMode,
    next_client::context::ClientContextType,
    next_config::NextConfig,
    next_shared::transforms::{
        get_next_dynamic_transform_rule, get_next_font_transform_rule, get_next_image_rule,
        get_next_modularize_imports_rule, get_next_pages_transforms_rule,
        get_server_actions_transform_rule, next_amp_attributes::get_next_amp_attr_rule,
        next_cjs_optimizer::get_next_cjs_optimizer_rule,
        next_disallow_re_export_all_in_page::get_next_disallow_export_all_in_page_rule,
        next_page_config::get_next_page_config_rule,
        next_page_static_info::get_next_page_static_info_assert_rule,
        next_pure::get_next_pure_rule, server_actions::ActionsTransform,
    },
};

/// Returns a list of module rules which apply client-side, Next.js-specific
/// transforms.
pub async fn get_next_client_transforms_rules(
    next_config: Vc<NextConfig>,
    context_ty: ClientContextType,
    mode: Vc<NextMode>,
    foreign_code: bool,
) -> Result<Vec<ModuleRule>> {
    let mut rules = vec![];

    let modularize_imports_config = &next_config.await?.modularize_imports;
    let enable_mdx_rs = next_config.mdx_rs().await?.is_some();
    if let Some(modularize_imports_config) = modularize_imports_config {
        rules.push(get_next_modularize_imports_rule(
            modularize_imports_config,
            enable_mdx_rs,
        ));
    }

    rules.push(get_next_font_transform_rule(enable_mdx_rs));

    match context_ty {
        ClientContextType::Pages { pages_dir } => {
            if !foreign_code {
                rules.push(
                    get_next_pages_transforms_rule(
                        pages_dir,
                        ExportFilter::StripDataExports,
                        enable_mdx_rs,
                    )
                    .await?,
                );
                rules.push(get_next_disallow_export_all_in_page_rule(
                    enable_mdx_rs,
                    pages_dir.await?,
                ));
                rules.push(get_next_page_config_rule(enable_mdx_rs, pages_dir.await?));
            }
        }
        ClientContextType::App { .. } => {
            rules.push(get_server_actions_transform_rule(
                ActionsTransform::Client,
                enable_mdx_rs,
            ));
        }
        ClientContextType::Fallback | ClientContextType::Other => {}
    };

    if !foreign_code {
        rules.push(get_next_amp_attr_rule(enable_mdx_rs));
        rules.push(get_next_cjs_optimizer_rule(enable_mdx_rs));
        rules.push(get_next_pure_rule(enable_mdx_rs));

        rules.push(get_next_dynamic_transform_rule(false, false, mode, enable_mdx_rs).await?);

        rules.push(get_next_image_rule());
        rules.push(get_next_page_static_info_assert_rule(
            enable_mdx_rs,
            None,
            Some(context_ty),
        ));
    }

    Ok(rules)
}
