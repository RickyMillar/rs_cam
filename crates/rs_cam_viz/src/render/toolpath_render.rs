use super::LineVertex;
use super::gpu_safety::{self, GpuLimits};
use egui_wgpu::wgpu;
use rs_cam_core::feeds::{AdvancePerToothMm, ChiploadBandClass, VendorChiploadBand};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, SpanClass};
use std::collections::HashMap;

// Re-export palette from centralized colors module for backward compatibility.
pub use super::colors::{TOOLPATH_PALETTE, palette_color};

// Per-move colour classification used to be a private `SpanColor` enum with
// its own walk of the span path, duplicated (differently, and wrongly) by the
// PNG exporter. The decision now lives in core as
// `AnnotatedToolpath::classify_span_path` -> `SpanClass`, and both renderers
// read it. The rules that function implements are *these* rules — this
// renderer is the reference; the exporter is the side that moved (X-1).

fn push_segment(out: &mut Vec<LineVertex>, p0: [f32; 3], p1: [f32; 3], color: [f32; 3]) {
    out.push(LineVertex {
        position: p0,
        color,
    });
    out.push(LineVertex {
        position: p1,
        color,
    });
}

/// Emit a single segment as two short sub-segments (start third + end third)
/// so it reads as a centre-gapped dash without needing a stipple shader.
fn push_dashed_segment(out: &mut Vec<LineVertex>, p0: [f32; 3], p1: [f32; 3], color: [f32; 3]) {
    let lerp = |t: f32| {
        [
            p0[0] + (p1[0] - p0[0]) * t,
            p0[1] + (p1[1] - p0[1]) * t,
            p0[2] + (p1[2] - p0[2]) * t,
        ]
    };
    let a = lerp(0.35);
    let b = lerp(0.65);
    out.push(LineVertex {
        position: p0,
        color,
    });
    out.push(LineVertex { position: a, color });
    out.push(LineVertex { position: b, color });
    out.push(LineVertex {
        position: p1,
        color,
    });
}

/// Toolpath line data uploaded to GPU.
/// Vertices are in move-sequence order so partial drawing works for simulation scrubbing.
pub struct ToolpathGpuData {
    /// Identifies which toolpath this GPU buffer came from, so the render
    /// loop can apply per-toolpath visibility overrides. `None` only during
    /// transient states where the id isn't known.
    pub toolpath_id: Option<rs_cam_core::ToolpathId>,
    /// What this buffer was built from (V8). Set by `upload_gpu_data` right
    /// after construction; the next upload pass reuses the buffers untouched
    /// when the freshly computed key compares equal. `None` on a buffer that
    /// has not been through the upload pass, which forces a rebuild.
    pub upload_key: Option<super::upload_cache::ToolpathUploadKey>,
    /// Cutting move vertices (line list, 2 verts per segment).
    pub cut_vertex_buffer: wgpu::Buffer,
    pub cut_vertex_count: u32,
    /// Rapid/link move vertices (line list, 2 verts per segment).
    pub rapid_vertex_buffer: wgpu::Buffer,
    pub rapid_vertex_count: u32,
    /// All moves interleaved in sequence order (for partial sim display).
    /// Each entry: (is_cutting, vertex_pair_index_in_respective_buffer).
    /// Used to compute how many cut/rapid verts to draw up to move N.
    pub move_cut_counts: Vec<u32>,
    pub move_rapid_counts: Vec<u32>,
    /// Entry path preview lines (ramp/helix/lead-in indicator) for selected toolpaths.
    pub entry_preview_buffer: Option<wgpu::Buffer>,
    pub entry_preview_count: u32,
    /// Tool-profile preview lines — a ghost of the cutter silhouette at each
    /// cutting endpoint, used to visualize what material the operation will
    /// remove before running a simulation.
    pub tool_profile_preview_buffer: Option<wgpu::Buffer>,
    pub tool_profile_preview_count: u32,
}

impl ToolpathGpuData {
    /// Compute how many cut and rapid vertices to draw for the first `n_moves` moves.
    #[allow(clippy::indexing_slicing)] // n - 1 is safe: n > 0 and n <= len
    pub fn vertices_for_moves(&self, n_moves: usize) -> (u32, u32) {
        let n = n_moves.min(self.move_cut_counts.len());
        if n == 0 {
            return (0, 0);
        }
        (self.move_cut_counts[n - 1], self.move_rapid_counts[n - 1])
    }

