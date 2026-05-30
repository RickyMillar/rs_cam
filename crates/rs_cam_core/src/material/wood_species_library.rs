//! Wood species library — Janka hardness lookup for the parametric
//! `Material::SolidWoodByJanka` variant. Surfaces species from the
//! FPL Wood Handbook Ch.5 (peer-reviewed, 2010) and The Wood
//! Database (cross-checked single-source), totalling ~130 species
//! beyond the 10 first-class `WoodSpecies` enum variants.
//!
//! Dedup precedence: FPL wins when the same common name appears in
//! both sources. Wood Database fills in species FPL Table 5-3a
//! doesn't cover (typically tropical species). See
//! `planning/feeds_data_ingest_phaseE_2026-05-31.md` for the dedup
//! reconciliation table.
//!
//! Generated 2026-05-31 from
//! `planning/data_ingest_2026-05-30/{fpl_ch5_extract,wood_database_species}.md`.
//! Regenerate via the script captured in `phaseE_2026-05-31.md` if
//! either source updates.

/// A single wood species entry in [`WOOD_SPECIES_LIBRARY`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WoodSpeciesEntry {
    /// Human-readable common name as printed by the source.
    pub display_name: &'static str,
    /// Binomial scientific name when known; `None` for species the
    /// staging source didn't pair with a binomial.
    pub scientific_name: Option<&'static str>,
    /// Janka side-hardness in lbf at 12% MC. For FPL entries this is
    /// converted from the published Newton value (`lbf = N / 4.448`,
    /// rounded to 1 decimal); Wood Database entries are quoted in lbf
    /// directly.
    pub janka_lbf: f64,
    /// Citation key — matches an entry in
    /// `crates/rs_cam_core/data/vendor_lut/source_manifest.json`.
    pub source_id: &'static str,
}

