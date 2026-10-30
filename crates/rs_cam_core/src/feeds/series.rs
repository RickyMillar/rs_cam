//! The series tier of a vendor row (operator ruling 2026-10-31).
//!
//! The ruling: "A specialty-series row (ZrN carving, etc.) serves only a
//! tool of that series; a generic tool uses standard rows first and falls
//! back to a specialty row only when no standard row exists." The lead's
//! rulings of the same day name the specialty series: compression,
//! downcut, the ZrN carving rows, O-flute, single flute and the chipbreaker
//! roughing spirals. A standard row is a plain (upcut) spiral row. The
//! facing-bit `2_wing` rows stay unclassified: only a facing tool reads them.
//! The Onsrud series whose geometry no stored text states stay standard.
//!
//! # The row's series ([`row_series`])
//!
//! 1. An entry of [`SERIES_RULES`] names the `(source_id, tool_subfamily)`
//!    pair: its series (or `None`, a standard row) and its reason. The
//!    table holds the ZrN rows (no subfamily states the coating in a
//!    form the rule below can read) and the rows whose subfamily does not
//!    state a geometry that the same vendor states elsewhere.
//! 2. Else the subfamily states the geometry, in this order: `compression`
//!    is [`ToolSeries::Compression`]; `roughing_` with `spiral` is
//!    [`ToolSeries::Rougher`]; `o_flute` or `o-flute` is
//!    [`ToolSeries::OFlute`]; `single_flute` is [`ToolSeries::SingleFlute`];
//!    `downcut` or `down_spiral` is [`ToolSeries::Downcut`].
//! 3. Else the row is standard (`None`).
//!
//! # The tool's series ([`ToolSeries::for_cut_direction`])
//!
//! The tool declares its geometry series through the existing
//! `ToolConfig::cut_direction`: `UpCut` is a generic tool (no series),
//! `DownCut` and `Compression` name their series. No tool field names the
//! ZrN carving series yet: a lookup query can name it
//! (`LookupQuery::tool_subfamily = Some("zrn_carving")`), the product
//! doors do not.
//!
//! # The rank ([`series_rank`])
//!
//! The score picks the best row as before. The series decides only between
//! rows that tie on material and size (lead ruling 2026-10-31,
//! `vendor_lookup::prefer_series`): when another row is printed for the
//! same material family at the same size and ranks better (the tool's own
//! series, then the standard rows, then the other specialty rows), that row
//! takes the place of the best row. A specialty row still ships where no
//! better-ranked row prints its material and size.

use super::vendor_lut::VendorObservation;
use crate::compute::tool_config::BitCutDirection;

/// A cutter series that a generic (plain upcut spiral) end mill does not
/// have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSeries {
    /// A compression spiral (up and down cut on one tool).
    Compression,
    /// A downcut spiral.
    Downcut,
    /// The Amana ZrN-coated carving series.
    ZrnCarving,
    /// An O-flute (single or double, plastics and aluminium) cutter.
    OFlute,
    /// A single-flute cutter that is not an O-flute.
    SingleFlute,
    /// A chipbreaker roughing spiral.
    Rougher,
}

impl ToolSeries {
    /// Every series.
    pub const ALL: [Self; 6] = [
        Self::Compression,
        Self::Downcut,
        Self::ZrnCarving,
        Self::OFlute,
        Self::SingleFlute,
        Self::Rougher,
    ];

    /// The key that a lookup query carries (`LookupQuery::tool_subfamily`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Compression => "compression",
            Self::Downcut => "downcut",
            Self::ZrnCarving => "zrn_carving",
            Self::OFlute => "o_flute",
            Self::SingleFlute => "single_flute",
            Self::Rougher => "rougher",
        }
    }

    /// The series of a query key, if the key names one.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }

    /// The series that a tool's cut direction declares. `UpCut` is a
    /// generic tool.
    #[must_use]
    pub const fn for_cut_direction(direction: BitCutDirection) -> Option<Self> {
        match direction {
            BitCutDirection::UpCut => None,
            BitCutDirection::DownCut => Some(Self::Downcut),
            BitCutDirection::Compression => Some(Self::Compression),
        }
    }
}

/// One entry of [`SERIES_RULES`].
#[derive(Debug, Clone, Copy)]
pub struct SeriesRule {
    /// The row's `source_id`.
    pub source_id: &'static str,
    /// The row's `tool_subfamily`.
    pub tool_subfamily: &'static str,
    /// The series of the rows that this entry names; `None` is standard.
    pub series: Option<ToolSeries>,
    /// Why the rows have this series.
    pub reason: &'static str,
}

