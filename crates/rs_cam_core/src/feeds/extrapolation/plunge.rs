//! G10: the plunge claim. The milling plunge is a printed fraction of the
//! side feed that ships (`planning/extrapolation_2026-09-24/G10_PLAN.md`
//! §1 F1, §3 A1).
//!
//! The operator rulings Q4 and Q5 (`RULINGS.md`, "Operator rulings,
//! 2026-09-25: G10 entry parameters") set one form for four tool families:
//!
//! - a flat end mill plunges at feed / Z (Q4: the Amana Spektra rule, "Ramp
//!   Down = feed / flutes");
//! - a ball nose plunges at 0.50 x feed (Q5);
//! - a tapered ball nose plunges at 0.50 x feed (Q5);
//! - a 60° V-bit plunges at 0.33 x feed (Q5). The rule holds 1/3, the
//!   fraction that the IDC chart prints (20 / 60).
//!
//! The claim reads the tool family, the tool key, Z, the material and the
//! feed. It does not scale a LUT row's chip, so it is not a `LookupResult`
//! basis like `DrillBasis`. It is a rule table with evidence and a range,
//! like [`super::DRILL_RULES`] and [`super::FAMILY_RULES`].
//!
//! Every statement id is in `fetch/G10/statements.json`. Outside a rule's
//! range the claim refuses ([`PlungeRefusal`]), and the resolver
//! (`feeds::plunge`) falls back to the named repo rule.

use super::drill::SPEKTRA_DRILL_RULE;
use super::size::EXACT_DIAMETER_TOLERANCE;
use super::{ClaimConfidence, Gap};
use crate::feeds::vendor_lut::{MaterialFamily, ToolFamily};

/// The plunge rule in words, for the detail line.
pub const PLUNGE_RULE_TEXT: &str = "plunge = fraction x shipped side feed; the fraction is the \
     printed plunge / feed of the family";

/// The extra sentence of the flat end mill claim (rulings Q3 and Q4).
const FLAT_RAMP_DOWN_TEXT: &str =
    "Amana Ramp Down is the vertical (Z) rate (ruling Q3); ruling Q4 reads it as feed / Z";

/// The key that selects a rule's range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlungeKey {
    /// The tool diameter (mm), inside `lo_mm..=hi_mm`.
    Diameter { lo_mm: f64, hi_mm: f64 },
    /// The tip diameter (mm) of a tapered ball, inside `lo_mm..=hi_mm`.
    Tip { lo_mm: f64, hi_mm: f64 },
    /// The included angle (degrees) of a V-bit, within `tol_deg` of `deg`,
    /// at any diameter. The chart prints no diameter (the B4 `AngleKey`
    /// precedent).
    Angle { deg: f64, tol_deg: f64 },
}

impl PlungeKey {
    /// The range in words, for example "3.0-12.7 mm" or "60.0° ± 0.5°, any
    /// diameter".
    #[must_use]
    pub fn text(&self) -> String {
        match *self {
            Self::Diameter { lo_mm, hi_mm } => format!("{lo_mm:?}-{hi_mm:?} mm"),
            Self::Tip { lo_mm, hi_mm } => format!("tip {lo_mm:?}-{hi_mm:?} mm"),
            Self::Angle { deg, tol_deg } => format!("{deg:?}° ± {tol_deg:?}°, any diameter"),
        }
    }
}

/// The fraction of the shipped side feed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlungeFraction {
    /// `1 / Z`: the Amana "Ramp Down = feed / flutes" rule.
    OneOverZ,
    /// A fixed fraction at every printed Z.
    Fixed(f64),
}

/// One plunge rule: the tools it serves, its fraction and its evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlungeRule {
    /// The rule id. Provenance stamps it (`PublishedRule`).
    pub id: &'static str,
    /// The vendors of the witnesses, in short, for the card headline.
    pub vendors: &'static str,
    /// The tool family of the rule.
    pub tool_family: ToolFamily,
    /// The key and its printed range.
    pub key: PlungeKey,
    pub fraction: PlungeFraction,
    /// The flute counts that the witnesses print.
    pub flutes: &'static [u32],
    /// The material columns that the witnesses print.
    pub materials: &'static [MaterialFamily],
    /// What supports the rule, in words.
    pub witness: &'static str,
    /// The statement ids (`fetch/G10/statements.json`) of the witnesses.
    pub statements: &'static [&'static str],
    /// The witnesses that disagree, in words. The card names them.
    pub spread: &'static str,
    pub confidence: ClaimConfidence,
}

