use anyhow::{bail, Result};
use indexmap::indexmap;
use turbo_tasks::{Value, ValueToString, Vc};
use turbopack_binding::{
    turbo::tasks_fs::FileSystemPath,
    turbopack::{
        core::{
            context::AssetContext,
            module::Module,
            reference_type::{EntryReferenceSubType, ReferenceType},
            source::Source,
        },
        ecmascript::chunk::EcmascriptChunkPlaceable,
        turbopack::ModuleAssetContext,
    },
};

use crate::{
    app_segment_config::NextSegmentConfig,
    next_app::{AppEntry, AppPage, AppPath},
    next_config::{NextConfig, OutputType},
    next_edge::entry::wrap_edge_entry,
    parse_segment_config_from_source,
    util::{load_next_js_template, NextRuntime},
};

/// Computes the entry for a Next.js app route.
/// # Arguments
///
/// * `original_segment_config` - A next segment config to be specified
///   explicitly for the given source.
/// For some cases `source` may not be the original but the handler (dynamic
/// metadata) which will lose segment config.
#[turbo_tasks::function]
pub async fn get_app_route_entry(
    nodejs_context: Vc<ModuleAssetContext>,
    edge_context: Vc<ModuleAssetContext>,
    source: Vc<Box<dyn Source>>,
    page: AppPage,
    project_root: Vc<FileSystemPath>,
    original_segment_config: Option<Vc<NextSegmentConfig>>,
    next_config: Vc<NextConfig>,
) -> Result<Vc<AppEntry>> {
    let segment_from_source = parse_segment_config_from_source(source);
    let config = if let Some(original_segment_config) = original_segment_config {
        let mut segment_config = (*segment_from_source.await?).clone();
        segment_config.apply_parent_config(&*original_segment_config.await?);
        segment_config.into()
    } else {
        segment_from_source
    };

    let is_edge = matches!(config.await?.runtime, Some(NextRuntime::Edge));
    let context = if is_edge {
        edge_context
    } else {
        nodejs_context
    };

    let original_name = page.to_string();
    let pathname = AppPath::from(page.clone()).to_string();

    let path = source.ident().path();

    const INNER: &str = "INNER_APP_ROUTE";

    let output_type = next_config
        .await?
        .output
        .as_ref()
        .map(|o| match o {
            OutputType::Standalone => "\"standalone\"".to_string(),
            OutputType::Export => "\"export\"".to_string(),
        })
        .unwrap_or_else(|| "\"\"".to_string());

    // Load the file from the next.js codebase.
    let virtual_source = load_next_js_template(
        "app-route.js",
        project_root,
        indexmap! {
            "VAR_DEFINITION_PAGE" => page.to_string(),
            "VAR_DEFINITION_PATHNAME" => pathname.clone(),
            "VAR_DEFINITION_FILENAME" => path.file_stem().await?.as_ref().unwrap().clone(),
            // TODO(alexkirsz) Is this necessary?
            "VAR_DEFINITION_BUNDLE_PATH" => "".to_string(),
            "VAR_ORIGINAL_PATHNAME" => original_name.clone(),
            "VAR_RESOLVED_PAGE_PATH" => path.to_string().await?.clone_value(),
            "VAR_USERLAND" => INNER.to_string(),
        },
        indexmap! {
            "nextConfigOutput" => output_type
        },
        indexmap! {},
    )
    .await?;

    let userland_module = context
        .process(
            source,
            Value::new(ReferenceType::Entry(EntryReferenceSubType::AppRoute)),
        )
        .module();

    let inner_assets = indexmap! {
        INNER.to_string() => userland_module
    };

    let mut rsc_entry = context
        .process(
            Vc::upcast(virtual_source),
            Value::new(ReferenceType::Internal(Vc::cell(inner_assets))),
        )
        .module();

    if is_edge {
        rsc_entry = wrap_edge_route(
            Vc::upcast(context),
            project_root,
            rsc_entry,
            pathname.clone(),
        );
    }

    let Some(rsc_entry) =
        Vc::try_resolve_downcast::<Box<dyn EcmascriptChunkPlaceable>>(rsc_entry).await?
    else {
        bail!("expected an ECMAScript chunk placeable module");
    };

    Ok(AppEntry {
        pathname,
        original_name,
        rsc_entry,
        config,
    }
    .cell())
}

#[turbo_tasks::function]
async fn wrap_edge_route(
    context: Vc<Box<dyn AssetContext>>,
    project_root: Vc<FileSystemPath>,
    entry: Vc<Box<dyn Module>>,
    pathname: String,
) -> Result<Vc<Box<dyn Module>>> {
    const INNER: &str = "INNER_ROUTE_ENTRY";

    let source = load_next_js_template(
        "edge-app-route.js",
        project_root,
        indexmap! {
            "VAR_USERLAND" => INNER.to_string(),
        },
        indexmap! {},
        indexmap! {},
    )
    .await?;

    let inner_assets = indexmap! {
        INNER.to_string() => entry
    };

    let wrapped = context
        .process(
            Vc::upcast(source),
            Value::new(ReferenceType::Internal(Vc::cell(inner_assets))),
        )
        .module();

    Ok(wrap_edge_entry(context, project_root, wrapped, pathname))
}
