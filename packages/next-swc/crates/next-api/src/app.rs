use anyhow::{Context, Result};
use indexmap::IndexSet;
use next_core::{
    all_assets_from_entries,
    app_segment_config::NextSegmentConfig,
    app_structure::{
        get_entrypoints, Entrypoint as AppEntrypoint, Entrypoints as AppEntrypoints, LoaderTree,
        MetadataItem,
    },
    get_edge_resolve_options_context, get_next_package,
    next_app::{
        app_client_references_chunks::get_app_server_reference_modules,
        get_app_client_references_chunks, get_app_client_shared_chunk_group, get_app_page_entry,
        get_app_route_entry, metadata::route::get_app_metadata_route_entry, AppEntry, AppPage,
    },
    next_client::{
        get_client_module_options_context, get_client_resolve_options_context,
        get_client_runtime_entries, ClientContextType, RuntimeEntries,
    },
    next_client_reference::{
        ClientReferenceGraph, ClientReferenceType, NextEcmascriptClientReferenceTransition,
    },
    next_config::NextConfig,
    next_dynamic::NextDynamicTransition,
    next_edge::route_regex::get_named_middleware_regex,
    next_manifests::{
        AppBuildManifest, AppPathsManifest, BuildManifest, ClientReferenceManifest,
        EdgeFunctionDefinition, MiddlewareMatcher, MiddlewaresManifestV2, PagesManifest, Regions,
    },
    next_server::{
        get_server_module_options_context, get_server_resolve_options_context,
        get_server_runtime_entries, ServerContextType,
    },
    parse_segment_config_from_source,
    util::NextRuntime,
};
use serde::{Deserialize, Serialize};
use tracing::Instrument;
use turbo_tasks::{trace::TraceRawVcs, Completion, TryJoinIterExt, Value, Vc};
use turbopack_binding::{
    turbo::{
        tasks_env::{CustomProcessEnv, ProcessEnv},
        tasks_fs::{File, FileContent, FileSystemPath},
    },
    turbopack::{
        core::{
            asset::{Asset, AssetContent},
            chunk::{
                availability_info::AvailabilityInfo, ChunkingContext, ChunkingContextExt,
                EvaluatableAssets,
            },
            file_source::FileSource,
            module::Module,
            output::{OutputAsset, OutputAssets},
            source::Source,
            virtual_output::VirtualOutputAsset,
        },
        nodejs::EntryChunkGroupResult,
        turbopack::{
            module_options::ModuleOptionsContext,
            resolve_options_context::ResolveOptionsContext,
            transition::{ContextTransition, FullContextTransition},
            ModuleAssetContext,
        },
    },
};

use crate::{
    dynamic_imports::{
        collect_chunk_group, collect_evaluated_chunk_group, collect_next_dynamic_imports,
    },
    font::create_font_manifest,
    loadable_manifest::create_react_loadable_manifest,
    paths::{
        all_paths_in_root, all_server_paths, get_js_paths_from_root, get_wasm_paths_from_root,
        wasm_paths_to_bindings,
    },
    project::Project,
    route::{AppPageRoute, Endpoint, Route, Routes, WrittenEndpoint},
    server_actions::create_server_actions_manifest,
};

#[turbo_tasks::value]
pub struct AppProject {
    project: Vc<Project>,
    app_dir: Vc<FileSystemPath>,
}

#[turbo_tasks::value(transparent)]
pub struct OptionAppProject(Option<Vc<AppProject>>);

impl AppProject {
    fn client_ty(self: Vc<Self>) -> ClientContextType {
        ClientContextType::App {
            app_dir: self.app_dir(),
        }
    }

    fn rsc_ty(self: Vc<Self>) -> ServerContextType {
        ServerContextType::AppRSC {
            app_dir: self.app_dir(),
            client_transition: Some(Vc::upcast(self.client_transition())),
            ecmascript_client_reference_transition_name: Some(self.client_transition_name()),
        }
    }

    fn route_ty(self: Vc<Self>) -> ServerContextType {
        ServerContextType::AppRoute {
            app_dir: self.app_dir(),
            ecmascript_client_reference_transition_name: Some(self.client_transition_name()),
        }
    }

    fn ssr_ty(self: Vc<Self>) -> ServerContextType {
        ServerContextType::AppSSR {
            app_dir: self.app_dir(),
        }
    }
}

const ECMASCRIPT_CLIENT_TRANSITION_NAME: &str = "next-ecmascript-client-reference";

#[turbo_tasks::value_impl]
impl AppProject {
    #[turbo_tasks::function]
    pub fn new(project: Vc<Project>, app_dir: Vc<FileSystemPath>) -> Vc<Self> {
        AppProject { project, app_dir }.cell()
    }

    #[turbo_tasks::function]
    fn project(&self) -> Vc<Project> {
        self.project
    }

    #[turbo_tasks::function]
    fn app_dir(&self) -> Vc<FileSystemPath> {
        self.app_dir
    }

    #[turbo_tasks::function]
    fn app_entrypoints(&self) -> Vc<AppEntrypoints> {
        get_entrypoints(self.app_dir, self.project.next_config().page_extensions())
    }