/// The material columns of every rule: the Sienci columns "Softwood, Soft
/// Plywood, MDF" and "Hardwood, Hard Plywood", the IDC wood chart and the
/// Amana columns.
const WOOD_PLYWOOD_MDF: &[MaterialFamily] = &[
    MaterialFamily::Softwood,
    MaterialFamily::Hardwood,
    MaterialFamily::PlywoodSoftwood,
    MaterialFamily::PlywoodHardwood,
    MaterialFamily::Mdf,
];

/// The flat end mill witnesses: the Amana Spektra rule and cells, the 60
/// Sienci flat rows, the IDC up-cut rows and the Carbide 3D S3 rows.
const FLAT_STATEMENTS: &[&str] = &[
    "g10-wood-amana-spektra-rule",
    "g10-wood-amana-spektra-2f-quarter",
    "g10-wood-amana-spektra-3f-quarter",
    "g10-wood-amana-spektra-3f-3-4",
    "g10-hobby-036",
    "g10-hobby-037",
    "g10-hobby-038",
    "g10-hobby-040",
    "g10-hobby-041",
    "g10-hobby-042",
    "g10-hobby-044",
    "g10-hobby-045",
    "g10-hobby-046",
    "g10-hobby-047",
    "g10-hobby-051",
    "g10-hobby-052",
    "g10-hobby-053",
    "g10-hobby-055",
    "g10-hobby-056",
    "g10-hobby-057",
    "g10-hobby-059",
    "g10-hobby-060",
    "g10-hobby-061",
    "g10-hobby-062",
    "g10-hobby-066",
    "g10-hobby-067",
    "g10-hobby-068",
    "g10-hobby-070",
    "g10-hobby-071",
    "g10-hobby-072",
    "g10-hobby-074",
    "g10-hobby-075",
    "g10-hobby-076",
    "g10-hobby-077",
    "g10-hobby-089",
    "g10-hobby-090",
    "g10-hobby-091",
    "g10-hobby-093",
    "g10-hobby-094",
    "g10-hobby-095",
    "g10-hobby-097",
    "g10-hobby-098",
    "g10-hobby-099",
    "g10-hobby-100",
    "g10-hobby-104",
    "g10-hobby-105",
    "g10-hobby-106",
    "g10-hobby-108",
    "g10-hobby-109",
    "g10-hobby-110",
    "g10-hobby-112",
    "g10-hobby-113",
    "g10-hobby-114",
    "g10-hobby-115",
    "g10-hobby-119",
    "g10-hobby-120",
    "g10-hobby-121",
    "g10-hobby-123",
    "g10-hobby-124",
    "g10-hobby-125",
    "g10-hobby-127",
    "g10-hobby-128",
    "g10-hobby-129",
    "g10-hobby-130",
    "g10-wood-idc-up-eighth",
    "g10-wood-idc-up-quarter",
    "g10-hobby-134",
    "g10-hobby-135",
    "g10-hobby-136",
    "g10-hobby-137",
    "g10-hobby-138",
];