    /// Build GPU data from an [`AnnotatedToolpath`], coloring cutting moves by
    /// [`AnnotatedToolpath::classify_span_path`] and palette + Z-depth blend.
    ///
    /// Coloring:
    /// - `Entry` spans → bright cyan tint (overrides palette)
    /// - `LeadOut` spans → magenta tint
    /// - `LinkBridge` spans → emitted as a centre-gapped pair of sub-segments
    ///   (visual stipple) in dim grey
    /// - `DressupArtifact` spans → desaturated muted variant of the palette
    /// - Otherwise: palette + Z-depth blend, with `DepthPass` `pass_index`
    ///   producing a small per-pass lightness shift
    ///
    /// `index`: toolpath index for deterministic palette color.
    /// `selected`: if true, brighten the toolpath by +30%.
    ///
    /// Very large toolpaths are downsampled to fit within GPU buffer limits.
    /// The toolpath data itself is unchanged — only the visual representation
    /// is simplified.
    #[allow(clippy::indexing_slicing)] // loop index i bounded by tp.moves.len()
    pub fn from_toolpath(
        device: &wgpu::Device,
        limits: &GpuLimits,
        annotated: &AnnotatedToolpath,
        index: usize,
        selected: bool,
        span_filter: &crate::state::viewport::SpanKindFilter,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let tp = &annotated.toolpath;

        // Use actual device limit with 6% headroom instead of hardcoded value.
        let max_buffer_bytes: usize = (limits.max_buffer_size as f64 * 0.94) as usize;
        let vertex_size: usize = std::mem::size_of::<LineVertex>(); // 24 bytes
        let max_verts: usize = max_buffer_bytes / vertex_size;

        // Estimate total vertices (2 per move for line-list).
        let total_moves = tp.moves.len().saturating_sub(1);
        // Downsample stride: show every Nth move if too many vertices.
        let stride = if total_moves * 2 > max_verts {
            (total_moves * 2 / max_verts) + 1
        } else {
            1
        };

        let base = palette_color(index);

        // Find Z range for depth blending
        let mut z_min = f64::INFINITY;
        let mut z_max = f64::NEG_INFINITY;
        for m in &tp.moves {
            match m.move_type {
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    z_min = z_min.min(m.target.z);
                    z_max = z_max.max(m.target.z);
                }
                _ => {}
            }
        }
        let z_range = (z_max - z_min).max(1e-6);

        let mut cut_verts = Vec::new();
        let mut rapid_verts = Vec::new();
        let mut move_cut_counts = Vec::new();
        let mut move_rapid_counts = Vec::new();

        let brighten = |mut c: [f32; 3]| -> [f32; 3] {
            if selected {
                c[0] = (c[0] * 1.3).min(1.0);
                c[1] = (c[1] * 1.3).min(1.0);
                c[2] = (c[2] * 1.3).min(1.0);
            }
            c
        };

        // Per-pass lightness shift on top of the palette+Z blend so passes
        // visually stratify without losing the palette identity.
        let z_color = |z: f64, pass_index: Option<u32>| -> [f32; 3] {
            let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
            let depth_factor = 0.7 + t * 0.3; // darker at bottom, brighter at top
            let pass_shift = match pass_index {
                Some(p) => 1.0 + ((p as f32 % 4.0) - 1.5) * 0.06, // ±~9% spread across 4 passes
                None => 1.0,
            };
            let f = depth_factor * pass_shift;
            brighten([
                (base[0] * f).min(1.0),
                (base[1] * f).min(1.0),
                (base[2] * f).min(1.0),
            ])
        };

        let rapid_color = brighten([base[0] * 0.35, base[1] * 0.35, base[2] * 0.35]);
        let entry_color = brighten([0.20, 0.95, 0.95]);
        let leadout_color = brighten([0.95, 0.30, 0.85]);
        let linkbridge_color = brighten([0.45, 0.45, 0.55]);
        let dressup_color = brighten([
            0.5 * (base[0] + 0.5),
            0.5 * (base[1] + 0.5),
            0.5 * (base[2] + 0.5),
        ]);

        if stride > 1 {
            tracing::warn!(
                moves = total_moves,
                stride,
                "Toolpath too large for GPU buffer — downsampling for display"
            );
        }

        // Precompute per-move span paths so coloring is O(1) per move.
        let span_paths = annotated.span_paths_by_move();

        // Classify a move's span path into a coloring decision. The rules
        // (forward walk; Entry/LeadOut/LinkBridge win outright; Dressup is
        // remembered but does not short-circuit; DepthPass contributes
        // `pass_index`; GeometryRefit and Region are transparent) are
        // unchanged — they simply live in core now, so the PNG exporter can
        // apply the same ones instead of its own reversed walk (X-1).
        let classify = |move_idx: usize| -> SpanClass {
            match span_paths.get(move_idx) {
                Some(path) => annotated.classify_span_path(path),
                None => SpanClass::Cut { pass_index: None },
            }
        };

        for i in 1..tp.moves.len() {
            // When downsampling, skip intermediate moves but always keep
            // rapid moves (they define retract/approach structure) and
            // the first/last moves.
            let keep = stride == 1
                || i % stride == 0
                || i == 1
                || i == tp.moves.len() - 1
                || tp.moves[i].move_type == MoveType::Rapid;

            if keep {
                let from = tp.moves[i - 1].target;
                let to = tp.moves[i].target;
                let p0 = [from.x as f32, from.y as f32, from.z as f32];
                let p1 = [to.x as f32, to.y as f32, to.z as f32];

                match tp.moves[i].move_type {
                    MoveType::Rapid => {
                        rapid_verts.push(LineVertex {
                            position: p0,
                            color: rapid_color,
                        });
                        rapid_verts.push(LineVertex {
                            position: p1,
                            color: rapid_color,
                        });
                    }
                    _ => match classify(i) {
                        SpanClass::Entry if span_filter.show_entry => {
                            push_segment(&mut cut_verts, p0, p1, entry_color);
                        }
                        SpanClass::LeadOut if span_filter.show_lead_out => {
                            push_segment(&mut cut_verts, p0, p1, leadout_color);
                        }
                        SpanClass::LinkBridge if span_filter.show_link_bridge => {
                            push_dashed_segment(&mut cut_verts, p0, p1, linkbridge_color);
                        }
                        SpanClass::Dressup if span_filter.show_dressup => {
                            push_segment(&mut cut_verts, p0, p1, dressup_color);
                        }
                        SpanClass::Cut { pass_index } => {
                            let c0 = z_color(from.z, pass_index);
                            let c1 = z_color(to.z, pass_index);
                            cut_verts.push(LineVertex {
                                position: p0,
                                color: c0,
                            });
                            cut_verts.push(LineVertex {
                                position: p1,
                                color: c1,
                            });
                        }
                        // Filter hid this segment — emit nothing but still
                        // bump move_*_counts so scrubbing indices stay aligned.
                        _ => {}
                    },
                }
            }

            move_cut_counts.push(cut_verts.len() as u32);
            move_rapid_counts.push(rapid_verts.len() as u32);
        }