    #[turbo_tasks::function]
    async fn client_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_client_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            self.project().client_compile_time_info().environment(),
            Value::new(self.client_ty()),
            self.project().next_mode(),
            self.project().next_config(),
        ))
    }

    #[turbo_tasks::function]
    async fn client_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_client_resolve_options_context(
            self.project().project_path(),
            Value::new(self.client_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    fn client_transition_name(self: Vc<Self>) -> Vc<String> {
        Vc::cell(ECMASCRIPT_CLIENT_TRANSITION_NAME.to_string())
    }

    #[turbo_tasks::function]
    async fn client_transition(self: Vc<Self>) -> Result<Vc<FullContextTransition>> {
        let module_context = self.client_module_context();
        Ok(FullContextTransition::new(module_context))
    }

    #[turbo_tasks::function]
    async fn rsc_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_server_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            Value::new(self.rsc_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            NextRuntime::NodeJs,
        ))
    }

    #[turbo_tasks::function]
    async fn edge_rsc_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_server_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            Value::new(self.rsc_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            NextRuntime::Edge,
        ))
    }

    #[turbo_tasks::function]
    async fn route_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_server_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            Value::new(self.route_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            NextRuntime::NodeJs,
        ))
    }

    #[turbo_tasks::function]
    async fn edge_route_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_server_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            Value::new(self.route_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            NextRuntime::Edge,
        ))
    }

    #[turbo_tasks::function]
    async fn rsc_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_server_resolve_options_context(
            self.project().project_path(),
            Value::new(self.rsc_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    async fn edge_rsc_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_edge_resolve_options_context(
            self.project().project_path(),
            Value::new(self.rsc_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    async fn route_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_server_resolve_options_context(
            self.project().project_path(),
            Value::new(self.route_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    async fn edge_route_resolve_options_context(
        self: Vc<Self>,
    ) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_edge_resolve_options_context(
            self.project().project_path(),
            Value::new(self.route_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    fn rsc_module_context(self: Vc<Self>) -> Vc<ModuleAssetContext> {
        let transitions = [
            (
                ECMASCRIPT_CLIENT_TRANSITION_NAME.to_string(),
                Vc::upcast(NextEcmascriptClientReferenceTransition::new(
                    Vc::upcast(self.client_transition()),
                    self.ssr_transition(),
                )),
            ),
            (
                "next-dynamic".to_string(),
                Vc::upcast(NextDynamicTransition::new(Vc::upcast(
                    self.client_transition(),
                ))),
            ),
            ("next-ssr".to_string(), Vc::upcast(self.ssr_transition())),
            (
                "next-shared".to_string(),
                Vc::upcast(self.shared_transition()),
            ),
        ]
        .into_iter()
        .collect();
        ModuleAssetContext::new(
            Vc::cell(transitions),
            self.project().server_compile_time_info(),
            self.rsc_module_options_context(),
            self.rsc_resolve_options_context(),
            Vc::cell("app-rsc".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn edge_rsc_module_context(self: Vc<Self>) -> Vc<ModuleAssetContext> {
        let transitions = [
            (
                ECMASCRIPT_CLIENT_TRANSITION_NAME.to_string(),
                Vc::upcast(NextEcmascriptClientReferenceTransition::new(
                    Vc::upcast(self.client_transition()),
                    self.edge_ssr_transition(),
                )),
            ),
            (
                "next-dynamic".to_string(),
                Vc::upcast(NextDynamicTransition::new(Vc::upcast(
                    self.client_transition(),
                ))),
            ),
            (
                "next-ssr".to_string(),
                Vc::upcast(self.edge_ssr_transition()),
            ),
            (
                "next-shared".to_string(),
                Vc::upcast(self.edge_shared_transition()),
            ),
        ]
        .into_iter()
        .collect();
        ModuleAssetContext::new(
            Vc::cell(transitions),
            self.project().edge_compile_time_info(),
            self.edge_rsc_module_options_context(),
            self.edge_rsc_resolve_options_context(),
            Vc::cell("app-edge-rsc".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn route_module_context(self: Vc<Self>) -> Vc<ModuleAssetContext> {
        let transitions = [
            (
                ECMASCRIPT_CLIENT_TRANSITION_NAME.to_string(),
                Vc::upcast(NextEcmascriptClientReferenceTransition::new(
                    Vc::upcast(self.client_transition()),
                    self.ssr_transition(),
                )),
            ),
            (
                "next-dynamic".to_string(),
                Vc::upcast(NextDynamicTransition::new(Vc::upcast(
                    self.client_transition(),
                ))),
            ),
            ("next-ssr".to_string(), Vc::upcast(self.ssr_transition())),
            (
                "next-shared".to_string(),
                Vc::upcast(self.shared_transition()),
            ),
        ]
        .into_iter()
        .collect();

        ModuleAssetContext::new(
            Vc::cell(transitions),
            self.project().server_compile_time_info(),
            self.route_module_options_context(),
            self.route_resolve_options_context(),
            Vc::cell("app-route".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn edge_route_module_context(self: Vc<Self>) -> Vc<ModuleAssetContext> {
        let transitions = [
            (
                ECMASCRIPT_CLIENT_TRANSITION_NAME.to_string(),
                Vc::upcast(NextEcmascriptClientReferenceTransition::new(
                    Vc::upcast(self.client_transition()),
                    self.edge_ssr_transition(),
                )),
            ),
            (
                "next-dynamic".to_string(),
                Vc::upcast(NextDynamicTransition::new(Vc::upcast(
                    self.client_transition(),
                ))),
            ),
            ("next-ssr".to_string(), Vc::upcast(self.ssr_transition())),
            (
                "next-shared".to_string(),
                Vc::upcast(self.edge_shared_transition()),
            ),
        ]
        .into_iter()
        .collect();
        ModuleAssetContext::new(
            Vc::cell(transitions),
            self.project().edge_compile_time_info(),
            self.edge_route_module_options_context(),
            self.edge_route_resolve_options_context(),
            Vc::cell("app-edge-route".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn client_module_context(self: Vc<Self>) -> Vc<ModuleAssetContext> {
        ModuleAssetContext::new(
            Vc::cell(Default::default()),
            self.project().client_compile_time_info(),
            self.client_module_options_context(),
            self.client_resolve_options_context(),
            Vc::cell("app-client".to_string()),
        )
    }

    #[turbo_tasks::function]
    async fn ssr_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_server_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            Value::new(self.ssr_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            NextRuntime::NodeJs,
        ))
    }

    #[turbo_tasks::function]
    async fn edge_ssr_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_server_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            Value::new(self.ssr_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            NextRuntime::Edge,
        ))
    }

    #[turbo_tasks::function]
    async fn ssr_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_server_resolve_options_context(
            self.project().project_path(),
            Value::new(self.ssr_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    async fn edge_ssr_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_edge_resolve_options_context(
            self.project().project_path(),
            Value::new(self.ssr_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    fn ssr_transition(self: Vc<Self>) -> Vc<ContextTransition> {
        ContextTransition::new(
            self.project().server_compile_time_info(),
            self.ssr_module_options_context(),
            self.ssr_resolve_options_context(),
            Vc::cell("app-ssr".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn shared_transition(self: Vc<Self>) -> Vc<ContextTransition> {
        ContextTransition::new(
            self.project().server_compile_time_info(),
            self.ssr_module_options_context(),
            self.ssr_resolve_options_context(),
            Vc::cell("app-shared".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn edge_ssr_transition(self: Vc<Self>) -> Vc<ContextTransition> {
        ContextTransition::new(
            self.project().edge_compile_time_info(),
            self.edge_ssr_module_options_context(),
            self.edge_ssr_resolve_options_context(),
            Vc::cell("app-edge-ssr".to_string()),
        )
    }

    #[turbo_tasks::function]
    fn edge_shared_transition(self: Vc<Self>) -> Vc<ContextTransition> {
        ContextTransition::new(
            self.project().edge_compile_time_info(),
            self.edge_ssr_module_options_context(),
            self.edge_ssr_resolve_options_context(),
            Vc::cell("app-edge-shared".to_string()),
        )
    }

    #[turbo_tasks::function]
    async fn runtime_entries(self: Vc<Self>) -> Result<Vc<RuntimeEntries>> {
        Ok(get_server_runtime_entries(
            Value::new(self.rsc_ty()),
            self.project().next_mode(),
        ))
    }

    #[turbo_tasks::function]
    fn rsc_runtime_entries(self: Vc<Self>) -> Vc<EvaluatableAssets> {
        self.runtime_entries()
            .resolve_entries(Vc::upcast(self.rsc_module_context()))
    }

    #[turbo_tasks::function]
    fn edge_rsc_runtime_entries(self: Vc<Self>) -> Vc<EvaluatableAssets> {
        self.runtime_entries()
            .resolve_entries(Vc::upcast(self.edge_rsc_module_context()))
    }

    #[turbo_tasks::function]
    fn client_env(self: Vc<Self>) -> Vc<Box<dyn ProcessEnv>> {
        Vc::upcast(CustomProcessEnv::new(
            self.project().env(),
            self.project().next_config().env(),
        ))
    }

    #[turbo_tasks::function]
    async fn client_runtime_entries(self: Vc<Self>) -> Result<Vc<EvaluatableAssets>> {
        Ok(get_client_runtime_entries(
            self.project().project_path(),
            Value::new(self.client_ty()),
            self.project().next_mode(),
            self.project().next_config(),
            self.project().execution_context(),
        )
        .resolve_entries(Vc::upcast(self.client_module_context())))
    }

    #[turbo_tasks::function]
    pub async fn routes(self: Vc<Self>) -> Result<Vc<Routes>> {
        let app_entrypoints = self.app_entrypoints();
        Ok(Vc::cell(
            app_entrypoints
                .await?
                .iter()
                .map(|(pathname, app_entrypoint)| async {
                    Ok((
                        pathname.to_string(),
                        app_entry_point_to_route(self, app_entrypoint.clone())
                            .await?
                            .clone_value(),
                    ))
                })
                .try_join()
                .await?
                .into_iter()
                .collect(),
        ))
    }
}

#[turbo_tasks::function]
pub fn app_entry_point_to_route(
    app_project: Vc<AppProject>,
    entrypoint: AppEntrypoint,
) -> Vc<Route> {
    match entrypoint {
        AppEntrypoint::AppPage { pages, loader_tree } => Route::AppPage(
            pages
                .into_iter()
                .map(|page| AppPageRoute {
                    original_name: page.to_string(),
                    html_endpoint: Vc::upcast(
                        AppEndpoint {
                            ty: AppEndpointType::Page {
                                ty: AppPageEndpointType::Html,
                                loader_tree,
                            },
                            app_project,
                            page: page.clone(),
                        }
                        .cell(),
                    ),
                    rsc_endpoint: Vc::upcast(
                        AppEndpoint {
                            ty: AppEndpointType::Page {
                                ty: AppPageEndpointType::Rsc,
                                loader_tree,
                            },
                            app_project,
                            page,
                        }
                        .cell(),
                    ),
                })
                .collect(),
        ),
        AppEntrypoint::AppRoute {
            page,
            path,
            root_layouts,
        } => Route::AppRoute {
            original_name: page.to_string(),
            endpoint: Vc::upcast(
                AppEndpoint {
                    ty: AppEndpointType::Route { path, root_layouts },
                    app_project,
                    page,
                }
                .cell(),
            ),
        },
        AppEntrypoint::AppMetadata { page, metadata } => Route::AppRoute {
            original_name: page.to_string(),
            endpoint: Vc::upcast(
                AppEndpoint {
                    ty: AppEndpointType::Metadata { metadata },
                    app_project,
                    page,
                }
                .cell(),
            ),
        },
    }
    .cell()
}

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, TraceRawVcs)]
enum AppPageEndpointType {
    Html,
    Rsc,
}

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, TraceRawVcs)]
enum AppEndpointType {
    Page {
        ty: AppPageEndpointType,
        loader_tree: Vc<LoaderTree>,
    },
    Route {
        path: Vc<FileSystemPath>,
        root_layouts: Vc<Vec<Vc<FileSystemPath>>>,
    },
    Metadata {
        metadata: MetadataItem,
    },
}

#[turbo_tasks::value]
struct AppEndpoint {
    ty: AppEndpointType,
    app_project: Vc<AppProject>,
    page: AppPage,
}

#[turbo_tasks::value_impl]
impl AppEndpoint {
    #[turbo_tasks::function]
    fn app_page_entry(&self, loader_tree: Vc<LoaderTree>) -> Vc<AppEntry> {
        get_app_page_entry(
            self.app_project.rsc_module_context(),
            self.app_project.edge_rsc_module_context(),
            loader_tree,
            self.page.clone(),
            self.app_project.project().project_path(),
            self.app_project.project().next_config(),
        )
    }

    #[turbo_tasks::function]
    async fn app_route_entry(
        &self,
        path: Vc<FileSystemPath>,
        root_layouts: Vc<Vec<Vc<FileSystemPath>>>,
        next_config: Vc<NextConfig>,
    ) -> Result<Vc<AppEntry>> {
        let root_layouts = root_layouts.await?;
        let config = if root_layouts.is_empty() {
            None
        } else {
            let mut config = NextSegmentConfig::default();

            for layout in root_layouts.iter().rev() {
                let source = Vc::upcast(FileSource::new(*layout));
                let layout_config = parse_segment_config_from_source(source);
                config.apply_parent_config(&*layout_config.await?);
            }

            Some(config.cell())
        };

        Ok(get_app_route_entry(
            self.app_project.route_module_context(),
            self.app_project.edge_route_module_context(),
            Vc::upcast(FileSource::new(path)),
            self.page.clone(),
            self.app_project.project().project_path(),
            config,
            next_config,
        ))
    }

    #[turbo_tasks::function]
    async fn app_metadata_entry(
        &self,
        metadata: MetadataItem,
        next_config: Vc<NextConfig>,
    ) -> Result<Vc<AppEntry>> {
        Ok(get_app_metadata_route_entry(
            self.app_project.rsc_module_context(),
            self.app_project.edge_rsc_module_context(),
            self.app_project.project().project_path(),
            self.page.clone(),
            *self.app_project.project().next_mode().await?,
            metadata,
            next_config,
        ))
    }

    #[turbo_tasks::function]
    fn output_assets(self: Vc<Self>) -> Vc<OutputAssets> {
        self.output().output_assets()
    }

    #[turbo_tasks::function]
    async fn output(self: Vc<Self>) -> Result<Vc<AppEndpointOutput>> {
        let this = self.await?;

        let next_config = self.await?.app_project.project().next_config();
        let (app_entry, process_client, process_ssr) = match this.ty {
            AppEndpointType::Page { ty, loader_tree } => (
                self.app_page_entry(loader_tree),
                true,
                matches!(ty, AppPageEndpointType::Html),
            ),
            // NOTE(alexkirsz) For routes, technically, a lot of the following code is not needed,
            // as we know we won't have any client references. However, for now, for simplicity's
            // sake, we just do the same thing as for pages.
            AppEndpointType::Route { path, root_layouts } => (
                self.app_route_entry(path, root_layouts, next_config),
                false,
                false,
            ),
            AppEndpointType::Metadata { metadata } => {
                (self.app_metadata_entry(metadata, next_config), false, false)
            }
        };

        let node_root = this.app_project.project().node_root();

        let client_relative_path = this.app_project.project().client_relative_path();
        let client_relative_path_ref = client_relative_path.await?;

        let server_path = node_root.join("server".to_string());

        let mut server_assets = vec![];
        let mut client_assets = vec![];
        // assets to add to the middleware manifest (to be loaded in the edge runtime).
        let mut middleware_assets = vec![];

        let app_entry = app_entry.await?;

        let runtime = app_entry.config.await?.runtime.unwrap_or_default();

        let rsc_entry = app_entry.rsc_entry;

        let rsc_entry_asset = Vc::upcast(rsc_entry);

        let client_chunking_context = this.app_project.project().client_chunking_context();

        let (app_server_reference_modules, client_dynamic_imports) = if process_client {
            let client_shared_chunk_group = get_app_client_shared_chunk_group(
                app_entry
                    .rsc_entry
                    .ident()
                    .with_modifier(Vc::cell("client_shared_chunks".to_string())),
                this.app_project.client_runtime_entries(),
                client_chunking_context,
            )
            .await?;

            let mut client_shared_chunks_paths = vec![];
            for chunk in client_shared_chunk_group.assets.await?.iter().copied() {
                client_assets.push(chunk);

                let chunk_path = chunk.ident().path().await?;
                if chunk_path.extension_ref() == Some("js") {
                    if let Some(chunk_path) = client_relative_path_ref.get_path_to(&chunk_path) {
                        client_shared_chunks_paths.push(chunk_path.to_string());
                    }
                }
            }
            let client_shared_availability_info = client_shared_chunk_group.availability_info;

            let client_reference_graph = ClientReferenceGraph::new(Vc::cell(vec![rsc_entry_asset]));
            let client_reference_types = client_reference_graph.types();
            let client_references = client_reference_graph.entry(rsc_entry_asset);

            let ssr_chunking_context = if process_ssr {
                Some(match runtime {
                    NextRuntime::NodeJs => {
                        Vc::upcast(this.app_project.project().server_chunking_context(true))
                    }
                    NextRuntime::Edge => this
                        .app_project
                        .project()
                        .edge_chunking_context(process_client),
                })
            } else {
                None
            };

            let client_dynamic_imports = collect_next_dynamic_imports(
                client_references
                    .await?
                    .client_references
                    .iter()
                    .filter_map(|r| match r.ty() {
                        ClientReferenceType::EcmascriptClientReference(entry) => Some(entry),
                        ClientReferenceType::CssClientReference(_) => None,
                    })
                    .map(|entry| async move { Ok(Vc::upcast(entry.await?.ssr_module)) })
                    .try_join()
                    .await?,
                Vc::upcast(this.app_project.client_module_context()),
            )
            .await?;

            let client_references_chunks = get_app_client_references_chunks(
                client_references,
                client_chunking_context,
                Value::new(client_shared_availability_info),
                ssr_chunking_context,
            );
            let client_references_chunks_ref = client_references_chunks.await?;

            let mut entry_client_chunks = IndexSet::new();
            // TODO(alexkirsz) In which manifest does this go?
            let mut entry_ssr_chunks = IndexSet::new();
            for chunks in client_references_chunks_ref
                .layout_segment_client_chunks
                .values()
            {
                entry_client_chunks.extend(chunks.await?.iter().copied());
            }
            for chunks in client_references_chunks_ref
                .client_component_client_chunks
                .values()
            {
                client_assets.extend(chunks.await?.iter().copied());
            }
            for chunks in client_references_chunks_ref
                .client_component_ssr_chunks
                .values()
            {
                entry_ssr_chunks.extend(chunks.await?.iter().copied());
            }

            client_assets.extend(entry_client_chunks.iter().copied());
            server_assets.extend(entry_ssr_chunks.iter().copied());

            let entry_client_chunks_paths = entry_client_chunks
                .iter()
                .map(|chunk| chunk.ident().path())
                .try_join()
                .await?;
            let mut entry_client_chunks_paths = entry_client_chunks_paths
                .iter()
                .map(|path| {
                    Ok(client_relative_path_ref
                        .get_path_to(path)
                        .context("asset path should be inside client root")?
                        .to_string())
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            entry_client_chunks_paths.extend(client_shared_chunks_paths.iter().cloned());

            let app_build_manifest = AppBuildManifest {
                pages: [(app_entry.original_name.clone(), entry_client_chunks_paths)]
                    .into_iter()
                    .collect(),
            };
            let manifest_path_prefix = &app_entry.original_name;
            let app_build_manifest_output = Vc::upcast(VirtualOutputAsset::new(
                node_root.join(format!(
                    "server/app{manifest_path_prefix}/app-build-manifest.json",
                )),
                AssetContent::file(
                    File::from(serde_json::to_string_pretty(&app_build_manifest)?).into(),
                ),
            ));
            server_assets.push(app_build_manifest_output);

            // polyfill-nomodule.js is a pre-compiled asset distributed as part of next,
            // load it as a RawModule.
            let next_package = get_next_package(this.app_project.project().project_path());
            let polyfill_source = FileSource::new(
                next_package.join("dist/build/polyfills/polyfill-nomodule.js".to_string()),
            );
            let polyfill_output_path =
                client_chunking_context.chunk_path(polyfill_source.ident(), ".js".to_string());
            let polyfill_output_asset =
                VirtualOutputAsset::new(polyfill_output_path, polyfill_source.content());
            let polyfill_client_path = client_relative_path_ref
                .get_path_to(&*polyfill_output_path.await?)
                .context("failed to resolve client-relative path to polyfill")?
                .to_string();
            let polyfill_client_paths = vec![polyfill_client_path];
            client_assets.push(Vc::upcast(polyfill_output_asset));

            let build_manifest = BuildManifest {
                root_main_files: client_shared_chunks_paths,
                polyfill_files: polyfill_client_paths,
                ..Default::default()
            };
            let build_manifest_output = Vc::upcast(VirtualOutputAsset::new(
                node_root.join(format!(
                    "server/app{manifest_path_prefix}/build-manifest.json",
                )),
                AssetContent::file(
                    File::from(serde_json::to_string_pretty(&build_manifest)?).into(),
                ),
            ));
            server_assets.push(build_manifest_output);

            let entry_manifest = ClientReferenceManifest::build_output(
                node_root,
                client_relative_path,
                app_entry.original_name.clone(),
                client_references,
                client_references_chunks,
                client_chunking_context,
                ssr_chunking_context,
                this.app_project.project().next_config(),
                runtime,
            );
            server_assets.push(entry_manifest);

            if runtime == NextRuntime::Edge {
                middleware_assets.push(entry_manifest);

                // as the edge runtime doesn't support chunk loading we need to add all client
                // references to the middleware manifest so they get loaded during runtime
                // initialization
                let client_references_chunks = &*client_references_chunks.await?;

                for &ssr_chunks in client_references_chunks
                    .client_component_ssr_chunks
                    .values()
                {
                    let ssr_chunks = ssr_chunks.await?;

                    middleware_assets.extend(ssr_chunks);
                }
            }

            (
                Some(get_app_server_reference_modules(client_reference_types)),
                Some(client_dynamic_imports),
            )
        } else {
            (None, None)
        };

        fn create_app_paths_manifest(
            node_root: Vc<FileSystemPath>,
            original_name: &str,
            filename: String,
        ) -> Result<Vc<Box<dyn OutputAsset>>> {
            let manifest_path_prefix = original_name;
            let path = node_root.join(format!(
                "server/app{manifest_path_prefix}/app-paths-manifest.json",
            ));
            let app_paths_manifest = AppPathsManifest {
                node_server_app_paths: PagesManifest {
                    pages: [(original_name.to_string(), filename)]
                        .into_iter()
                        .collect(),
                },
                ..Default::default()
            };
            Ok(Vc::upcast(VirtualOutputAsset::new(
                path,
                AssetContent::file(
                    File::from(serde_json::to_string_pretty(&app_paths_manifest)?).into(),
                ),
            )))
        }

        let client_assets = OutputAssets::new(client_assets);

        let next_font_manifest_output = create_font_manifest(
            this.app_project.project().client_root(),
            node_root,
            this.app_project.app_dir(),
            &app_entry.original_name,
            &app_entry.original_name,
            &app_entry.original_name,
            client_assets,
            true,
        )
        .await?;
        server_assets.push(next_font_manifest_output);

        let endpoint_output = match runtime {
            NextRuntime::Edge => {
                // create edge chunks
                let chunking_context = this
                    .app_project
                    .project()
                    .edge_chunking_context(process_client);
                let mut evaluatable_assets = this
                    .app_project
                    .edge_rsc_runtime_entries()
                    .await?
                    .clone_value();
                let evaluatable = Vc::try_resolve_sidecast(app_entry.rsc_entry)
                    .await?
                    .context("Entry module must be evaluatable")?;
                evaluatable_assets.push(evaluatable);

                if let Some(app_server_reference_modules) = app_server_reference_modules {
                    let (loader, manifest) = create_server_actions_manifest(
                        Vc::upcast(app_entry.rsc_entry),
                        app_server_reference_modules,
                        this.app_project.project().project_path(),
                        node_root,
                        &app_entry.original_name,
                        NextRuntime::Edge,
                        Vc::upcast(this.app_project.edge_rsc_module_context()),
                        Vc::upcast(chunking_context),
                    )
                    .await?;
                    server_assets.push(manifest);
                    evaluatable_assets.push(loader);
                }

                let files = chunking_context.evaluated_chunk_group_assets(
                    app_entry.rsc_entry.ident(),
                    Vc::cell(evaluatable_assets.clone()),
                    Value::new(AvailabilityInfo::Root),
                );
                let files_value = files.await?;

                server_assets.extend(files_value.iter().copied());

                // the next-edge-ssr-loader templates expect the manifests to be stored in
                // global variables defined in these files
                //
                // they are created in `setup-dev-bundler.ts`
                let mut file_paths_from_root = vec![
                    "server/server-reference-manifest.js".to_string(),
                    "server/middleware-build-manifest.js".to_string(),
                    "server/middleware-react-loadable-manifest.js".to_string(),
                    "server/next-font-manifest.js".to_string(),
                    "server/interception-route-rewrite-manifest.js".to_string(),
                ];
                let mut wasm_paths_from_root = vec![];

                let node_root_value = node_root.await?;

                file_paths_from_root
                    .extend(get_js_paths_from_root(&node_root_value, &middleware_assets).await?);
                file_paths_from_root
                    .extend(get_js_paths_from_root(&node_root_value, &files_value).await?);

                let all_output_assets = all_assets_from_entries(files).await?;

                wasm_paths_from_root
                    .extend(get_wasm_paths_from_root(&node_root_value, &middleware_assets).await?);
                wasm_paths_from_root
                    .extend(get_wasm_paths_from_root(&node_root_value, &all_output_assets).await?);

                let entry_file = "app-edge-has-no-entrypoint".to_string();

                // create middleware manifest
                // TODO(alexkirsz) This should be shared with next build.
                let named_regex = get_named_middleware_regex(&app_entry.pathname);
                let matchers = MiddlewareMatcher {
                    regexp: Some(named_regex),
                    original_source: app_entry.pathname.clone(),
                    ..Default::default()
                };
                let edge_function_definition = EdgeFunctionDefinition {
                    files: file_paths_from_root,
                    wasm: wasm_paths_to_bindings(wasm_paths_from_root),
                    name: app_entry.pathname.to_string(),
                    page: app_entry.original_name.clone(),
                    regions: app_entry
                        .config
                        .await?
                        .preferred_region
                        .clone()
                        .map(Regions::Multiple),
                    matchers: vec![matchers],
                    env: this.app_project.project().edge_env().await?.clone_value(),
                    ..Default::default()
                };
                let middleware_manifest_v2 = MiddlewaresManifestV2 {
                    sorted_middleware: vec![app_entry.original_name.clone()],
                    functions: [(app_entry.original_name.clone(), edge_function_definition)]
                        .into_iter()
                        .collect(),
                    ..Default::default()
                };
                let manifest_path_prefix = &app_entry.original_name;
                let middleware_manifest_v2 = Vc::upcast(VirtualOutputAsset::new(
                    node_root.join(format!(
                        "server/app{manifest_path_prefix}/middleware-manifest.json",
                    )),
                    AssetContent::file(
                        FileContent::Content(File::from(serde_json::to_string_pretty(
                            &middleware_manifest_v2,
                        )?))
                        .cell(),
                    ),
                ));
                server_assets.push(middleware_manifest_v2);

                // create app paths manifest
                let app_paths_manifest_output =
                    create_app_paths_manifest(node_root, &app_entry.original_name, entry_file)?;
                server_assets.push(app_paths_manifest_output);

                // create react-loadable-manifest for next/dynamic
                let mut dynamic_import_modules = collect_next_dynamic_imports(
                    [Vc::upcast(app_entry.rsc_entry)],
                    Vc::upcast(this.app_project.client_module_context()),
                )
                .await?;
                dynamic_import_modules.extend(client_dynamic_imports.into_iter().flatten());
                let dynamic_import_entries = collect_evaluated_chunk_group(
                    Vc::upcast(client_chunking_context),
                    dynamic_import_modules,
                )
                .await?;
                let loadable_manifest_output = create_react_loadable_manifest(
                    dynamic_import_entries,
                    client_relative_path,
                    node_root.join(format!(
                        "server/app{}/react-loadable-manifest.json",
                        &app_entry.original_name
                    )),
                );
                server_assets.extend(loadable_manifest_output.await?.iter().copied());

                AppEndpointOutput::Edge {
                    files,
                    server_assets: Vc::cell(server_assets),
                    client_assets,
                }
            }
            NextRuntime::NodeJs => {
                let mut evaluatable_assets =
                    this.app_project.rsc_runtime_entries().await?.clone_value();

                let chunking_context = this
                    .app_project
                    .project()
                    .server_chunking_context(process_client);

                if let Some(app_server_reference_modules) = app_server_reference_modules {
                    let (loader, manifest) = create_server_actions_manifest(
                        Vc::upcast(app_entry.rsc_entry),
                        app_server_reference_modules,
                        this.app_project.project().project_path(),
                        node_root,
                        &app_entry.original_name,
                        NextRuntime::NodeJs,
                        Vc::upcast(this.app_project.rsc_module_context()),
                        Vc::upcast(chunking_context),
                    )
                    .await?;
                    server_assets.push(manifest);
                    evaluatable_assets.push(loader);
                }

                let EntryChunkGroupResult {
                    asset: rsc_chunk, ..
                } = *chunking_context
                    .entry_chunk_group(
                        server_path.join(format!(
                            "app{original_name}.js",
                            original_name = app_entry.original_name
                        )),
                        app_entry.rsc_entry,
                        Vc::cell(evaluatable_assets),
                        Value::new(AvailabilityInfo::Root),
                    )
                    .await?;
                server_assets.push(rsc_chunk);

                let app_paths_manifest_output = create_app_paths_manifest(
                    node_root,
                    &app_entry.original_name,
                    server_path
                        .await?
                        .get_path_to(&*rsc_chunk.ident().path().await?)
                        .context("RSC chunk path should be within app paths manifest directory")?
                        .to_string(),
                )?;
                server_assets.push(app_paths_manifest_output);

                // create react-loadable-manifest for next/dynamic
                let availability_info = Value::new(AvailabilityInfo::Root);
                let mut dynamic_import_modules = collect_next_dynamic_imports(
                    [Vc::upcast(app_entry.rsc_entry)],
                    Vc::upcast(this.app_project.client_module_context()),
                )
                .await?;
                dynamic_import_modules.extend(client_dynamic_imports.into_iter().flatten());
                let dynamic_import_entries = collect_chunk_group(
                    Vc::upcast(client_chunking_context),
                    dynamic_import_modules,
                    availability_info,
                )
                .await?;
                let loadable_manifest_output = create_react_loadable_manifest(
                    dynamic_import_entries,
                    client_relative_path,
                    node_root.join(format!(
                        "server/app{}/react-loadable-manifest.json",
                        &app_entry.original_name
                    )),
                );
                server_assets.extend(loadable_manifest_output.await?.iter().copied());

                AppEndpointOutput::NodeJs {
                    rsc_chunk,
                    server_assets: Vc::cell(server_assets),
                    client_assets,
                }
            }
        }
        .cell();

        Ok(endpoint_output)
    }
}

#[turbo_tasks::value_impl]
impl Endpoint for AppEndpoint {
    #[turbo_tasks::function]
    async fn write_to_disk(self: Vc<Self>) -> Result<Vc<WrittenEndpoint>> {
        let this = self.await?;
        let page_name = this.page.to_string();
        let span = match this.ty {
            AppEndpointType::Page {
                ty: AppPageEndpointType::Html,
                ..
            } => {
                tracing::info_span!("app endpoint HTML", name = page_name)
            }
            AppEndpointType::Page {
                ty: AppPageEndpointType::Rsc,
                ..
            } => {
                tracing::info_span!("app endpoint RSC", name = page_name)
            }
            AppEndpointType::Route { .. } => {
                tracing::info_span!("app endpoint route", name = page_name)
            }
            AppEndpointType::Metadata { .. } => {
                tracing::info_span!("app endpoint metadata", name = page_name)
            }
        };
        async move {
            let output = self.output();
            // Must use self.output_assets() instead of output.output_assets() to make it a
            // single operation
            let output_assets = self.output_assets();

            let node_root = this.app_project.project().node_root();

            let node_root_ref = &node_root.await?;

            this.app_project
                .project()
                .emit_all_output_assets(Vc::cell(output_assets))
                .await?;

            let node_root = this.app_project.project().node_root();
            let server_paths = all_server_paths(output_assets, node_root)
                .await?
                .clone_value();

            let client_relative_root = this.app_project.project().client_relative_path();
            let client_paths = all_paths_in_root(output_assets, client_relative_root)
                .await?
                .clone_value();

            let written_endpoint = match *output.await? {
                AppEndpointOutput::NodeJs { rsc_chunk, .. } => WrittenEndpoint::NodeJs {
                    server_entry_path: node_root_ref
                        .get_path_to(&*rsc_chunk.ident().path().await?)
                        .context("Node.js chunk entry path must be inside the node root")?
                        .to_string(),
                    server_paths,
                    client_paths,
                },
                AppEndpointOutput::Edge { .. } => WrittenEndpoint::Edge {
                    server_paths,
                    client_paths,
                },
            };
            anyhow::Ok(written_endpoint.cell())
        }
        .instrument(span)
        .await
        .with_context(|| format!("Failed to write app endpoint {}", page_name))
    }

    #[turbo_tasks::function]
    async fn server_changed(self: Vc<Self>) -> Result<Vc<Completion>> {
        Ok(self
            .await?
            .app_project
            .project()
            .server_changed(self.output().server_assets()))
    }

    #[turbo_tasks::function]
    async fn client_changed(self: Vc<Self>) -> Result<Vc<Completion>> {
        Ok(self
            .await?
            .app_project
            .project()
            .client_changed(self.output().client_assets()))
    }
}

#[turbo_tasks::value]
enum AppEndpointOutput {
    NodeJs {
        rsc_chunk: Vc<Box<dyn OutputAsset>>,
        server_assets: Vc<OutputAssets>,
        client_assets: Vc<OutputAssets>,
    },
    Edge {
        files: Vc<OutputAssets>,
        server_assets: Vc<OutputAssets>,
        client_assets: Vc<OutputAssets>,
    },
}

#[turbo_tasks::value_impl]
impl AppEndpointOutput {
    #[turbo_tasks::function]
    pub async fn output_assets(self: Vc<Self>) -> Result<Vc<OutputAssets>> {
        let server_assets = self.server_assets().await?;
        let client_assets = self.client_assets().await?;
        Ok(Vc::cell(
            server_assets
                .iter()
                .cloned()
                .chain(client_assets.iter().cloned())
                .collect(),
        ))
    }

    #[turbo_tasks::function]
    pub fn server_assets(&self) -> Vc<OutputAssets> {
        match *self {
            AppEndpointOutput::NodeJs { server_assets, .. }
            | AppEndpointOutput::Edge { server_assets, .. } => server_assets,
        }
    }

    #[turbo_tasks::function]
    pub fn client_assets(&self) -> Vc<OutputAssets> {
        match *self {
            AppEndpointOutput::NodeJs { client_assets, .. }
            | AppEndpointOutput::Edge { client_assets, .. } => client_assets,
        }
    }
}