/// The ball nose witnesses: the Sienci ball end mills (1/8 and 1/4 in),
/// the Sienci round groove bits (12.7-25.4 mm) and the Amana ball nose v7
/// rule.
const BALL_STATEMENTS: &[&str] = &[
    "g10-hobby-034",
    "g10-hobby-035",
    "g10-hobby-039",
    "g10-hobby-043",
    "g10-hobby-054",
    "g10-hobby-058",
    "g10-hobby-069",
    "g10-hobby-073",
    "g10-hobby-087",
    "g10-hobby-088",
    "g10-hobby-092",
    "g10-hobby-096",
    "g10-hobby-107",
    "g10-hobby-111",
    "g10-hobby-122",
    "g10-hobby-126",
    "g10-hobby-048",
    "g10-hobby-049",
    "g10-hobby-050",
    "g10-hobby-063",
    "g10-hobby-064",
    "g10-hobby-065",
    "g10-hobby-078",
    "g10-hobby-079",
    "g10-hobby-080",
    "g10-hobby-101",
    "g10-hobby-102",
    "g10-hobby-103",
    "g10-hobby-116",
    "g10-hobby-117",
    "g10-hobby-118",
    "g10-hobby-131",
    "g10-hobby-132",
    "g10-hobby-133",
    "g10-wood-amana-ballnose-rule",
];

/// The tapered ball witnesses: the Sienci tips 0.5 and 1.5875 mm, and the
/// IDC tip of 0.762 mm (tip radius 0.015 in).
const TAPERED_STATEMENTS: &[&str] = &[
    "g10-hobby-030",
    "g10-hobby-031",
    "g10-hobby-032",
    "g10-hobby-033",
    "g10-hobby-083",
    "g10-hobby-084",
    "g10-hobby-085",
    "g10-hobby-086",
    "g10-wood-idc-taperball",
];

/// The 60° V-bit witnesses: the Sienci V-bit rows and the IDC 60° row.
const VBIT60_STATEMENTS: &[&str] = &[
    "g10-hobby-028",
    "g10-hobby-029",
    "g10-hobby-081",
    "g10-hobby-082",
    "g10-wood-idc-v60",
];

/// The G10 plunge rules (rulings Q4 and Q5, 2026-09-25; operator decision
/// D1 strict for the tapered ball).
///
/// The flat end mill rule reads the range and the flute counts of the G6
/// drill rule ([`SPEKTRA_DRILL_RULE`]). When `DRILL_RULES` widens, this rule
/// follows with no edit.
pub const PLUNGE_RULES: &[PlungeRule] = &[
    PlungeRule {
        id: "g10_plunge_flat",
        vendors: "Amana Spektra rule; Sienci, IDC",
        tool_family: ToolFamily::FlatEnd,
        key: PlungeKey::Diameter {
            lo_mm: SPEKTRA_DRILL_RULE.range_mm.0,
            hi_mm: SPEKTRA_DRILL_RULE.range_mm.1,
        },
        fraction: PlungeFraction::OneOverZ,
        flutes: SPEKTRA_DRILL_RULE.flutes,
        materials: WOOD_PLYWOOD_MDF,
        witness: "Amana Spektra Spiral Plunge chart v24 rule and cells (Ramp Down = feed / \
                  flutes; the G6-verified cells set the range); Sienci metric chart, 60 flat \
                  rows at 0.500-0.505; IDC Woodcraft up-cut 1/8 and 1/4 in at 0.50; Carbide 3D \
                  S3 chart, median 0.49",
        statements: FLAT_STATEMENTS,
        spread: "IDC down-cut 0.30 and 0.43 (g10-wood-idc-down-eighth, \
                 g10-wood-idc-down-quarter); Carbide 3D Nomad 0.24-0.44 \
                 (g10-hobby-139..143)",
        confidence: ClaimConfidence::TwoWitnesses,
    },
    PlungeRule {
        id: "g10_plunge_ball",
        vendors: "Sienci, Amana",
        tool_family: ToolFamily::BallNose,
        key: PlungeKey::Diameter {
            lo_mm: 3.175,
            hi_mm: 25.4,
        },
        fraction: PlungeFraction::Fixed(0.50),
        flutes: &[2],
        materials: WOOD_PLYWOOD_MDF,
        witness: "Sienci metric chart, ball end mills 1/8 and 1/4 in at 0.500-0.503 and round \
                  groove bits 12.7-25.4 mm at 0.500; Amana ball nose chart v7 rule, Ramp Down = \
                  feed / flutes at 2 flutes (grade b)",
        statements: BALL_STATEMENTS,
        spread: "IDC Woodcraft 0.25 at 1/8 in (g10-wood-idc-ball-eighth) and 0.43 at 1/4 in \
                 (g10-wood-idc-ball-quarter)",
        confidence: ClaimConfidence::TwoWitnesses,
    },
    PlungeRule {
        id: "g10_plunge_tapered",
        vendors: "Sienci, IDC",
        tool_family: ToolFamily::TaperedBallNose,
        key: PlungeKey::Tip {
            lo_mm: 0.5,
            hi_mm: 1.5875,
        },
        fraction: PlungeFraction::Fixed(0.50),
        flutes: &[2],
        materials: WOOD_PLYWOOD_MDF,
        witness: "Sienci metric chart, tips 0.5 and 1.5875 mm at 0.50; IDC Woodcraft tip 0.762 \
                  mm, 10° taper, at 0.417",
        statements: TAPERED_STATEMENTS,
        spread: "Sienci fine 0.334 (g10-hobby-031)",
        confidence: ClaimConfidence::OneWitness,
    },
    PlungeRule {
        id: "g10_plunge_vbit60",
        vendors: "Sienci, IDC",
        tool_family: ToolFamily::ChamferVbit,
        key: PlungeKey::Angle {
            deg: 60.0,
            tol_deg: 0.5,
        },
        fraction: PlungeFraction::Fixed(1.0 / 3.0),
        flutes: &[2],
        materials: WOOD_PLYWOOD_MDF,
        witness: "Sienci metric chart, V-bit rows at 0.330-0.335 (no diameter printed); IDC \
                  Woodcraft 60° V-bit at 0.333",
        statements: VBIT60_STATEMENTS,
        spread: "the 90° V-bit disagrees (IDC g10-wood-idc-v90, 0.556); a 30° 1-flute V-bit \
                 (g10-wood-idc-v30, 0.571); the Amana insert V-groove chart prints 0.5-1.0 for \
                 another tool (g10-wood-amana-vgroove-rule)",
        confidence: ClaimConfidence::TwoWitnesses,
    },
];