const ZRN_COATED: &str = "ZrN-coated carving series (operator ruling 2026-10-31: a specialty \
                          series serves only a tool of that series)";

/// The series table. A row that no entry names takes the subfamily rule
/// of [`row_series`].
pub const SERIES_RULES: &[SeriesRule] = &[
    SeriesRule {
        source_id: "amana_zrn_3d_profiling_v8",
        tool_subfamily: "zrn_2d3d_carving",
        series: Some(ToolSeries::ZrnCarving),
        reason: ZRN_COATED,
    },
    SeriesRule {
        source_id: "amana_zrn_3d_profiling_v8",
        tool_subfamily: "zrn",
        series: Some(ToolSeries::ZrnCarving),
        reason: ZRN_COATED,
    },
    SeriesRule {
        source_id: "amana_zrn_3d_profiling",
        tool_subfamily: "zrn",
        series: Some(ToolSeries::ZrnCarving),
        reason: ZRN_COATED,
    },
    SeriesRule {
        source_id: "amana_ball_nose_v7",
        tool_subfamily: "zrn",
        series: Some(ToolSeries::ZrnCarving),
        reason: "ZrN-coated ball nose rows of the v7 chart (aluminium columns); the uncoated \
                 rows of the chart are `solid_carbide`",
    },
    SeriesRule {
        source_id: "onsrud_cutting_data_recommendations",
        tool_subfamily: "zrn",
        series: Some(ToolSeries::ZrnCarving),
        reason: ZRN_COATED,
    },
    SeriesRule {
        source_id: "amana_zrn_aluminum_oflute_v13",
        tool_subfamily: "zrn_aluminum_o_flute",
        series: Some(ToolSeries::ZrnCarving),
        reason: "ZrN-coated single O-flute aluminium series",
    },
    SeriesRule {
        source_id: "amana_zrn_3d_profiling_v8",
        tool_subfamily: "zrn_2d3d_carving_tapered",
        series: None,
        reason: "the printed tip rows of the tapered ball nose: G3 ruling 2026-10-01 serves \
                 them to contour, scallop and trace; the lead kept them standard on \
                 2026-10-31",
    },
    SeriesRule {
        source_id: "amana_zrn_3d_profiling",
        tool_subfamily: "3d_profiling",
        series: None,
        reason: "the tapered ball nose rows of the ZrN 3D profiling chart: the same tapered \
                 ruling as the v8 tapered rows",
    },
    SeriesRule {
        source_id: "onsrud_hard_plastic_cutting_data",
        tool_subfamily: "60_200_series",
        series: Some(ToolSeries::Downcut),
        reason: "Onsrud 60-200 is the downcut series: the Onsrud wood rows of the same \
                 series carry the subfamily `60_200_downcut`",
    },
];

/// The entry of [`SERIES_RULES`] that names `obs`, if any.
#[must_use]
pub fn series_rule(obs: &VendorObservation) -> Option<&'static SeriesRule> {
    SERIES_RULES.iter().find(|rule| {
        obs.source_id == rule.source_id
            && obs.tool_subfamily.as_deref() == Some(rule.tool_subfamily)
    })
}

/// The series of `obs`: its [`SERIES_RULES`] entry, else the geometry that
/// its subfamily states, else `None` (a standard row).
#[must_use]
pub fn row_series(obs: &VendorObservation) -> Option<ToolSeries> {
    if let Some(rule) = series_rule(obs) {
        return rule.series;
    }
    let subfamily = obs.tool_subfamily.as_deref()?;
    if subfamily.contains("compression") {
        Some(ToolSeries::Compression)
    } else if subfamily.contains("roughing_") && subfamily.contains("spiral") {
        Some(ToolSeries::Rougher)
    } else if subfamily.contains("o_flute") || subfamily.contains("o-flute") {
        Some(ToolSeries::OFlute)
    } else if subfamily.contains("single_flute") {
        Some(ToolSeries::SingleFlute)
    } else if subfamily.contains("downcut") || subfamily.contains("down_spiral") {
        Some(ToolSeries::Downcut)
    } else {
        None
    }
}

/// True when `obs` is a standard row (no specialty series).
#[must_use]
pub fn is_standard(obs: &VendorObservation) -> bool {
    row_series(obs).is_none()
}

