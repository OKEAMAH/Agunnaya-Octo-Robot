use anyhow::Result;
use next_core::{all_assets_from_entries, next_manifests::NextFontManifest};
use turbo_tasks::{ValueToString, Vc};
use turbopack_binding::{
    turbo::tasks_fs::{File, FileSystemPath},
    turbopack::core::{
        asset::AssetContent,
        output::{OutputAsset, OutputAssets},
        virtual_output::VirtualOutputAsset,
    },
};

use crate::paths::get_font_paths_from_root;

pub(crate) async fn create_font_manifest(
    client_root: Vc<FileSystemPath>,
    node_root: Vc<FileSystemPath>,
    dir: Vc<FileSystemPath>,
    original_name: &str,
    manifest_path_prefix: &str,
    pathname: &str,
    client_assets: Vc<OutputAssets>,
    app_dir: bool,
) -> Result<Vc<Box<dyn OutputAsset>>> {
    let all_client_output_assets = all_assets_from_entries(client_assets).await?;

    // `_next` gets added again later, so we "strip" it here via
    // `get_font_paths_from_root`.
    let font_paths: Vec<String> =
        get_font_paths_from_root(&*client_root.await?, &all_client_output_assets)
            .await?
            .iter()
            .filter_map(|p| p.split("_next/").last().map(|f| f.to_string()))
            .collect();

    let path = if app_dir {
        node_root.join(format!(
            "server/app{manifest_path_prefix}/next-font-manifest.json",
        ))
    } else {
        node_root.join(format!(
            "server/pages{manifest_path_prefix}/next-font-manifest.json"
        ))
    };

    let has_fonts = !font_paths.is_empty();
    let using_size_adjust = font_paths.iter().any(|path| path.contains("-s"));

    let font_paths = font_paths
        .into_iter()
        .filter(|path| path.contains(".p."))
        .collect::<Vec<_>>();

    let next_font_manifest = if !has_fonts {
        Default::default()
    } else if app_dir {
        let dir_str = dir.to_string().await?;
        let page_path = format!("{}{}", dir_str, original_name);

        NextFontManifest {
            app: [(page_path, font_paths)].into_iter().collect(),
            app_using_size_adjust: using_size_adjust,
            ..Default::default()
        }
    } else {
        NextFontManifest {
            pages: [(pathname.to_string(), font_paths)].into_iter().collect(),
            pages_using_size_adjust: using_size_adjust,
            ..Default::default()
        }
    };

    Ok(Vc::upcast(VirtualOutputAsset::new(
        path,
        AssetContent::file(File::from(serde_json::to_string_pretty(&next_font_manifest)?).into()),
    )))
}