/// The name of a tool family in card text.
#[must_use]
pub const fn tool_family_label(family: ToolFamily) -> &'static str {
    match family {
        ToolFamily::FlatEnd => "flat end mill",
        ToolFamily::BallNose => "ball nose",
        ToolFamily::TaperedBallNose => "tapered ball nose",
        ToolFamily::BullNose => "bull nose",
        ToolFamily::ChamferVbit => "V-bit",
        ToolFamily::FacingBit => "facing bit",
    }
}

/// "2 flutes" or "2-3 flutes".
fn flutes_text(flutes: &[u32]) -> String {
    match (flutes.first(), flutes.last()) {
        (Some(lo), Some(hi)) if lo == hi => format!("{lo} flutes"),
        (Some(lo), Some(hi)) => format!("{lo}-{hi} flutes"),
        _ => "no flute count".to_owned(),
    }
}

/// The statement ids with each run of consecutive `g10-hobby-NNN` ids
/// joined, for example "g10-hobby-030..033".
fn compact_ids(ids: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut run: Option<(&str, u32, u32, usize)> = None;
    let flush = |out: &mut Vec<String>, run: Option<(&str, u32, u32, usize)>| {
        if let Some((prefix, lo, hi, width)) = run {
            if lo == hi {
                out.push(format!("{prefix}{lo:0width$}"));
            } else {
                out.push(format!("{prefix}{lo:0width$}..{hi:0width$}"));
            }
        }
    };
    for id in ids {
        let parsed = id.rfind('-').and_then(|at| {
            let (prefix, digits) = id.split_at(at + 1);
            digits
                .parse::<u32>()
                .ok()
                .map(|n| (prefix, n, digits.len()))
        });
        match (parsed, run) {
            (Some((prefix, n, width)), Some((p, lo, hi, w)))
                if p == prefix && w == width && n == hi + 1 =>
            {
                run = Some((p, lo, n, w));
            }
            (Some((prefix, n, width)), _) => {
                flush(&mut out, run);
                run = Some((prefix, n, n, width));
            }
            (None, _) => {
                flush(&mut out, run);
                run = None;
                out.push((*id).to_owned());
            }
        }
    }
    flush(&mut out, run);
    out.join(", ")
}

