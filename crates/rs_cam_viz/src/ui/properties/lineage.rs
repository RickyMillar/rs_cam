//! The Lineage of the toolpath inspector (feeds contract P5b, K8, ruling
//! Q7): which facts the generated toolpath used and how each is known.
//!
//! ONE closed `Details` under the issue stack, with a count. Open, it is
//! one `DataTable` row per fact: the cut, the pick, the stored speeds, each
//! tool fact the op read, the entry and the findings. No row wraps; the
//! source text is the hover (IC1, IC2). The basis glyph is the one
//! provenance vocabulary, `ProvKind` (decision D4). A fact with no
//! recorded state reads in the neutral role, never `CAUTION` (Q7: no stress
//! on an estimated fact).

use rs_cam_core::trace::lineage::{
    Lineage, ValueUse, axial_text, consequence_name, contract_name, entry_kind_name, radial_text,
};

use crate::ui::components::{DataRow, DataTable, Details, ProvKind, Source};

/// The label of the `Details`.
pub const LINEAGE_LABEL: &str = "Lineage";

/// One row of the open Lineage, before the kit draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct LineageRow {
    /// The row name.
    pub name: String,
    /// The value text, with its unit.
    pub value: String,
    /// The basis glyph, when the fact has a stamp.
    pub source: Option<ProvKind>,
    /// The source text (the hover).
    pub hover: String,
}

/// The rows of the open Lineage, in the mock order. A pure function of the
/// core record, so a sentry compares it field by field.
#[must_use]
pub fn lineage_rows(l: &Lineage) -> Vec<LineageRow> {
    let mut rows = Vec::new();
    let s = &l.speeds;
    let (axial, axial_mm) = axial_text(l.cut.axial);
    let (radial, radial_mm) = radial_text(l.cut.radial);
    let stamped = |u: &ValueUse| u.basis.as_ref().map(ProvKind::from);
    let stamp_hover = |u: &ValueUse| match &u.basis {
        Some(b) => {
            let kind = ProvKind::from(b);
            match &b.reference {
                Some(r) => format!("{}: {r}", kind.label()),
                None => kind.label().to_owned(),
            }
        }
        None => "No stamp: a stated value that nothing wrote.".to_owned(),
    };
    // The cut (K1). A depth per pass and a stepover carry their stamps.
    if let Some(mm) = axial_mm {
        let basis = match axial {
            "depth per pass" | "z step" => Some(&s.depth_per_pass),
            _ => None,
        };
        rows.push(LineageRow {
            name: capitalised(axial),
            value: format!("{mm:.2} mm"),
            source: basis.and_then(stamped).or(Some(ProvKind::Manual)),
            hover: basis.map_or_else(|| "The operator's value (CutSpec).".to_owned(), stamp_hover),
        });
    }
    if let Some(v) = radial_mm {
        let (unit, basis) = match radial {
            "stepover" => ("mm", Some(&s.stepover)),
            "angular step" => ("\u{00B0}", None),
            _ => ("mm", None),
        };
        rows.push(LineageRow {
            name: capitalised(radial),
            value: format!("{v:.2} {unit}"),
            source: basis.and_then(stamped).or(Some(ProvKind::Manual)),
            hover: basis.map_or_else(|| "The operator's value (CutSpec).".to_owned(), stamp_hover),
        });
    }
    // The envelope pick (K2b).
    if let Some(pick) = l.pick {
        let value = match (pick.ap_mm, pick.ae_mm) {
            (Some(ap), Some(ae)) => format!("{ap:.2} \u{00D7} {ae:.2} mm"),
            (Some(v), None) | (None, Some(v)) => format!("{v:.2} mm"),
            (None, None) => "picked".to_owned(),
        };
        rows.push(LineageRow {
            name: "Pick".to_owned(),
            value,
            source: Some(ProvKind::Manual),
            hover: "A point of the safe envelope that the operator applied.".to_owned(),
        });
    }
    // The stored speeds, one glyph per basis.
    for (name, unit, decimals, u) in [
        ("Spindle", "rpm", 0, &s.spindle_rpm),
        ("Feed", "mm/min", 0, &s.feed_rate),
        ("Plunge", "mm/min", 0, &s.plunge_rate),
        ("Ramp feed", "mm/min", 0, &s.ramp_feed_rate),
    ] {
        if let Some(v) = u.value {
            rows.push(LineageRow {
                name: name.to_owned(),
                value: format!("{v:.decimals$} {unit}"),
                source: stamped(u),
                hover: stamp_hover(u),
            });
        }
    }
    // The tool facts (K3): the kind is on hover; no glyph (ruling Q7).
    for f in &l.tool_facts {
        let unit = f.fact.unit();
        let value = if unit.is_empty() {
            format!("{:.0}", f.value)
        } else {
            format!("{:.2} {unit}", f.value)
        };
        rows.push(LineageRow {
            name: f.fact.label().to_owned(),
            value,
            source: None,
            hover: format!("The tool record: {}.", f.state.label()),
        });
    }
    // The entry (K4).
    let counts = l
        .entry
        .counts()
        .iter()
        .map(|(k, n)| format!("{} \u{00D7}{n}", entry_kind_name(*k)))
        .collect::<Vec<_>>()
        .join(", ");
    rows.push(LineageRow {
        name: "Entry".to_owned(),
        value: if counts.is_empty() {
            "none".to_owned()
        } else {
            counts
        },
        source: None,
        hover: format!("Entry contract: {}.", contract_name(l.entry.contract)),
    });
    // The findings (K6), the ids on hover.
    if !l.findings.is_empty() {
        let warns = l
            .findings
            .iter()
            .filter(|f| consequence_name(f.consequence).starts_with("warn"))
            .count();
        rows.push(LineageRow {
            name: "Findings".to_owned(),
            value: format!("{} \u{00B7} {warns} warn", l.findings.len()),
            source: None,
            hover: l
                .findings
                .iter()
                .map(|f| match &f.detail {
                    Some(detail) => {
                        format!("{} ({}): {detail}", f.id, consequence_name(f.consequence))
                    }
                    None => format!("{} ({})", f.id, consequence_name(f.consequence)),
                })
                .collect::<Vec<_>>()
                .join("\n"),
        });
    }
    rows
}

fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Draw the Lineage: ONE closed `Details` with the row count; open, the
/// rows of [`lineage_rows`] in one `DataTable`.
pub fn draw_lineage(ui: &mut egui::Ui, toolpath_id: rs_cam_core::ToolpathId, l: &Lineage) {
    let rows = lineage_rows(l);
    let _ = Details::new(("toolpath_lineage", toolpath_id.raw()))
        .label(LINEAGE_LABEL)
        .count(rows.len())
        .show(ui, |ui| {
            let _ = DataTable::new(("toolpath_lineage_rows", toolpath_id.raw()))
                .rows(rows.into_iter().map(|r| {
                    let row = DataRow::value(r.name, r.value).hover(r.hover);
                    match r.source {
                        Some(kind) => row.source(Source::Prov(kind, None)),
                        None => row,
                    }
                }))
                .show(ui);
        });
}
