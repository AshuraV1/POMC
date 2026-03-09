use std::collections::HashMap;

use azalea_block::BlockState;

#[derive(Clone, Copy)]
pub enum Tint {
    None,
    Grass,
    Foliage,
}

#[derive(Clone)]
pub struct FaceTextures {
    pub top: &'static str,
    pub bottom: &'static str,
    pub north: &'static str,
    pub south: &'static str,
    pub east: &'static str,
    pub west: &'static str,
    pub side_overlay: Option<&'static str>,
    pub tint: Tint,
}

impl FaceTextures {
    fn all(name: &'static str) -> Self {
        Self {
            top: name,
            bottom: name,
            north: name,
            south: name,
            east: name,
            west: name,
            side_overlay: None,
            tint: Tint::None,
        }
    }

    fn top_bottom_side(top: &'static str, bottom: &'static str, side: &'static str) -> Self {
        Self {
            top,
            bottom,
            north: side,
            south: side,
            east: side,
            west: side,
            side_overlay: None,
            tint: Tint::None,
        }
    }

    fn with_tint(mut self, tint: Tint) -> Self {
        self.tint = tint;
        self
    }

    fn with_side_overlay(mut self, overlay: &'static str) -> Self {
        self.side_overlay = Some(overlay);
        self
    }
}

#[derive(Clone)]
pub struct BlockRegistry {
    textures: HashMap<&'static str, FaceTextures>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        let mut textures = HashMap::new();

        let all = |name: &'static str| FaceTextures::all(name);
        let tbs = |t: &'static str, b: &'static str, s: &'static str| {
            FaceTextures::top_bottom_side(t, b, s)
        };

        textures.insert("stone", all("stone"));
        textures.insert("granite", all("granite"));
        textures.insert("polished_granite", all("polished_granite"));
        textures.insert("diorite", all("diorite"));
        textures.insert("polished_diorite", all("polished_diorite"));
        textures.insert("andesite", all("andesite"));
        textures.insert("polished_andesite", all("polished_andesite"));
        textures.insert(
            "grass_block",
            tbs("grass_block_top", "dirt", "grass_block_side")
                .with_tint(Tint::Grass)
                .with_side_overlay("grass_block_side_overlay"),
        );
        textures.insert("dirt", all("dirt"));
        textures.insert("coarse_dirt", all("coarse_dirt"));
        textures.insert("cobblestone", all("cobblestone"));
        textures.insert("bedrock", all("bedrock"));
        textures.insert("sand", all("sand"));
        textures.insert("red_sand", all("red_sand"));
        textures.insert("gravel", all("gravel"));
        textures.insert("oak_log", tbs("oak_log_top", "oak_log_top", "oak_log"));
        textures.insert("oak_planks", all("oak_planks"));
        textures.insert("oak_leaves", all("oak_leaves").with_tint(Tint::Foliage));
        textures.insert("glass", all("glass"));
        textures.insert("coal_ore", all("coal_ore"));
        textures.insert("iron_ore", all("iron_ore"));
        textures.insert("gold_ore", all("gold_ore"));
        textures.insert("diamond_ore", all("diamond_ore"));
        textures.insert(
            "deepslate",
            tbs("deepslate_top", "deepslate_top", "deepslate"),
        );
        textures.insert("cobbled_deepslate", all("cobbled_deepslate"));
        textures.insert("tuff", all("tuff"));
        textures.insert("water", all("water_still"));
        textures.insert("lava", all("lava_still"));
        textures.insert("clay", all("clay"));
        textures.insert("snow_block", all("snow"));
        textures.insert("short_grass", all("short_grass").with_tint(Tint::Grass));

        Self { textures }
    }

    pub fn get_textures(&self, state: BlockState) -> Option<&FaceTextures> {
        let block: Box<dyn azalea_block::BlockTrait> = state.into();
        let name = block.id();
        self.textures.get(name)
    }

    pub fn texture_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.textures.values().flat_map(|ft| {
            let base = [ft.top, ft.bottom, ft.north, ft.south, ft.east, ft.west];
            base.into_iter().chain(ft.side_overlay)
        })
    }
}
