use egui::Color32;

// --- Semantic status colors ---
pub const WARNING: Color32 = Color32::from_rgb(220, 180, 60);
pub const WARNING_MILD: Color32 = Color32::from_rgb(200, 170, 60);
pub const WARNING_TEXT: Color32 = Color32::from_rgb(255, 220, 100);

pub const ERROR: Color32 = Color32::from_rgb(220, 80, 80);
pub const ERROR_MILD: Color32 = Color32::from_rgb(200, 80, 80);

pub const SUCCESS: Color32 = Color32::from_rgb(100, 180, 100);
pub const SUCCESS_BRIGHT: Color32 = Color32::from_rgb(80, 180, 80);

pub const INFO: Color32 = Color32::from_rgb(100, 180, 220);

// --- Text hierarchy ---
pub const TEXT_HEADING: Color32 = Color32::from_rgb(180, 180, 195);
pub const TEXT_STRONG: Color32 = Color32::from_rgb(200, 205, 220);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 155);
pub const TEXT_DIM: Color32 = Color32::from_rgb(120, 120, 130);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(100, 100, 115);

// --- Accent ---
pub const ACCENT: Color32 = Color32::from_rgb(100, 160, 220);

// --- Card / frame ---
pub const CARD_FILL: Color32 = Color32::from_rgb(36, 36, 44);
pub const CARD_FILL_SELECTED: Color32 = Color32::from_rgb(38, 42, 55);

// --- Lane / status ---
pub const LANE_IDLE: Color32 = Color32::from_rgb(140, 140, 150);
pub const LANE_QUEUED: Color32 = Color32::from_rgb(150, 170, 210);
pub const LANE_RUNNING: Color32 = Color32::from_rgb(210, 190, 90);
pub const LANE_CANCELLING: Color32 = Color32::from_rgb(220, 120, 90);

/// Standard card frame for list items and info panels.
pub fn card_frame(selected: bool) -> egui::Frame {
    egui::Frame::default()
        .fill(if selected {
            CARD_FILL_SELECTED
        } else {
            CARD_FILL
        })
        .inner_margin(6.0)
        .rounding(4.0)
}
