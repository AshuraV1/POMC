use std::collections::HashMap;
use std::path::Path;

use azalea_block::BlockState;
use serde::{Deserialize, Serialize};

use crate::assets::AssetIndex;

use super::model;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Tint {
    None,
    Grass,
    Foliage,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FaceTextures {
    pub top: String,
    pub bottom: String,
    pub north: String,
    pub south: String,
    pub east: String,
    pub west: String,
    pub side_overlay: Option<String>,
    pub tint: Tint,
}

impl FaceTextures {
    pub fn new(
        top: &str, bottom: &str, north: &str, south: &str, east: &str, west: &str,
        side_overlay: Option<&str>, tint: Tint,
    ) -> Self {
        Self {
            top: top.into(),
            bottom: bottom.into(),
            north: north.into(),
            south: south.into(),
            east: east.into(),
            west: west.into(),
            side_overlay: side_overlay.map(Into::into),
            tint,
        }
    }

    pub fn uniform(name: &str, tint: Tint) -> Self {
        Self::new(name, name, name, name, name, name, None, tint)
    }
}

#[derive(Clone)]
pub struct BlockRegistry {
    textures: HashMap<String, FaceTextures>,
}

impl BlockRegistry {
    pub fn load(assets_dir: &Path, asset_index: &Option<AssetIndex>, game_dir: &Path) -> Self {
        let cache_path = game_dir.join("pomc_block_cache.json");

        if let Some(cached) = load_cache(&cache_path) {
            log::info!("Block registry: {} blocks (cached)", cached.len());
            return Self { textures: cached };
        }

        let mut textures = model::load_all_block_textures(assets_dir, asset_index);

        textures.entry("water".into())
            .or_insert_with(|| FaceTextures::uniform("water_still", Tint::None));
        textures.entry("lava".into())
            .or_insert_with(|| FaceTextures::uniform("lava_still", Tint::None));

        save_cache(&cache_path, &textures);
        log::info!("Block registry: {} blocks (built and cached)", textures.len());
        Self { textures }
    }

    pub fn get_textures(&self, state: BlockState) -> Option<&FaceTextures> {
        let block: Box<dyn azalea_block::BlockTrait> = state.into();
        self.textures.get(block.id())
    }

    pub fn texture_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.textures.values().flat_map(|ft| {
            let base = [&ft.top, &ft.bottom, &ft.north, &ft.south, &ft.east, &ft.west];
            base.into_iter()
                .map(|s| s.as_str())
                .chain(ft.side_overlay.as_deref())
        })
    }
}

fn load_cache(path: &Path) -> Option<HashMap<String, FaceTextures>> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_cache(path: &Path, textures: &HashMap<String, FaceTextures>) {
    if let Ok(json) = serde_json::to_string(textures) {
        if let Err(e) = std::fs::write(path, json) {
            log::warn!("Failed to write block cache: {e}");
        }
    }
}
