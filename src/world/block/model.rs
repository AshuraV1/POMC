use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::assets::{resolve_asset_path, AssetIndex};

use super::registry::{FaceTextures, Tint};

#[derive(Deserialize)]
struct BlockstateFile {
    variants: Option<HashMap<String, VariantEntry>>,
    multipart: Option<Vec<MultipartCase>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VariantEntry {
    Single(ModelRef),
    Array(Vec<ModelRef>),
}

#[derive(Deserialize)]
struct MultipartCase {
    apply: MultipartApply,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MultipartApply {
    Single(ModelRef),
    Array(Vec<ModelRef>),
}

#[derive(Deserialize)]
struct ModelRef {
    model: String,
}

#[derive(Deserialize, Default)]
struct ModelFile {
    parent: Option<String>,
    #[serde(default)]
    textures: HashMap<String, String>,
}

const FOLIAGE_TINTED: &[&str] = &[
    "oak_leaves", "dark_oak_leaves", "jungle_leaves", "acacia_leaves",
    "mangrove_leaves",
];

const GRASS_TINTED: &[&str] = &[
    "grass_block", "short_grass", "tall_grass", "fern", "large_fern",
];

pub fn load_all_block_textures(
    assets_dir: &Path,
    asset_index: &Option<AssetIndex>,
) -> HashMap<String, FaceTextures> {
    let Some(blockstates_dir) = resolve_blockstates_dir(assets_dir, asset_index) else {
        log::warn!("Blockstates directory not found");
        return HashMap::new();
    };

    let entries = match std::fs::read_dir(&blockstates_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("Failed to read blockstates dir: {e}");
            return HashMap::new();
        }
    };

    let mut results = HashMap::new();
    let mut model_cache: HashMap<String, ModelFile> = HashMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue }

        let block_name = name.to_string();

        let Ok(contents) = std::fs::read_to_string(&path) else { continue };
        let Ok(blockstate) = serde_json::from_str::<BlockstateFile>(&contents) else { continue };

        let Some(model_id) = extract_default_model(&blockstate) else { continue };
        let resolved = resolve_textures(&model_id, assets_dir, asset_index, &mut model_cache);
        let Some(face_textures) = build_face_textures(&block_name, &resolved) else { continue };

        results.insert(block_name, face_textures);
    }

    log::info!("Loaded {} block texture mappings from vanilla assets", results.len());
    results
}

fn resolve_blockstates_dir(assets_dir: &Path, asset_index: &Option<AssetIndex>) -> Option<PathBuf> {
    let candidates = [
        assets_dir.join("assets/minecraft/blockstates"),
        assets_dir.join("jar/assets/minecraft/blockstates"),
        PathBuf::from("reference/assets/assets/minecraft/blockstates"),
    ];

    for c in &candidates {
        if c.is_dir() {
            return Some(c.clone());
        }
    }

    if asset_index.is_some() {
        let test_path = resolve_asset_path(assets_dir, asset_index, "minecraft/blockstates/stone.json");
        if test_path.exists() {
            return test_path.parent().map(|p| p.to_path_buf());
        }
    }

    None
}

fn extract_default_model(blockstate: &BlockstateFile) -> Option<String> {
    if let Some(variants) = &blockstate.variants {
        let entry = variants.get("")
            .or_else(|| variants.values().next())?;
        first_model_ref(entry)
    } else if let Some(multipart) = &blockstate.multipart {
        first_multipart_model(&multipart.first()?.apply)
    } else {
        None
    }
}

fn first_model_ref(entry: &VariantEntry) -> Option<String> {
    match entry {
        VariantEntry::Single(r) => Some(r.model.clone()),
        VariantEntry::Array(arr) => arr.first().map(|r| r.model.clone()),
    }
}

fn first_multipart_model(apply: &MultipartApply) -> Option<String> {
    match apply {
        MultipartApply::Single(r) => Some(r.model.clone()),
        MultipartApply::Array(arr) => arr.first().map(|r| r.model.clone()),
    }
}

fn resolve_textures(
    model_id: &str,
    assets_dir: &Path,
    asset_index: &Option<AssetIndex>,
    cache: &mut HashMap<String, ModelFile>,
) -> HashMap<String, String> {
    let mut texture_map: HashMap<String, String> = HashMap::new();
    let mut current_id = model_id.to_string();

    for _ in 0..20 {
        let Some(model) = load_model(&current_id, assets_dir, asset_index, cache) else { break };

        for (key, value) in &model.textures {
            texture_map.entry(key.clone()).or_insert_with(|| value.clone());
        }

        match &model.parent {
            Some(parent) => current_id = parent.clone(),
            None => break,
        }
    }

    let mut resolved = HashMap::new();
    for (key, value) in &texture_map {
        resolved.insert(key.clone(), resolve_ref(value, &texture_map, 0));
    }
    resolved
}