/// Curated wood-species library. Entries are kept in the published
/// order of each source (FPL first, Wood Database fill-ins second).
/// Use [`find_by_display_name`] for case-insensitive lookup.
pub const WOOD_SPECIES_LIBRARY: &[WoodSpeciesEntry] = &[
    WoodSpeciesEntry {
        display_name: "Alder, red",
        scientific_name: Some("Alnus rubra"),
        janka_lbf: 584.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Ash, black",
        scientific_name: Some("Fraxinus nigra"),
        janka_lbf: 854.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Ash, green",
        scientific_name: Some("Fraxinus pennsylvanica"),
        janka_lbf: 1191.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Ash, Oregon",
        scientific_name: Some("Fraxinus latifolia"),
        janka_lbf: 1169.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Ash, white",
        scientific_name: Some("Fraxinus americana"),
        janka_lbf: 1326.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Aspen, quaking",
        scientific_name: Some("Populus tremuloides"),
        janka_lbf: 359.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Basswood, American",
        scientific_name: Some("Tilia americana"),
        janka_lbf: 404.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Beech, American",
        scientific_name: Some("Fagus grandifolia"),
        janka_lbf: 1304.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Birch, paper",
        scientific_name: Some("Betula papyrifera"),
        janka_lbf: 899.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Birch, sweet",
        scientific_name: Some("Betula lenta"),
        janka_lbf: 1461.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Birch, yellow",
        scientific_name: Some("Betula alleghaniensis"),
        janka_lbf: 1259.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Butternut",
        scientific_name: Some("Juglans cinerea"),
        janka_lbf: 494.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cherry, black",
        scientific_name: Some("Prunus serotina"),
        janka_lbf: 944.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Chestnut, American",
        scientific_name: Some("Castanea dentata"),
        janka_lbf: 539.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cottonwood, black",
        scientific_name: Some("Populus trichocarpa"),
        janka_lbf: 359.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cottonwood, eastern",
        scientific_name: Some("Populus deltoides"),
        janka_lbf: 427.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Elm, American",
        scientific_name: Some("Ulmus americana"),
        janka_lbf: 831.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Elm, slippery",
        scientific_name: Some("Ulmus rubra"),
        janka_lbf: 854.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hackberry",
        scientific_name: Some("Celtis occidentalis"),
        janka_lbf: 876.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hickory (pecan), pecan",
        scientific_name: Some("Carya illinoinensis"),
        janka_lbf: 1821.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hickory (true), mockernut",
        scientific_name: Some("Carya tomentosa"),
        janka_lbf: 1978.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hickory (true), pignut",
        scientific_name: Some("Carya glabra"),
        janka_lbf: 2135.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hickory (true), shagbark",
        scientific_name: Some("Carya ovata"),
        janka_lbf: 1888.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hickory (true), shellbark",
        scientific_name: Some("Carya laciniosa"),
        janka_lbf: 1821.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Honeylocust",
        scientific_name: Some("Gleditsia triacanthos"),
        janka_lbf: 1573.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Locust, black",
        scientific_name: Some("Robinia pseudoacacia"),
        janka_lbf: 1708.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Magnolia, cucumbertree",
        scientific_name: Some("Magnolia acuminata"),
        janka_lbf: 696.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Magnolia, southern",
        scientific_name: Some("Magnolia grandiflora"),
        janka_lbf: 1011.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Maple, bigleaf",
        scientific_name: Some("Acer macrophyllum"),
        janka_lbf: 854.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Maple, black",
        scientific_name: Some("Acer nigrum"),
        janka_lbf: 1169.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Maple, red",
        scientific_name: Some("Acer rubrum"),
        janka_lbf: 944.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Maple, silver",
        scientific_name: Some("Acer saccharinum"),
        janka_lbf: 696.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Maple, sugar",
        scientific_name: Some("Acer saccharum"),
        janka_lbf: 1438.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, black (red oak group)",
        scientific_name: Some("Quercus velutina"),
        janka_lbf: 1214.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, cherrybark (red oak group)",
        scientific_name: Some("Quercus pagoda"),
        janka_lbf: 1483.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, laurel (red oak group)",
        scientific_name: Some("Quercus laurifolia"),
        janka_lbf: 1214.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, northern red",
        scientific_name: Some("Quercus rubra"),
        janka_lbf: 1281.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, pin (red oak group)",
        scientific_name: Some("Quercus palustris"),
        janka_lbf: 1506.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, scarlet (red oak group)",
        scientific_name: Some("Quercus coccinea"),
        janka_lbf: 1393.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, southern red",
        scientific_name: Some("Quercus falcata"),
        janka_lbf: 1056.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, water (red oak group)",
        scientific_name: Some("Quercus nigra"),
        janka_lbf: 1191.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, willow (red oak group)",
        scientific_name: Some("Quercus phellos"),
        janka_lbf: 1461.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, bur (white oak group)",
        scientific_name: Some("Quercus macrocarpa"),
        janka_lbf: 1371.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, chestnut (white oak group)",
        scientific_name: Some("Quercus prinus"),
        janka_lbf: 1124.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, overcup (white oak group)",
        scientific_name: Some("Quercus lyrata"),
        janka_lbf: 1191.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, post (white oak group)",
        scientific_name: Some("Quercus stellata"),
        janka_lbf: 1348.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, swamp chestnut (white oak group)",
        scientific_name: Some("Quercus michauxii"),
        janka_lbf: 1236.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, swamp white",
        scientific_name: Some("Quercus bicolor"),
        janka_lbf: 1618.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Oak, white",
        scientific_name: Some("Quercus alba"),
        janka_lbf: 1348.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Sweetgum",
        scientific_name: Some("Liquidambar styraciflua"),
        janka_lbf: 854.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Sycamore, American",
        scientific_name: Some("Platanus occidentalis"),
        janka_lbf: 764.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Tupelo, black",
        scientific_name: Some("Nyssa sylvatica"),
        janka_lbf: 809.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Tupelo, water",
        scientific_name: Some("Nyssa aquatica"),
        janka_lbf: 876.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Walnut, black",
        scientific_name: Some("Juglans nigra"),
        janka_lbf: 1011.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Yellow-poplar",
        scientific_name: Some("Liriodendron tulipifera"),
        janka_lbf: 539.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Baldcypress",
        scientific_name: Some("Taxodium distichum"),
        janka_lbf: 517.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, Atlantic white",
        scientific_name: Some("Chamaecyparis thyoides"),
        janka_lbf: 359.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, eastern redcedar",
        scientific_name: Some("Juniperus virginiana"),
        janka_lbf: 899.3,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, incense",
        scientific_name: Some("Calocedrus decurrens"),
        janka_lbf: 472.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, northern white",
        scientific_name: Some("Thuja occidentalis"),
        janka_lbf: 314.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, Port-Orford",
        scientific_name: Some("Chamaecyparis lawsoniana"),
        janka_lbf: 629.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, western redcedar",
        scientific_name: Some("Thuja plicata"),
        janka_lbf: 359.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Cedar, yellow",
        scientific_name: Some("Cupressus nootkatensis"),
        janka_lbf: 584.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Douglas-fir, coast",
        scientific_name: Some("Pseudotsuga menziesii"),
        janka_lbf: 719.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Douglas-fir, Interior West",
        scientific_name: Some("Pseudotsuga menziesii"),
        janka_lbf: 652.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Douglas-fir, Interior North",
        scientific_name: Some("Pseudotsuga menziesii"),
        janka_lbf: 607.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Douglas-fir, Interior South",
        scientific_name: Some("Pseudotsuga menziesii"),
        janka_lbf: 517.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, balsam",
        scientific_name: Some("Abies balsamea"),
        janka_lbf: 382.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, California red",
        scientific_name: Some("Abies magnifica"),
        janka_lbf: 494.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, grand",
        scientific_name: Some("Abies grandis"),
        janka_lbf: 494.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, noble",
        scientific_name: Some("Abies procera"),
        janka_lbf: 404.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, Pacific silver",
        scientific_name: Some("Abies amabilis"),
        janka_lbf: 427.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, subalpine",
        scientific_name: Some("Abies lasiocarpa"),
        janka_lbf: 359.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Fir, white",
        scientific_name: Some("Abies concolor"),
        janka_lbf: 472.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hemlock, eastern",
        scientific_name: Some("Tsuga canadensis"),
        janka_lbf: 494.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hemlock, mountain",
        scientific_name: Some("Tsuga mertensiana"),
        janka_lbf: 674.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Hemlock, western",
        scientific_name: Some("Tsuga heterophylla"),
        janka_lbf: 539.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Larch, western",
        scientific_name: Some("Larix occidentalis"),
        janka_lbf: 831.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, eastern white",
        scientific_name: Some("Pinus strobus"),
        janka_lbf: 382.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, jack",
        scientific_name: Some("Pinus banksiana"),
        janka_lbf: 562.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, loblolly",
        scientific_name: Some("Pinus taeda"),
        janka_lbf: 696.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, lodgepole",
        scientific_name: Some("Pinus contorta"),
        janka_lbf: 472.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, longleaf",
        scientific_name: Some("Pinus palustris"),
        janka_lbf: 876.8,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, ponderosa",
        scientific_name: Some("Pinus ponderosa"),
        janka_lbf: 449.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, red",
        scientific_name: Some("Pinus resinosa"),
        janka_lbf: 562.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, shortleaf",
        scientific_name: Some("Pinus echinata"),
        janka_lbf: 696.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, spruce",
        scientific_name: Some("Pinus glabra"),
        janka_lbf: 652.0,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, sugar",
        scientific_name: Some("Pinus lambertiana"),
        janka_lbf: 382.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, Virginia",
        scientific_name: Some("Pinus virginiana"),
        janka_lbf: 741.9,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Pine, western white",
        scientific_name: Some("Pinus monticola"),
        janka_lbf: 427.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Redwood, old-growth",
        scientific_name: Some("Sequoia sempervirens"),
        janka_lbf: 472.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Redwood, young-growth",
        scientific_name: Some("Sequoia sempervirens"),
        janka_lbf: 427.2,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Spruce, black",
        scientific_name: Some("Picea mariana"),
        janka_lbf: 539.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Spruce, Engelmann",
        scientific_name: Some("Picea engelmannii"),
        janka_lbf: 393.4,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Spruce, red",
        scientific_name: Some("Picea rubens"),
        janka_lbf: 494.6,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Spruce, Sitka",
        scientific_name: Some("Picea sitchensis"),
        janka_lbf: 517.1,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Spruce, white",
        scientific_name: Some("Picea glauca"),
        janka_lbf: 404.7,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Tamarack",
        scientific_name: Some("Larix laricina"),
        janka_lbf: 584.5,
        source_id: "fpl_ch5_2010",
    },
    WoodSpeciesEntry {
        display_name: "Red Oak (Northern)",
        scientific_name: Some("Quercus rubra"),
        janka_lbf: 1220.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Black Cherry",
        scientific_name: Some("Prunus serotina"),
        janka_lbf: 950.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Yellow Poplar",
        scientific_name: Some("Liriodendron tulipifera"),
        janka_lbf: 540.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "White Ash",
        scientific_name: Some("Fraxinus americana"),
        janka_lbf: 1320.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Honduran Mahogany",
        scientific_name: Some("Swietenia macrophylla"),
        janka_lbf: 900.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Douglas-Fir",
        scientific_name: Some("Pseudotsuga menziesii"),
        janka_lbf: 620.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Red Alder",
        scientific_name: Some("Alnus rubra"),
        janka_lbf: 590.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "American Beech",
        scientific_name: Some("Fagus grandifolia"),
        janka_lbf: 1300.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Shagbark Hickory",
        scientific_name: Some("Carya ovata"),
        janka_lbf: 1880.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Western Red Cedar",
        scientific_name: Some("Thuja plicata"),
        janka_lbf: 350.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Sapele",
        scientific_name: Some("Entandrophragma cylindricum"),
        janka_lbf: 1360.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "African Padauk",
        scientific_name: Some("Pterocarpus soyauxii"),
        janka_lbf: 1710.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Purpleheart",
        scientific_name: Some("Peltogyne spp"),
        janka_lbf: 2520.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Wenge",
        scientific_name: Some("Millettia laurentii"),
        janka_lbf: 1930.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Bubinga",
        scientific_name: Some("Guibourtia spp"),
        janka_lbf: 2410.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Zebrawood",
        scientific_name: Some("Microberlinia brazzavillensis"),
        janka_lbf: 1830.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Bloodwood",
        scientific_name: Some("Brosimum rubescens"),
        janka_lbf: 2900.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Cocobolo",
        scientific_name: Some("Dalbergia retusa"),
        janka_lbf: 2960.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Bocote",
        scientific_name: Some("Cordia spp"),
        janka_lbf: 2010.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Lacewood",
        scientific_name: Some("Panopsis spp"),
        janka_lbf: 840.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Yellow Birch",
        scientific_name: Some("Betula alleghaniensis"),
        janka_lbf: 1260.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Sweet Cherry",
        scientific_name: Some("Prunus avium"),
        janka_lbf: 1150.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Spanish Cedar",
        scientific_name: Some("Cedrela odorata"),
        janka_lbf: 600.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Loblolly Pine",
        scientific_name: Some("Pinus taeda"),
        janka_lbf: 690.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Soft Maple",
        scientific_name: Some("Acer rubrum"),
        janka_lbf: 950.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "European Beech",
        scientific_name: Some("Fagus sylvatica"),
        janka_lbf: 1450.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Teak",
        scientific_name: Some("Tectona grandis"),
        janka_lbf: 1070.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Gaboon Ebony",
        scientific_name: Some("Diospyros crassiflora"),
        janka_lbf: 3080.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "East Indian Rosewood",
        scientific_name: Some("Dalbergia latifolia"),
        janka_lbf: 2350.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Brazilian Rosewood",
        scientific_name: Some("Dalbergia nigra"),
        janka_lbf: 2790.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Light Red Meranti",
        scientific_name: Some("Shorea spp"),
        janka_lbf: 550.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Dark Red Meranti",
        scientific_name: Some("Shorea spp"),
        janka_lbf: 800.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "White Meranti",
        scientific_name: Some("Shorea spp"),
        janka_lbf: 1050.0,
        source_id: "wood_database_2026-05-30",
    },
    WoodSpeciesEntry {
        display_name: "Yellow Meranti",
        scientific_name: Some("Shorea spp"),
        janka_lbf: 700.0,
        source_id: "wood_database_2026-05-30",
    },
];

/// Case-insensitive lookup by display name. Returns `None` when no
/// library entry matches.
pub fn find_by_display_name(name: &str) -> Option<&'static WoodSpeciesEntry> {
    let lower = name.to_lowercase();
    WOOD_SPECIES_LIBRARY
        .iter()
        .find(|e| e.display_name.to_lowercase() == lower)
}
