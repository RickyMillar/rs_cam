//! Text + JSON report renderers for cell verdicts.

use super::shim::ShimSnapshot;
use super::verdict::CellVerdict;

pub fn render_text(verdict: &CellVerdict, snapshot: Option<&ShimSnapshot>) -> String {
    let mut s = String::new();
    s.push_str(&format!("cell: {} [{}]\n", verdict.cell_id, verdict.mode));
    if let Some(snap) = snapshot {
        s.push_str(&format!(
            "  rpm:        {:>8.0}    feed: {:>8.1} mm/min   fpt: {:>6.4} mm/tooth\n",
            snap.rpm, snap.feed_rate_mm_min, snap.effective_chip_load_mm
        ));
        s.push_str(&format!(
            "  doc:        {:>8.3} mm  woc:  {:>8.3} mm        plunge: {:>6.1} mm/min\n",
            snap.axial_doc_mm, snap.radial_woc_mm, snap.plunge_rate_mm_min
        ));
        s.push_str(&format!(
            "  power:      {:>8.3} kW  mrr:  {:>8.1} mm^3/min\n",
            snap.power_kw, snap.mrr_mm3_min
        ));
    }
    for row in &verdict.rows {
        s.push_str(&format!(
            "  {:<32}  {:<8}  {}\n",
            row.label,
            row.detail.verdict.as_str(),
            row.detail.reason
        ));
    }
    s.push_str(&format!("  verdict: {}\n", verdict.summary));
    s
}

pub fn render_json(verdict: &CellVerdict, snapshot: Option<&ShimSnapshot>) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "cell_id".into(),
        serde_json::Value::String(verdict.cell_id.clone()),
    );
    obj.insert(
        "overall".into(),
        serde_json::Value::String(verdict.overall.as_str().into()),
    );
    obj.insert(
        "summary".into(),
        serde_json::Value::String(verdict.summary.clone()),
    );
    obj.insert(
        "mode".into(),
        serde_json::Value::String(verdict.mode.clone()),
    );
    if let Some(snap) = snapshot {
        let mut sn = serde_json::Map::new();
        sn.insert("rpm".into(), serde_json::json!(snap.rpm));
        sn.insert(
            "feed_rate_mm_min".into(),
            serde_json::json!(snap.feed_rate_mm_min),
        );
        sn.insert(
            "effective_chip_load_mm".into(),
            serde_json::json!(snap.effective_chip_load_mm),
        );
        sn.insert("axial_doc_mm".into(), serde_json::json!(snap.axial_doc_mm));
        sn.insert(
            "radial_woc_mm".into(),
            serde_json::json!(snap.radial_woc_mm),
        );
        sn.insert(
            "plunge_rate_mm_min".into(),
            serde_json::json!(snap.plunge_rate_mm_min),
        );
        sn.insert("power_kw".into(), serde_json::json!(snap.power_kw));
        sn.insert("mrr_mm3_min".into(), serde_json::json!(snap.mrr_mm3_min));
        obj.insert("snapshot".into(), serde_json::Value::Object(sn));
    }
    let rows: Vec<serde_json::Value> = verdict
        .rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "label": r.label,
                "verdict": r.detail.verdict.as_str(),
                "reason": r.detail.reason,
                "severity_on_fail": r.severity_on_fail.as_str(),
            })
        })
        .collect();
    obj.insert("rows".into(), serde_json::Value::Array(rows));
    let value = serde_json::Value::Object(obj);
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
}