/// One plunge claim: a stated rule gives the plunge as a fraction of the
/// shipped side feed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlungeClaim {
    /// Always [`Gap::Plunge`].
    pub gap: Gap,
    /// The rule that serves the tool.
    pub rule: &'static PlungeRule,
    /// The fraction at the tool's Z.
    pub fraction: f64,
    /// The tool's flute count (Z).
    pub flutes: u32,
}

impl PlungeClaim {
    /// The fraction in words: "feed / 2" for the 1/Z rule, else "0.50 x
    /// feed".
    #[must_use]
    pub fn fraction_text(&self) -> String {
        match self.rule.fraction {
            PlungeFraction::OneOverZ => format!("feed / {}", self.flutes),
            PlungeFraction::Fixed(f) => format!("{f:.2} x feed"),
        }
    }

    /// The card text: a headline and a detail line.
    ///
    /// Example headline: "plunge claim (G10 plunge): 0.50 x feed, ball nose
    /// 3.175-25.4 mm, 2 flutes". The detail names the rule, the witnesses,
    /// the statement ids, the spread and the range.
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        let rule = self.rule;
        let headline = format!(
            "plunge claim ({} {}): {}, {} {}, {}",
            self.gap.group(),
            self.gap.label(),
            self.fraction_text(),
            tool_family_label(rule.tool_family),
            rule.key.text(),
            flutes_text(rule.flutes)
        );
        let confidence = match rule.confidence {
            ClaimConfidence::OneWitness => "one witness per size",
            ClaimConfidence::TwoWitnesses => "two or more witnesses",
        };
        let flat = if matches!(rule.fraction, PlungeFraction::OneOverZ) {
            format!("; {FLAT_RAMP_DOWN_TEXT}")
        } else {
            String::new()
        };
        let detail = format!(
            "{PLUNGE_RULE_TEXT}; rule {} (rulings Q4 and Q5, 2026-09-25): x{:.3} at {} flutes; \
             witnesses: {}; statements {}; spread: {}; valid {}, {}, in softwood, hardwood, \
             plywood and MDF; {confidence}{flat}",
            rule.id,
            self.fraction,
            self.flutes,
            rule.witness,
            compact_ids(rule.statements),
            rule.spread,
            rule.key.text(),
            flutes_text(rule.flutes),
        );
        (headline, detail)
    }
}

/// Why no plunge rule serves the tool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlungeRefusal {
    /// No rule serves the tool family (a bull nose, a facing bit).
    NoRuleForFamily(ToolFamily),
    /// The key is outside the rule's printed range.
    OutsideRange {
        rule: &'static PlungeRule,
        key_value: f64,
    },
    /// The flute count is not printed.
    FlutesNotPrinted {
        rule: &'static PlungeRule,
        flutes: u32,
    },
    /// The included angle is not printed.
    AngleNotPrinted {
        rule: &'static PlungeRule,
        angle_deg: Option<f64>,
    },
    /// The material is not a printed column.
    MaterialNotPrinted(MaterialFamily),
}

impl PlungeRefusal {
    /// A short reason for the headline, for example "bull nose" or "6.35
    /// mm is outside 3.0-12.7 mm".
    #[must_use]
    pub fn short(&self) -> String {
        match *self {
            Self::NoRuleForFamily(ToolFamily::BullNose) => {
                "bull nose; nearest printed: Amana corner-radius Ramp Down = feed / flutes, no \
                 feed-paired witness"
                    .to_owned()
            }
            Self::NoRuleForFamily(family) => format!("{}: no rule", tool_family_label(family)),
            Self::OutsideRange { rule, key_value } => format!(
                "{} {key_value:?} mm is outside {}",
                tool_family_label(rule.tool_family),
                rule.key.text()
            ),
            Self::FlutesNotPrinted { rule, flutes } => format!(
                "{} with {flutes} flutes; the rule prints {}",
                tool_family_label(rule.tool_family),
                flutes_text(rule.flutes)
            ),
            Self::AngleNotPrinted { rule, angle_deg } => match angle_deg {
                Some(a) => format!("{a:?}° V-bit; the rule prints {}", rule.key.text()),
                None => format!("V-bit with no angle; the rule prints {}", rule.key.text()),
            },
            Self::MaterialNotPrinted(material) => {
                format!("{} is not a printed column", material_label(material))
            }
        }
    }

