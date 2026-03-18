//! Simple visualization output.
//!
//! Generates SVG files for visual verification of toolpaths.

use crate::geo::BoundingBox3;
use crate::toolpath::{MoveType, Toolpath};
use std::fmt::Write;

/// Generate an SVG showing the toolpath from a top-down (XY) view.
/// Z is encoded as color: deeper = darker blue, higher = lighter/warmer.
pub fn toolpath_to_svg(toolpath: &Toolpath, width: f64, height: f64) -> String {
    if toolpath.moves.is_empty() {
        return String::from("<svg xmlns='http://www.w3.org/2000/svg'/>");
    }

    // Find XY bounds
    let mut bbox = BoundingBox3::empty();
    for m in &toolpath.moves {
        bbox.expand_to(m.target);
    }

    let margin = 10.0;
    let data_w = bbox.max.x - bbox.min.x;
    let data_h = bbox.max.y - bbox.min.y;
    if data_w < 1e-10 || data_h < 1e-10 {
        return String::from("<svg xmlns='http://www.w3.org/2000/svg'/>");
    }

    let scale = ((width - 2.0 * margin) / data_w).min((height - 2.0 * margin) / data_h);
    let z_min = bbox.min.z;
    let z_range = (bbox.max.z - bbox.min.z).max(1e-6);

    let mut svg = String::new();
    writeln!(svg, "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>").unwrap();
    writeln!(svg, "<rect width='{width}' height='{height}' fill='#1a1a2e'/>").unwrap();

    // Draw rapids as thin gray dashed lines
    // Draw feed moves as colored lines (Z-based color)
    for i in 1..toolpath.moves.len() {
        let from = &toolpath.moves[i - 1].target;
        let to = &toolpath.moves[i].target;

        let x1 = margin + (from.x - bbox.min.x) * scale;
        let y1 = height - margin - (from.y - bbox.min.y) * scale; // flip Y
        let x2 = margin + (to.x - bbox.min.x) * scale;
        let y2 = height - margin - (to.y - bbox.min.y) * scale;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {
                writeln!(svg, "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#333' stroke-width='0.3' stroke-dasharray='2,2'/>").unwrap();
            }
            MoveType::Linear { .. } => {
                // Color by Z: low=deep blue, high=bright cyan/white
                let t = ((to.z - z_min) / z_range).clamp(0.0, 1.0);
                let r = (t * 100.0) as u8;
                let g = (80.0 + t * 175.0) as u8;
                let b = (180.0 + t * 75.0) as u8;
                writeln!(svg, "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#{r:02x}{g:02x}{b:02x}' stroke-width='0.5'/>").unwrap();
            }
        }
    }

    // Add legend
    writeln!(svg, "<text x='5' y='15' fill='white' font-size='10' font-family='monospace'>Z: {:.2} to {:.2} mm</text>", z_min, bbox.max.z).unwrap();
    writeln!(svg, "<text x='5' y='27' fill='white' font-size='10' font-family='monospace'>{} moves, {:.0}mm cutting</text>", toolpath.moves.len(), toolpath.total_cutting_distance()).unwrap();

    writeln!(svg, "</svg>").unwrap();
    svg
}
