pub mod common;
mod design_craft;
mod nature_worlds;
mod optical_space;
mod polar_winter;
mod worlds_tactical;

use crate::state::AppState;
use common::{PackedThemeParams, GLSL_COMMON};

pub fn resolve_shader_and_params(st: &AppState) -> (String, PackedThemeParams) {
    let (body, packed) = match st.theme_id.as_str() {
        "liquidglass" => (optical_space::LIQUIDGLASS_BODY, optical_space::pack_liquidglass(st)),
        "thinking-orbs" => (optical_space::THINKING_ORBS_BODY, optical_space::pack_thinking_orbs(st)),
        "solar-system" => (optical_space::SOLAR_SYSTEM_BODY, optical_space::pack_solar_system(st)),
        "studio-glass" => (optical_space::STUDIO_GLASS_BODY, optical_space::pack_studio_glass(st)),
        "beach" => (nature_worlds::BEACH_BODY, nature_worlds::pack_beach(st)),
        "meadow" => (nature_worlds::MEADOW_BODY, nature_worlds::pack_meadow(st)),
        "desert" => (nature_worlds::DESERT_BODY, nature_worlds::pack_desert(st)),
        "rosebrook" => (nature_worlds::ROSEBROOK_BODY, nature_worlds::pack_rosebrook(st)),
        "glade" => (nature_worlds::GLADE_BODY, nature_worlds::pack_glade(st)),
        "zen-archipelago" => (nature_worlds::ZEN_ARCHIPELAGO_BODY, nature_worlds::pack_zen_archipelago(st)),
        "aurora" => (polar_winter::AURORA_BODY, polar_winter::pack_aurora(st)),
        "polarnight" => (polar_winter::POLARNIGHT_BODY, polar_winter::pack_polarnight(st)),
        "quietsnow" => (polar_winter::QUIETSNOW_BODY, polar_winter::pack_quietsnow(st)),
        "winter" => (polar_winter::WINTER_BODY, polar_winter::pack_winter(st)),
        "neumorph" => (design_craft::NEUMORPH_BODY, design_craft::pack_neumorph(st)),
        "claymorph" => (design_craft::CLAYMORPH_BODY, design_craft::pack_claymorph(st)),
        "neobrutalism" => (design_craft::NEOBRUTALISM_BODY, design_craft::pack_neobrutalism(st)),
        "voxelcraft" => (design_craft::VOXELCRAFT_BODY, design_craft::pack_voxelcraft(st)),
        "papercut" => (design_craft::PAPERCUT_BODY, design_craft::pack_papercut(st)),
        "istanbul" => (design_craft::ISTANBUL_BODY, design_craft::pack_istanbul(st)),
        "software-world" => (worlds_tactical::SOFTWARE_WORLD_BODY, worlds_tactical::pack_software_world(st)),
        "rift" => (worlds_tactical::RIFT_BODY, worlds_tactical::pack_rift(st)),
        "strike" => (worlds_tactical::STRIKE_BODY, worlds_tactical::pack_strike(st)),
        "pocket" => (worlds_tactical::POCKET_BODY, worlds_tactical::pack_pocket(st)),
        "roach" => (worlds_tactical::ROACH_BODY, worlds_tactical::pack_roach(st)),
        _ => (polar_winter::AURORA_BODY, polar_winter::pack_aurora(st)),
    };
    let mut src = String::with_capacity(GLSL_COMMON.len() + body.len() + 2);
    src.push_str(GLSL_COMMON);
    src.push('\n');
    src.push_str(body);
    (src, packed)
}