    /// One sentence for the card. It names the printed range.
    #[must_use]
    pub fn text(&self) -> String {
        match *self {
            Self::NoRuleForFamily(ToolFamily::BullNose) => {
                "No plunge rule covers a bull nose. The nearest printed figure is the Amana \
                 corner-radius rule, Ramp Down = feed / flutes (g10-wood-amana-bullnose-rule). No \
                 bull nose witness pairs a plunge with a feed."
                    .to_owned()
            }
            Self::NoRuleForFamily(family) => format!(
                "No plunge rule covers a {}. The rules cover flat end mills, ball noses, tapered \
                 ball noses and 60° V-bits.",
                tool_family_label(family)
            ),
            Self::OutsideRange { rule, key_value } => format!(
                "The {} rule {} covers {} only; this tool is {key_value:?} mm.",
                tool_family_label(rule.tool_family),
                rule.id,
                rule.key.text()
            ),
            Self::FlutesNotPrinted { rule, flutes } => format!(
                "The {} rule {} covers {} only; this tool has {flutes}.",
                tool_family_label(rule.tool_family),
                rule.id,
                flutes_text(rule.flutes)
            ),
            Self::AngleNotPrinted { rule, angle_deg } => {
                let angle = angle_deg.map_or_else(|| "no angle".to_owned(), |a| format!("{a:?}°"));
                format!(
                    "The V-bit rule {} covers {} only; this tool is {angle}.",
                    rule.id,
                    rule.key.text()
                )
            }
            Self::MaterialNotPrinted(material) => format!(
                "The plunge rules cover softwood, hardwood, plywood and MDF only; this material \
                 is {}.",
                material_label(material)
            ),
        }
    }
}

/// The material family in lower case, for example "acrylic".
fn material_label(material: MaterialFamily) -> String {
    format!("{material:?}").to_lowercase()
}

/// True when `value` is inside `lo..=hi`, with
/// [`EXACT_DIAMETER_TOLERANCE`] (relative) at each end.
fn in_range(value: f64, lo: f64, hi: f64) -> bool {
    value.is_finite()
        && value > 0.0
        && value / lo >= 1.0 - EXACT_DIAMETER_TOLERANCE
        && value / hi <= 1.0 + EXACT_DIAMETER_TOLERANCE
}