        // Ensure non-empty buffers
        if cut_verts.is_empty() {
            cut_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }
        if rapid_verts.is_empty() {
            rapid_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }

        let cut_vertex_count = cut_verts.len() as u32;
        let rapid_vertex_count = rapid_verts.len() as u32;

        // Use guarded buffer creation; fall back to placeholder on overflow
        // (shouldn't happen due to stride logic above, but defense-in-depth).
        let cut_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "toolpath_cut",
            bytemuck::cast_slice(&cut_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            let placeholder = [LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            }];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toolpath_cut_placeholder"),
                contents: bytemuck::cast_slice(&placeholder),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        let rapid_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "toolpath_rapid",
            bytemuck::cast_slice(&rapid_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            let placeholder = [LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            }];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("toolpath_rapid_placeholder"),
                contents: bytemuck::cast_slice(&placeholder),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        Self {
            toolpath_id: None,
            upload_key: None,
            cut_vertex_buffer,
            cut_vertex_count,
            rapid_vertex_buffer,
            rapid_vertex_count,
            move_cut_counts,
            move_rapid_counts,
            entry_preview_buffer: None,
            entry_preview_count: 0,
            tool_profile_preview_buffer: None,
            tool_profile_preview_count: 0,
        }
    }

    /// Attach entry path preview geometry for a selected toolpath.
    /// Call after `from_toolpath` to add the entry indicator overlay.
    pub fn attach_entry_preview(
        &mut self,
        device: &wgpu::Device,
        limits: &GpuLimits,
        verts: &[LineVertex],
    ) {
        if verts.is_empty() {
            return;
        }
        self.entry_preview_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "entry_preview",
            bytemuck::cast_slice(verts),
            wgpu::BufferUsages::VERTEX,
        );
        self.entry_preview_count = if self.entry_preview_buffer.is_some() {
            verts.len() as u32
        } else {
            0
        };
    }

    /// Attach a tool-profile preview overlay (circle outlines of the cutter
    /// silhouette sampled along the toolpath).
    pub fn attach_tool_profile_preview(
        &mut self,
        device: &wgpu::Device,
        limits: &GpuLimits,
        verts: &[LineVertex],
    ) {
        if verts.is_empty() {
            return;
        }
        self.tool_profile_preview_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "tool_profile_preview",
            bytemuck::cast_slice(verts),
            wgpu::BufferUsages::VERTEX,
        );
        self.tool_profile_preview_count = if self.tool_profile_preview_buffer.is_some() {
            verts.len() as u32
        } else {
            0
        };
    }

    /// Build GPU data colored by feed rate engagement.
    /// Green = nominal feed (light cut), yellow = reduced feed (moderate), red = heavily loaded.
    /// `nominal_feed` is the base feed rate for the operation.
    #[allow(clippy::indexing_slicing)]
    pub fn from_toolpath_engagement(
        device: &wgpu::Device,
        limits: &GpuLimits,
        tp: &Toolpath,
        nominal_feed: f64,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let max_buffer_bytes: usize = (limits.max_buffer_size as f64 * 0.94) as usize;
        let vertex_size: usize = std::mem::size_of::<LineVertex>();
        let max_verts: usize = max_buffer_bytes / vertex_size;
        let total_moves = tp.moves.len().saturating_sub(1);
        let stride = if total_moves * 2 > max_verts {
            (total_moves * 2 / max_verts) + 1
        } else {
            1
        };

        let nominal = nominal_feed.max(1.0);

        // Engagement color: feed_rate/nominal → color
        // ratio = 1.0 (nominal) → green, 0.5 → yellow, 0.0 → red
        let engagement_color = |feed: f64| -> [f32; 3] {
            let ratio = (feed / nominal).clamp(0.0, 1.5) as f32;
            if ratio >= 1.0 {
                // At or above nominal: green (light engagement / air cut)
                [0.2, 0.8, 0.3]
            } else if ratio >= 0.5 {
                // 50-100% of nominal: green → yellow
                let t = (ratio - 0.5) * 2.0; // 0..1
                [
                    0.2 + (1.0 - t) * 0.7,
                    0.8 - (1.0 - t) * 0.1,
                    0.3 - (1.0 - t) * 0.2,
                ]
            } else {
                // Below 50%: yellow → red (heavy engagement)
                let t = ratio * 2.0; // 0..1
                [0.9 - (1.0 - t) * 0.1, 0.7 * t, 0.1 * t]
            }
        };

        let rapid_color: [f32; 3] = [0.15, 0.15, 0.2];
        let mut cut_verts = Vec::new();
        let mut rapid_verts = Vec::new();
        let mut move_cut_counts = Vec::new();
        let mut move_rapid_counts = Vec::new();

        for i in 1..tp.moves.len() {
            let keep = stride == 1
                || i % stride == 0
                || i == 1
                || i == tp.moves.len() - 1
                || tp.moves[i].move_type == MoveType::Rapid;

            if keep {
                let from = tp.moves[i - 1].target;
                let to = tp.moves[i].target;
                let p0 = [from.x as f32, from.y as f32, from.z as f32];
                let p1 = [to.x as f32, to.y as f32, to.z as f32];

                match tp.moves[i].move_type {
                    MoveType::Rapid => {
                        rapid_verts.push(LineVertex {
                            position: p0,
                            color: rapid_color,
                        });
                        rapid_verts.push(LineVertex {
                            position: p1,
                            color: rapid_color,
                        });
                    }
                    _ => {
                        let feed = tp.moves[i].move_type.feed_rate().unwrap_or(nominal);
                        let c = engagement_color(feed);
                        cut_verts.push(LineVertex {
                            position: p0,
                            color: c,
                        });
                        cut_verts.push(LineVertex {
                            position: p1,
                            color: c,
                        });
                    }
                }
            }

            move_cut_counts.push(cut_verts.len() as u32);
            move_rapid_counts.push(rapid_verts.len() as u32);
        }

        if cut_verts.is_empty() {
            cut_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }
        if rapid_verts.is_empty() {
            rapid_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }

        let cut_vertex_count = cut_verts.len() as u32;
        let rapid_vertex_count = rapid_verts.len() as u32;

        let cut_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "tp_engagement_cut",
            bytemuck::cast_slice(&cut_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tp_engagement_cut_placeholder"),
                contents: bytemuck::cast_slice(&[LineVertex {
                    position: [0.0; 3],
                    color: [0.0; 3],
                }]),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        let rapid_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "tp_engagement_rapid",
            bytemuck::cast_slice(&rapid_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tp_engagement_rapid_placeholder"),
                contents: bytemuck::cast_slice(&[LineVertex {
                    position: [0.0; 3],
                    color: [0.0; 3],
                }]),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        Self {
            toolpath_id: None,
            upload_key: None,
            cut_vertex_buffer,
            cut_vertex_count,
            rapid_vertex_buffer,
            rapid_vertex_count,
            move_cut_counts,
            move_rapid_counts,
            entry_preview_buffer: None,
            entry_preview_count: 0,
            tool_profile_preview_buffer: None,
            tool_profile_preview_count: 0,
        }
    }

    /// Build GPU data coloured by per-segment **achieved advance per
    /// tooth** against the matched LUT row's vendor chipload band.
    ///
    /// `band`: the matched vendor row's window, in advance per tooth.
    /// `None` when the toolpath has no model-able band (custom material,
    /// no vendor data, etc.) — every cut segment is then rendered grey.
    ///
    /// `move_advance`: per-move achieved advance/tooth (worst-case across
    /// samples sharing the same `move_index`), from
    /// `rs_cam_core::tool_load::display::advance_per_tooth_per_move`.
    /// Moves missing from the map (rapids, samples with no usable
    /// `rpm × flutes` divisor) render dim grey.
    ///
    /// Until 2026-08-08 this took an arc-mean chip thickness and compared
    /// it to the same band — F-HEATMAP. The two arguments are now
    /// distinct types precisely so that pairing cannot be rebuilt.
    #[allow(clippy::indexing_slicing)]
    pub fn from_toolpath_advance_per_tooth(
        device: &wgpu::Device,
        limits: &GpuLimits,
        tp: &Toolpath,
        band: Option<&VendorChiploadBand>,
        move_advance: &HashMap<usize, AdvancePerToothMm>,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let max_buffer_bytes: usize = (limits.max_buffer_size as f64 * 0.94) as usize;
        let vertex_size: usize = std::mem::size_of::<LineVertex>();
        let max_verts: usize = max_buffer_bytes / vertex_size;
        let total_moves = tp.moves.len().saturating_sub(1);
        let stride = if total_moves * 2 > max_verts {
            (total_moves * 2 / max_verts) + 1
        } else {
            1
        };

        let advance_color = |move_idx: usize| -> [f32; 3] {
            advance_per_tooth_segment_color(band, move_advance.get(&move_idx).copied())
        };

        let rapid_color: [f32; 3] = [0.15, 0.15, 0.2];
        let mut cut_verts = Vec::new();
        let mut rapid_verts = Vec::new();
        let mut move_cut_counts = Vec::new();
        let mut move_rapid_counts = Vec::new();

        for i in 1..tp.moves.len() {
            let keep = stride == 1
                || i % stride == 0
                || i == 1
                || i == tp.moves.len() - 1
                || tp.moves[i].move_type == MoveType::Rapid;

            if keep {
                let from = tp.moves[i - 1].target;
                let to = tp.moves[i].target;
                let p0 = [from.x as f32, from.y as f32, from.z as f32];
                let p1 = [to.x as f32, to.y as f32, to.z as f32];

                match tp.moves[i].move_type {
                    MoveType::Rapid => {
                        rapid_verts.push(LineVertex {
                            position: p0,
                            color: rapid_color,
                        });
                        rapid_verts.push(LineVertex {
                            position: p1,
                            color: rapid_color,
                        });
                    }
                    _ => {
                        let c = advance_color(i);
                        cut_verts.push(LineVertex {
                            position: p0,
                            color: c,
                        });
                        cut_verts.push(LineVertex {
                            position: p1,
                            color: c,
                        });
                    }
                }
            }

            move_cut_counts.push(cut_verts.len() as u32);
            move_rapid_counts.push(rapid_verts.len() as u32);
        }

        if cut_verts.is_empty() {
            cut_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }
        if rapid_verts.is_empty() {
            rapid_verts.push(LineVertex {
                position: [0.0; 3],
                color: [0.0; 3],
            });
        }

        let cut_vertex_count = cut_verts.len() as u32;
        let rapid_vertex_count = rapid_verts.len() as u32;

        let cut_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "tp_chipload_cut",
            bytemuck::cast_slice(&cut_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tp_chipload_cut_placeholder"),
                contents: bytemuck::cast_slice(&[LineVertex {
                    position: [0.0; 3],
                    color: [0.0; 3],
                }]),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        let rapid_vertex_buffer = gpu_safety::try_create_buffer(
            device,
            limits,
            "tp_chipload_rapid",
            bytemuck::cast_slice(&rapid_verts),
            wgpu::BufferUsages::VERTEX,
        )
        .unwrap_or_else(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tp_chipload_rapid_placeholder"),
                contents: bytemuck::cast_slice(&[LineVertex {
                    position: [0.0; 3],
                    color: [0.0; 3],
                }]),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        Self {
            toolpath_id: None,
            upload_key: None,
            cut_vertex_buffer,
            cut_vertex_count,
            rapid_vertex_buffer,
            rapid_vertex_count,
            move_cut_counts,
            move_rapid_counts,
            entry_preview_buffer: None,
            entry_preview_count: 0,
            tool_profile_preview_buffer: None,
            tool_profile_preview_count: 0,
        }
    }
}

/// Map a per-move achieved advance per tooth against a vendor chipload
/// band to a segment colour. Pure function — no GPU state — kept
/// free-standing so it's unit-testable without a wgpu device.
///
/// **Where the band comparison lives.** The five classes and their
/// thresholds moved to `rs_cam_core::feeds::VendorChiploadBand::classify`
/// on 2026-08-08, so the same classification the A-1 two-arc fixture
/// asserts on is the one that paints the viewport. This function is now
/// only the class → RGB map, which is the part that is genuinely a viz
/// concern. The RGB triples are unchanged.
fn advance_per_tooth_segment_color(
    band: Option<&VendorChiploadBand>,
    observed: Option<AdvancePerToothMm>,
) -> [f32; 3] {
    let Some(band) = band else {
        return [0.40, 0.40, 0.40];
    };
    let Some(observed) = observed else {
        return [0.25, 0.25, 0.30];
    };
    match band.classify(observed) {
        // Under-engaged — rubbing risk.
        ChiploadBandClass::BelowBand => [0.20, 0.40, 0.90],
        // Just above the floor: blend blue → green.
        ChiploadBandClass::JustAboveFloor => {
            let t = band.floor_blend_fraction(observed) as f32;
            [
                0.20 + (0.00 - 0.20) * t,
                0.40 + (0.85 - 0.40) * t,
                0.90 + (0.30 - 0.90) * t,
            ]
        }
        ChiploadBandClass::Within => [0.20, 0.85, 0.30],
        ChiploadBandClass::NearCeiling => [1.00, 0.60, 0.10],
        ChiploadBandClass::AboveBand => [0.95, 0.20, 0.20],
    }
}

/// Entry/exit style configuration passed from the toolpath dressup settings.
///
/// `PartialEq` so the upload cache can key the selected toolpath's entry
/// overlay on the exact dial values it was drawn from (see
/// `upload_cache::ToolpathUploadKey`).
#[derive(Debug, Clone, PartialEq)]
pub struct EntryPreviewConfig {
    pub entry_style: EntryStyle,
    pub ramp_angle_deg: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,
    pub lead_in_out: bool,
    pub lead_radius: f64,
    /// Resolved feed_z and top_z heights for the entry path.
    pub feed_z: f64,
    pub top_z: f64,
}

/// Simplified entry style enum (mirrors DressupEntryStyle without serde dependency).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStyle {
    None,
    Ramp,
    Helix,
}

/// Bright cyan color for entry preview lines.
const ENTRY_PREVIEW_COLOR: [f32; 3] = [0.2, 0.9, 0.9];

/// Generate schematic entry path preview vertices for a toolpath.
///
/// Draws a visual indicator of the configured entry strategy at the first plunge point:
/// - Ramp: a sloped line descending from feed_z to top_z at the configured ramp angle
/// - Helix: a helical spiral descending from feed_z to top_z at the first move position
/// - Lead-in arc: a quarter-circle arc leading into the first cutting direction
///
/// Returns empty if entry_style is None or there are no cutting moves.
#[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
pub fn entry_preview_vertices(tp: &Toolpath, config: &EntryPreviewConfig) -> Vec<LineVertex> {
    let color = ENTRY_PREVIEW_COLOR;

    // Find the first non-Rapid move position (the first plunge/cut point)
    let Some(first_cut_idx) = tp
        .moves
        .iter()
        .enumerate()
        .position(|(i, m)| i > 0 && !matches!(m.move_type, MoveType::Rapid))
    else {
        return Vec::new();
    };

    let entry_pos = tp.moves[first_cut_idx].target;
    let approach_pos = tp.moves[first_cut_idx - 1].target;
    let mut verts = Vec::new();

    let feed_z = config.feed_z;
    let top_z = config.top_z;
    let z_drop = (feed_z - top_z).abs();

    match config.entry_style {
        EntryStyle::None => {}
        EntryStyle::Ramp => {
            if z_drop > 0.01 {
                // Compute horizontal distance from ramp angle
                let angle_rad = config.ramp_angle_deg.to_radians().max(0.1_f64.to_radians());
                let horiz_dist = z_drop / angle_rad.tan();

                // Direction of approach in XY
                let dx = entry_pos.x - approach_pos.x;
                let dy = entry_pos.y - approach_pos.y;
                let len = (dx * dx + dy * dy).sqrt();
                let (dir_x, dir_y) = if len < 1e-9 {
                    (1.0, 0.0)
                } else {
                    (dx / len, dy / len)
                };

                // Ramp starts offset backward from entry point at feed_z
                let start = [
                    (entry_pos.x - dir_x * horiz_dist) as f32,
                    (entry_pos.y - dir_y * horiz_dist) as f32,
                    feed_z as f32,
                ];
                let end = [entry_pos.x as f32, entry_pos.y as f32, top_z as f32];

                verts.push(LineVertex {
                    position: start,
                    color,
                });
                verts.push(LineVertex {
                    position: end,
                    color,
                });
            }
        }
        EntryStyle::Helix => {
            if z_drop > 0.01 && config.helix_radius > 0.01 {
                let segments = 16;
                let cx = entry_pos.x;
                let cy = entry_pos.y;
                let r = config.helix_radius;

                // Helix descends from feed_z to top_z over one or more turns
                // Number of turns based on pitch
                let pitch = config.helix_pitch.max(0.1);
                let turns = z_drop / pitch;
                let total_angle = turns * std::f64::consts::TAU;

                for i in 0..segments {
                    let t0 = i as f64 / segments as f64;
                    let t1 = (i + 1) as f64 / segments as f64;

                    let a0 = t0 * total_angle;
                    let a1 = t1 * total_angle;
                    let z0 = feed_z - t0 * z_drop;
                    let z1 = feed_z - t1 * z_drop;

                    verts.push(LineVertex {
                        position: [
                            (cx + r * a0.cos()) as f32,
                            (cy + r * a0.sin()) as f32,
                            z0 as f32,
                        ],
                        color,
                    });
                    verts.push(LineVertex {
                        position: [
                            (cx + r * a1.cos()) as f32,
                            (cy + r * a1.sin()) as f32,
                            z1 as f32,
                        ],
                        color,
                    });
                }
            }
        }
    }

    // Lead-in arc: quarter-circle arc before the first cutting move
    if config.lead_in_out && config.lead_radius > 0.01 {
        let dx = entry_pos.x - approach_pos.x;
        let dy = entry_pos.y - approach_pos.y;
        let len = (dx * dx + dy * dy).sqrt();
        let (dir_x, dir_y) = if len < 1e-9 {
            (1.0, 0.0)
        } else {
            (dx / len, dy / len)
        };

        // Arc center is offset perpendicular to approach direction by lead_radius
        let perp_x = -dir_y;
        let perp_y = dir_x;
        let arc_cx = entry_pos.x + perp_x * config.lead_radius;
        let arc_cy = entry_pos.y + perp_y * config.lead_radius;
        let r = config.lead_radius;

        // Quarter-circle arc from tangent approach to entry point
        let arc_segments = 8;
        let start_angle = std::f64::consts::PI; // start opposite to perpendicular
        let sweep = std::f64::consts::FRAC_PI_2; // 90 degrees

        for i in 0..arc_segments {
            let t0 = i as f64 / arc_segments as f64;
            let t1 = (i + 1) as f64 / arc_segments as f64;
            let a0 = start_angle + t0 * sweep;
            let a1 = start_angle + t1 * sweep;

            let z = entry_pos.z; // lead-in at entry Z level
            verts.push(LineVertex {
                position: [
                    (arc_cx + r * a0.cos()) as f32,
                    (arc_cy + r * a0.sin()) as f32,
                    z as f32,
                ],
                color,
            });
            verts.push(LineVertex {
                position: [
                    (arc_cx + r * a1.cos()) as f32,
                    (arc_cy + r * a1.sin()) as f32,
                    z as f32,
                ],
                color,
            });
        }
    }

    verts
}

/// Generate entry point marker vertices for a toolpath.
/// Returns line vertices forming a small arrowhead at the first cutting move position,
/// pointing in the direction of the first cut.
/// `palette_color`: the toolpath's palette color.
#[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
pub fn entry_marker_vertices(tp: &Toolpath, palette_color: [f32; 3]) -> Vec<LineVertex> {
    // Find the first non-Rapid move at index > 0
    let Some(first_cut_idx) = tp
        .moves
        .iter()
        .enumerate()
        .position(|(i, m)| i > 0 && !matches!(m.move_type, MoveType::Rapid))
    else {
        return Vec::new();
    };

    let approach = tp.moves[first_cut_idx - 1].target;
    let entry = tp.moves[first_cut_idx].target;

    // Direction from approach point to first cut position
    let dx = entry.x - approach.x;
    let dy = entry.y - approach.y;
    let len = (dx * dx + dy * dy).sqrt();

    // If approach and entry are coincident in XY, try to use (1, 0) as default direction
    let (dir_x, dir_y) = if len < 1e-9 {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    };

    // Arrowhead size in mm
    let size: f64 = 2.0;
    let wing_len = size * 0.6;

    // Tip of the arrow is at the entry point; tail is behind it
    let tip = [entry.x as f32, entry.y as f32, entry.z as f32];
    let tail = [
        (entry.x - dir_x * size) as f32,
        (entry.y - dir_y * size) as f32,
        entry.z as f32,
    ];

    // Wing vectors: rotate direction by ±30 degrees, pointing backward from tip
    let angle = std::f64::consts::FRAC_PI_6; // 30 degrees
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Backward direction (from tip toward tail)
    let back_x = -dir_x;
    let back_y = -dir_y;

    // Left wing: rotate backward direction by +30 degrees
    let lw_x = back_x * cos_a - back_y * sin_a;
    let lw_y = back_x * sin_a + back_y * cos_a;
    let left_wing = [
        (entry.x + lw_x * wing_len) as f32,
        (entry.y + lw_y * wing_len) as f32,
        entry.z as f32,
    ];

    // Right wing: rotate backward direction by -30 degrees
    let rw_x = back_x * cos_a + back_y * sin_a;
    let rw_y = -back_x * sin_a + back_y * cos_a;
    let right_wing = [
        (entry.x + rw_x * wing_len) as f32,
        (entry.y + rw_y * wing_len) as f32,
        entry.z as f32,
    ];

    vec![
        // Center line: tail to tip
        LineVertex {
            position: tail,
            color: palette_color,
        },
        LineVertex {
            position: tip,
            color: palette_color,
        },
        // Left wing: tip to left wing end
        LineVertex {
            position: tip,
            color: palette_color,
        },
        LineVertex {
            position: left_wing,
            color: palette_color,
        },
        // Right wing: tip to right wing end
        LineVertex {
            position: tip,
            color: palette_color,
        },
        LineVertex {
            position: right_wing,
            color: palette_color,
        },
    ]
}

/// Pale white-ish color for the tool-profile ghost overlay.
const TOOL_PROFILE_COLOR: [f32; 3] = [0.85, 0.85, 1.0];

/// Generate a "tool profile preview" overlay: circle outlines of the cutter
/// silhouette stacked along the cutter's length, drawn at a sampled subset
/// of cutting-move endpoints so the user can see the swept footprint before
/// running a simulation.
///
/// Sampling is capped so the output stays below ~64k vertices even on long
/// toolpaths; the stride is tuned from the cutting move count.
///
/// Each sample emits a fixed number of stacked circles (profile height
/// levels) as line loops (16 segments each).
pub fn tool_profile_preview_vertices(
    tp: &Toolpath,
    cutter: &dyn rs_cam_core::tool::MillingCutter,
) -> Vec<LineVertex> {
    const CIRCLE_SEGMENTS: usize = 16;
    const PROFILE_LEVELS: usize = 6;
    const TARGET_MAX_VERTS: usize = 64_000;

    // Collect cutting endpoints only (skip rapids).
    let cut_positions: Vec<rs_cam_core::geo::P3> = tp
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target)
        .collect();
    if cut_positions.is_empty() {
        return Vec::new();
    }

    let verts_per_sample = PROFILE_LEVELS * CIRCLE_SEGMENTS * 2;
    let max_samples = (TARGET_MAX_VERTS / verts_per_sample).max(1);
    let stride = cut_positions.len().div_ceil(max_samples).max(1);

    // Sample the cutter profile at evenly-spaced heights between 0 and
    // `cutter.cutting_length()` (or fall back to 2× radius if unknown).
    let max_h = cutter.length().max(cutter.radius() * 2.0);
    let heights: Vec<f64> = (0..PROFILE_LEVELS)
        .map(|i| (i as f64 / (PROFILE_LEVELS - 1).max(1) as f64) * max_h)
        .collect();

    let mut verts = Vec::with_capacity(cut_positions.len() / stride * verts_per_sample);
    for (i, pos) in cut_positions.iter().enumerate() {
        if i % stride != 0 {
            continue;
        }
        for &h in &heights {
            let r = cutter.width_at_height(h).max(0.05);
            // Circle in the XY plane at Z = pos.z + h, radius = r.
            for seg in 0..CIRCLE_SEGMENTS {
                let a0 = (seg as f64) / (CIRCLE_SEGMENTS as f64) * std::f64::consts::TAU;
                let a1 = ((seg + 1) as f64) / (CIRCLE_SEGMENTS as f64) * std::f64::consts::TAU;
                verts.push(LineVertex {
                    position: [
                        (pos.x + r * a0.cos()) as f32,
                        (pos.y + r * a0.sin()) as f32,
                        (pos.z + h) as f32,
                    ],
                    color: TOOL_PROFILE_COLOR,
                });
                verts.push(LineVertex {
                    position: [
                        (pos.x + r * a1.cos()) as f32,
                        (pos.y + r * a1.sin()) as f32,
                        (pos.z + h) as f32,
                    ],
                    color: TOOL_PROFILE_COLOR,
                });
            }
        }
    }
    verts
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// The six colour cases, byte-identical to the pre-2026-08-08
    /// assertions (band 0.05–0.10, the same five probe values). Only the
    /// *quantity* being classified changed at Checkpoint H; **no RGB
    /// triple and no threshold moved**, and these tests are the pin on
    /// that claim.
    fn probe_band() -> VendorChiploadBand {
        VendorChiploadBand::from_advance_range(&(0.05..0.10))
    }

    #[test]
    fn advance_color_no_band_is_grey() {
        let c = advance_per_tooth_segment_color(None, Some(AdvancePerToothMm::new(0.05)));
        assert_eq!(c, [0.40, 0.40, 0.40]);
    }

    #[test]
    fn advance_color_no_sample_is_dim_grey() {
        let c = advance_per_tooth_segment_color(Some(&probe_band()), None);
        assert_eq!(c, [0.25, 0.25, 0.30]);
    }

    #[test]
    fn advance_color_below_min_is_blue() {
        let c = advance_per_tooth_segment_color(
            Some(&probe_band()),
            Some(AdvancePerToothMm::new(0.04)),
        );
        assert_eq!(c, [0.20, 0.40, 0.90]);
    }

    #[test]
    fn advance_color_within_band_is_green() {
        let c = advance_per_tooth_segment_color(
            Some(&probe_band()),
            Some(AdvancePerToothMm::new(0.075)),
        );
        assert_eq!(c, [0.20, 0.85, 0.30]);
    }

    #[test]
    fn advance_color_near_max_is_orange() {
        let c = advance_per_tooth_segment_color(
            Some(&probe_band()),
            Some(AdvancePerToothMm::new(0.095)),
        );
        assert_eq!(c, [1.00, 0.60, 0.10]);
    }

    #[test]
    fn advance_color_above_max_is_red() {
        let c = advance_per_tooth_segment_color(
            Some(&probe_band()),
            Some(AdvancePerToothMm::new(0.12)),
        );
        assert_eq!(c, [0.95, 0.20, 0.20]);
    }

    /// A-2 screenshot sentry, automated half: **the colour and the gate
    /// verdict cannot disagree**, because both read one classification of
    /// one quantity. Walking a value across the band must produce the
    /// class sequence the gate's own bounds imply, in order, with no
    /// class reachable from two disjoint value regions.
    #[test]
    fn colour_classes_are_monotonic_across_the_band() {
        let band = probe_band();
        let mut seen: Vec<ChiploadBandClass> = Vec::new();
        let mut v = 0.0_f64;
        while v <= 0.15 {
            let c = band.classify(AdvancePerToothMm::new(v));
            if seen.last() != Some(&c) {
                assert!(
                    !seen.contains(&c),
                    "class {c:?} recurs after leaving it — the colour scale is not monotonic \
                     in the displayed quantity, so a colour would not identify a band position"
                );
                seen.push(c);
            }
            v += 0.0005;
        }
        assert_eq!(
            seen,
            vec![
                ChiploadBandClass::BelowBand,
                ChiploadBandClass::JustAboveFloor,
                ChiploadBandClass::Within,
                ChiploadBandClass::NearCeiling,
                ChiploadBandClass::AboveBand,
            ]
        );
    }

    #[test]
    fn dashed_segment_emits_two_sub_segments() {
        let mut out = Vec::new();
        push_dashed_segment(&mut out, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        // Two sub-segments → 4 vertices, with a centre gap between idx 1 and 2.
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].position, [0.0, 0.0, 0.0]);
        assert!((out[1].position[0] - 3.5).abs() < 1e-5);
        assert!((out[2].position[0] - 6.5).abs() < 1e-5);
        assert_eq!(out[3].position, [10.0, 0.0, 0.0]);
    }

    #[test]
    fn span_aware_renderer_colors_entry_distinct_from_default() {
        use rs_cam_core::geo::P3;
        use rs_cam_core::toolpath::{Move, MoveIntent, MoveType};
        use rs_cam_core::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

        // Build a 3-move toolpath: rapid → cut(entry) → cut(default).
        let mut tp = Toolpath::new();
        tp.moves.push(Move {
            target: P3::new(0.0, 0.0, 0.0),
            move_type: MoveType::Rapid,
            intent: MoveIntent::Unknown,
        });
        tp.moves.push(Move {
            target: P3::new(1.0, 0.0, 0.0),
            move_type: MoveType::Linear { feed_rate: 100.0 },
            intent: MoveIntent::Unknown,
        });
        tp.moves.push(Move {
            target: P3::new(2.0, 0.0, 0.0),
            move_type: MoveType::Linear { feed_rate: 100.0 },
            intent: MoveIntent::Unknown,
        });
        // Entry span covers move 1 only; default cut for move 2.
        let spans = vec![Span::new(1, 2, SpanKind::Entry)];
        let annotated = AnnotatedToolpath::with_spans(tp, spans);

        // Move 1 (entry) → SpanClass::Entry
        let paths = annotated.span_paths_by_move();
        let move1_path = paths.get(1).expect("move 1 path");
        let move2_path = paths.get(2).expect("move 2 path");
        assert!(!move1_path.is_empty());
        assert!(move2_path.is_empty());
    }
}