fn resolve_ref(value: &str, map: &HashMap<String, String>, depth: u32) -> String {
    if depth > 10 { return value.to_string(); }
    if let Some(ref_name) = value.strip_prefix('#') {
        if let Some(target) = map.get(ref_name) {
            return resolve_ref(target, map, depth + 1);
        }
    }
    value.to_string()
}

fn load_model<'a>(
    model_id: &str,
    assets_dir: &Path,
    asset_index: &Option<AssetIndex>,
    cache: &'a mut HashMap<String, ModelFile>,
) -> Option<&'a ModelFile> {
    if cache.contains_key(model_id) {
        return cache.get(model_id);
    }

    let asset_key = model_id_to_asset_key(model_id);
    let file_path = resolve_model_path(assets_dir, asset_index, &asset_key)?;

    let contents = std::fs::read_to_string(&file_path).ok()?;
    let model: ModelFile = serde_json::from_str(&contents).ok()?;
    cache.insert(model_id.to_string(), model);
    cache.get(model_id)
}

fn resolve_model_path(assets_dir: &Path, asset_index: &Option<AssetIndex>, asset_key: &str) -> Option<PathBuf> {
    let primary = resolve_asset_path(assets_dir, asset_index, asset_key);
    if primary.exists() {
        return Some(primary);
    }

    let ref_path = Path::new("reference/assets/assets")
        .join(asset_key.strip_prefix("minecraft/").unwrap_or(asset_key));
    if ref_path.exists() {
        return Some(ref_path);
    }

    None
}

fn model_id_to_asset_key(model_id: &str) -> String {
    let stripped = model_id.strip_prefix("minecraft:").unwrap_or(model_id);
    format!("minecraft/models/{stripped}.json")
}

fn texture_to_name(texture_ref: &str) -> Option<&str> {
    let stripped = texture_ref.strip_prefix("minecraft:").unwrap_or(texture_ref);
    stripped.strip_prefix("block/")
}

fn build_face_textures(block_name: &str, textures: &HashMap<String, String>) -> Option<FaceTextures> {
    let get = |key: &str| -> Option<&str> {
        textures.get(key).and_then(|v| texture_to_name(v))
    };

    let (up, down, north, south, east, west) =
        (get("up"), get("down"), get("north"), get("south"), get("east"), get("west"));

    let tint = determine_tint(block_name);

    if let (Some(up), Some(down), Some(north), Some(south), Some(east), Some(west)) =
        (up, down, north, south, east, west)
    {
        let (side_overlay, tint) = if block_name == "grass_block" {
            (Some("grass_block_side_overlay"), Tint::Grass)
        } else {
            (None, tint)
        };
        return Some(FaceTextures::new(up, down, north, south, east, west, side_overlay, tint));
    }

    if let Some(all) = get("all") {
        return Some(FaceTextures::uniform(all, tint));
    }

    if let (Some(end), Some(side)) = (get("end"), get("side")) {
        return Some(FaceTextures::new(end, end, side, side, side, side, None, Tint::None));
    }

    if let (Some(top), Some(side)) = (get("top"), get("side")) {
        let bottom = get("bottom").unwrap_or(top);
        return Some(FaceTextures::new(top, bottom, side, side, side, side, None, tint));
    }

    if let Some(cross) = get("cross") {
        return Some(FaceTextures::uniform(cross, tint));
    }

    if let (Some(front), Some(side)) = (get("front"), get("side")) {
        let top = get("top").or(get("end")).unwrap_or(side);
        let bottom = get("bottom").unwrap_or(top);
        return Some(FaceTextures::new(top, bottom, front, side, side, side, None, Tint::None));
    }

    if let Some(p) = get("particle") {
        return Some(FaceTextures::uniform(p, tint));
    }

    None
}

fn determine_tint(block_name: &str) -> Tint {
    if GRASS_TINTED.contains(&block_name) {
        Tint::Grass
    } else if FOLIAGE_TINTED.contains(&block_name) || block_name.ends_with("_leaves") {
        Tint::Foliage
    } else {
        Tint::None
    }
}