/// The plunge claim for one tool and material, or why no rule serves it.
///
/// - `diameter_mm` is the tool diameter;
/// - `tip_d_mm` is the tip diameter of a tapered ball;
/// - `angle_deg` is the included angle of a V-bit.
///
/// The checks run in this order: the family, the key, the flute count, the
/// material.
///
/// # Errors
///
/// The [`PlungeRefusal`] that names the printed range.
pub fn plunge_rule(
    family: ToolFamily,
    diameter_mm: f64,
    tip_d_mm: Option<f64>,
    angle_deg: Option<f64>,
    flutes: u32,
    material: MaterialFamily,
) -> Result<PlungeClaim, PlungeRefusal> {
    let rule = PLUNGE_RULES
        .iter()
        .find(|r| r.tool_family == family)
        .ok_or(PlungeRefusal::NoRuleForFamily(family))?;
    match rule.key {
        PlungeKey::Diameter { lo_mm, hi_mm } => {
            if !in_range(diameter_mm, lo_mm, hi_mm) {
                return Err(PlungeRefusal::OutsideRange {
                    rule,
                    key_value: diameter_mm,
                });
            }
        }
        PlungeKey::Tip { lo_mm, hi_mm } => {
            let tip = tip_d_mm.unwrap_or(diameter_mm);
            if tip_d_mm.is_none() || !in_range(tip, lo_mm, hi_mm) {
                return Err(PlungeRefusal::OutsideRange {
                    rule,
                    key_value: tip,
                });
            }
        }
        PlungeKey::Angle { deg, tol_deg } => {
            if !angle_deg.is_some_and(|a| a.is_finite() && (a - deg).abs() <= tol_deg) {
                return Err(PlungeRefusal::AngleNotPrinted { rule, angle_deg });
            }
        }
    }
    if !rule.flutes.contains(&flutes) {
        return Err(PlungeRefusal::FlutesNotPrinted { rule, flutes });
    }
    if !rule.materials.contains(&material) {
        return Err(PlungeRefusal::MaterialNotPrinted(material));
    }
    // `rule.flutes` holds no zero, so the divisor is positive.
    let fraction = match rule.fraction {
        PlungeFraction::OneOverZ => 1.0 / f64::from(flutes),
        PlungeFraction::Fixed(f) => f,
    };
    Ok(PlungeClaim {
        gap: Gap::Plunge,
        rule,
        fraction,
        flutes,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::feeds::extrapolation::DRILL_RULES;

    const WOOD: MaterialFamily = MaterialFamily::Hardwood;

    fn flat(d: f64, z: u32) -> Result<PlungeClaim, PlungeRefusal> {
        plunge_rule(ToolFamily::FlatEnd, d, None, None, z, WOOD)
    }

    /// Each rule accepts its own key: the FM1 flat and ball sizes, the
    /// printed tapered tips (no FM1 tapered tip is inside D1 strict) and the
    /// 60° V-bit.
    #[test]
    fn each_rule_accepts_its_own_key() {
        for (d, z) in [(3.175, 2), (6.0, 2), (6.0, 3), (6.35, 2), (12.7, 3)] {
            let claim = flat(d, z).unwrap_or_else(|e| panic!("{d} mm {z}F: {}", e.text()));
            assert_eq!(claim.rule.id, "g10_plunge_flat");
        }
        for d in [3.175, 6.0, 12.7, 25.4] {
            let claim = plunge_rule(ToolFamily::BallNose, d, None, None, 2, WOOD).unwrap();
            assert_eq!(claim.rule.id, "g10_plunge_ball");
        }
        for tip in [0.5, 0.762, 1.0, 1.5875] {
            let claim =
                plunge_rule(ToolFamily::TaperedBallNose, 6.35, Some(tip), None, 2, WOOD).unwrap();
            assert_eq!(claim.rule.id, "g10_plunge_tapered");
        }
        for d in [6.35, 12.7] {
            let claim = plunge_rule(ToolFamily::ChamferVbit, d, None, Some(60.0), 2, WOOD).unwrap();
            assert_eq!(claim.rule.id, "g10_plunge_vbit60");
        }
    }

    /// The flat rule refuses 16 mm and 2 mm (outside 3.0-12.7 mm), and 1
    /// and 4 flutes.
    #[test]
    fn the_flat_rule_refuses_outside_its_range_and_flutes() {
        for d in [16.0, 2.0] {
            let refusal = flat(d, 2).unwrap_err();
            assert!(
                matches!(refusal, PlungeRefusal::OutsideRange { .. }),
                "{d} mm: {refusal:?}"
            );
            assert!(refusal.text().contains("3.0-12.7 mm"), "{}", refusal.text());
        }
        for z in [1, 4] {
            let refusal = flat(6.0, z).unwrap_err();
            assert!(
                matches!(refusal, PlungeRefusal::FlutesNotPrinted { .. }),
                "{z}F: {refusal:?}"
            );
            assert!(refusal.text().contains("2-3 flutes"), "{}", refusal.text());
        }
    }

    /// The V-bit rule refuses 90°; the tapered rule refuses a 3.175 mm tip
    /// (D1 strict); a bull nose has no rule.
    #[test]
    fn the_other_rules_refuse_outside_their_keys() {
        let refusal =
            plunge_rule(ToolFamily::ChamferVbit, 12.7, None, Some(90.0), 2, WOOD).unwrap_err();
        assert!(matches!(refusal, PlungeRefusal::AngleNotPrinted { .. }));
        assert!(refusal.text().contains("60.0°"), "{}", refusal.text());
        let refusal = plunge_rule(
            ToolFamily::TaperedBallNose,
            6.35,
            Some(3.175),
            None,
            2,
            WOOD,
        )
        .unwrap_err();
        assert!(matches!(refusal, PlungeRefusal::OutsideRange { .. }));
        let refusal = plunge_rule(ToolFamily::BullNose, 6.0, None, None, 2, WOOD).unwrap_err();
        assert_eq!(
            refusal,
            PlungeRefusal::NoRuleForFamily(ToolFamily::BullNose)
        );
        assert!(
            refusal.text().contains("g10-wood-amana-bullnose-rule"),
            "{}",
            refusal.text()
        );
    }

    /// Every rule refuses acrylic.
    #[test]
    fn every_rule_refuses_acrylic() {
        let acrylic = MaterialFamily::Acrylic;
        for (family, d, tip, angle) in [
            (ToolFamily::FlatEnd, 6.0, None, None),
            (ToolFamily::BallNose, 6.0, None, None),
            (ToolFamily::TaperedBallNose, 6.35, Some(1.0), None),
            (ToolFamily::ChamferVbit, 12.7, None, Some(60.0)),
        ] {
            let refusal = plunge_rule(family, d, tip, angle, 2, acrylic).unwrap_err();
            assert_eq!(
                refusal,
                PlungeRefusal::MaterialNotPrinted(acrylic),
                "{family:?}"
            );
        }
    }

    /// The flat rule gives 1/2 at Z 2 and 1/3 at Z 3; the card states the
    /// Q3 and Q4 reading.
    #[test]
    fn the_flat_rule_is_one_over_z() {
        let two = flat(6.0, 2).unwrap();
        assert!((two.fraction - 0.5).abs() < 1e-15);
        let three = flat(6.0, 3).unwrap();
        assert!((three.fraction - 1.0 / 3.0).abs() < 1e-15);
        assert!((three.fraction - 0.333).abs() < 1e-3);
        let (headline, detail) = two.card_text();
        assert_eq!(
            headline,
            "plunge claim (G10 plunge): feed / 2, flat end mill 3.0-12.7 mm, 2-3 flutes"
        );
        for needle in [
            PLUNGE_RULE_TEXT,
            FLAT_RAMP_DOWN_TEXT,
            "g10-wood-amana-spektra-rule",
            "g10-hobby-036..038",
            "IDC down-cut",
        ] {
            assert!(detail.contains(needle), "{needle:?} missing from {detail}");
        }
        let ball = plunge_rule(ToolFamily::BallNose, 6.0, None, None, 2, WOOD).unwrap();
        assert_eq!(
            ball.card_text().0,
            "plunge claim (G10 plunge): 0.50 x feed, ball nose 3.175-25.4 mm, 2 flutes"
        );
    }

    /// The flat rule reads the G6 drill rule: its range and its flute
    /// counts are the `DRILL_RULES` values, not a copy.
    #[test]
    fn the_flat_range_is_the_drill_range() {
        let drill = DRILL_RULES.first().expect("one drill rule");
        let flat_rule = PLUNGE_RULES
            .iter()
            .find(|r| r.id == "g10_plunge_flat")
            .expect("the flat rule");
        assert_eq!(
            flat_rule.key,
            PlungeKey::Diameter {
                lo_mm: drill.range_mm.0,
                hi_mm: drill.range_mm.1,
            }
        );
        assert_eq!(flat_rule.flutes, drill.flutes);
    }

    /// Runs of statement ids print as one range.
    #[test]
    fn statement_runs_print_as_ranges() {
        assert_eq!(
            compact_ids(&[
                "g10-hobby-030",
                "g10-hobby-031",
                "g10-hobby-033",
                "g10-wood-idc-v60"
            ]),
            "g10-hobby-030..031, g10-hobby-033, g10-wood-idc-v60"
        );
    }
}