/// The series rank of `obs` for a query whose series key is `query_key`
/// (`LookupQuery::tool_subfamily`): 0 for a row of the tool's own series
/// (or a row whose subfamily is the key), 1 for a standard row, 2 for any
/// other specialty row. A lower rank wins a tie on material and size.
#[must_use]
pub fn series_rank(query_key: Option<&str>, obs: &VendorObservation) -> u8 {
    if query_key.is_some_and(|key| row_matches_key(key, obs)) {
        0
    } else if is_standard(obs) {
        1
    } else {
        2
    }
}

/// True when the row is of the series that `key` names, or its subfamily
/// is `key`.
#[must_use]
pub fn row_matches_key(key: &str, obs: &VendorObservation) -> bool {
    obs.tool_subfamily.as_deref() == Some(key)
        || ToolSeries::from_key(key).is_some_and(|s| row_series(obs) == Some(s))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::panic,
        reason = "test code: a panic is how a test reports a failure"
    )]
    use super::*;
    use crate::feeds::vendor_lut::VendorLut;

    fn names_zrn(obs: &VendorObservation) -> bool {
        obs.source_title.contains("ZrN")
            || obs
                .tool_subfamily
                .as_deref()
                .is_some_and(|s| s.contains("zrn"))
    }

    /// Every row that names ZrN in its source title or subfamily has an
    /// entry, so a new ZrN row cannot become standard by omission.
    #[test]
    fn every_zrn_row_is_classified() {
        let lut = VendorLut::embedded();
        let missing: Vec<_> = lut
            .observations
            .iter()
            .filter(|o| names_zrn(o) && series_rule(o).is_none())
            .map(|o| {
                format!(
                    "{} ({}, {:?})",
                    o.observation_id, o.source_id, o.tool_subfamily
                )
            })
            .collect();
        assert!(
            missing.is_empty(),
            "rows that name ZrN with no SERIES_RULES entry: {missing:?}"
        );
    }

    /// Every entry names at least one row: no dead entry.
    #[test]
    fn every_series_rule_names_a_row() {
        let lut = VendorLut::embedded();
        for rule in SERIES_RULES {
            assert!(
                lut.observations.iter().any(|o| {
                    o.source_id == rule.source_id
                        && o.tool_subfamily.as_deref() == Some(rule.tool_subfamily)
                }),
                "SERIES_RULES entry ({}, {}) names no row",
                rule.source_id,
                rule.tool_subfamily
            );
            assert!(!rule.reason.is_empty());
        }
    }

    /// The classes of the rows that the rulings name.
    #[test]
    fn the_ruled_rows_have_their_series() {
        let lut = VendorLut::embedded();
        let series_of = |id: &str| {
            let obs = lut
                .observations
                .iter()
                .find(|o| o.observation_id == id)
                .unwrap_or_else(|| panic!("row {id} is in the LUT"));
            row_series(obs)
        };
        assert_eq!(
            series_of("amana-zrn-flat-softwood-pocket-6000-2f"),
            Some(ToolSeries::ZrnCarving)
        );
        assert_eq!(
            series_of("amana-flat-softwood-pocket-6000-2f-spektra"),
            None
        );
        assert_eq!(series_of("amana-flat-softwood-pocket-6000-2f"), None);
        assert_eq!(
            series_of("onsrud-softwood-60-200-1_4-finish"),
            Some(ToolSeries::Downcut)
        );
        for obs in &lut.observations {
            let sub = obs.tool_subfamily.as_deref().unwrap_or("");
            if sub == "roughing_up_spiral" {
                assert_eq!(row_series(obs), Some(ToolSeries::Rougher));
            }
            if sub.contains("o_flute") && series_rule(obs).is_none() {
                assert_eq!(row_series(obs), Some(ToolSeries::OFlute));
            }
            if sub == "single_flute" {
                assert_eq!(row_series(obs), Some(ToolSeries::SingleFlute));
            }
            match obs.tool_subfamily.as_deref() {
                Some("zrn_2d3d_carving_tapered") => assert_eq!(row_series(obs), None),
                Some(s) if s.contains("compression") => {
                    assert_eq!(row_series(obs), Some(ToolSeries::Compression));
                }
                _ => {}
            }
        }
    }

    #[test]
    fn the_cut_direction_declares_the_series() {
        assert_eq!(ToolSeries::for_cut_direction(BitCutDirection::UpCut), None);
        assert_eq!(
            ToolSeries::for_cut_direction(BitCutDirection::DownCut),
            Some(ToolSeries::Downcut)
        );
        assert_eq!(
            ToolSeries::for_cut_direction(BitCutDirection::Compression),
            Some(ToolSeries::Compression)
        );
        for s in ToolSeries::ALL {
            assert_eq!(ToolSeries::from_key(s.key()), Some(s));
        }
    }
}
