//! Which `wgpu::PresentMode` the primary surface asks for — and which one it
//! actually got.
//!
//! **Checkpoint O, ruled 2026-08-13 (BINDING).** A GUI launched with `--mcp`
//! requests [`PresentMode::AutoNoVsync`]; a plain interactive launch keeps
//! [`PresentMode::AutoVsync`], which is exactly what
//! `WgpuConfiguration::default()` carries (`egui-wgpu-0.34.3/src/lib.rs:335`).
//! Nothing about a human session moves.
//!
//! **Why the flip exists (G-LV.1).** Under a real park — GNOME Wayland, window
//! minimised — the main thread blocks in `poll()` on one fd with an infinite
//! timeout, *below* winit, in the Wayland/Mesa WSI waiting for a FIFO buffer
//! release a minimised surface never gives. `AutoVsync` negotiates to `Fifo` on
//! that surface, so the shipped default *was* the hazard. Wave N-2 measured the
//! A/B (`planning/review_2026-08-08/PRESENT_MODE_AB.md`): FIFO parked the main
//! thread in **405/405** syscall samples and every frame-door MCP call timed out
//! at 15 s; `AutoNoVsync` (→ `Mailbox` there) parked it in **0/355**, kept the
//! minimised window painting, and answered every call live in 1.6–1.9 ms.
//!
//! **This fixes the minimise reproduction; incident-state coverage (occluded /
//! visible-frozen) is pending N-3 at close-out.** The 2026-08-07 incident and
//! the ~9.5 h A-1/A-2 park were occluded or merely unfocused, and one was
//! explicitly "visible". Whether those share the FIFO mechanism is not
//! established here.
//!
//! # Requested is not negotiated — and that is the whole reason this module
//! keeps a record
//!
//! `wgpu` resolves the two `Auto*` rules against the surface's real capability
//! list and logs its choice (`wgpu-core-29.0.3/src/device/resource.rs:4963-5001`).
//! `AutoNoVsync`'s rule is `Immediate → Mailbox → Fifo`, and it **always ends at
//! Fifo** so that it can never fail. That safety is also the hazard: on a
//! surface that offers neither `Immediate` nor `Mailbox`, `AutoNoVsync`
//! **silently restores the park**. Checkpoint O-2 therefore requires the
//! negotiated mode to be observable — logged at startup and reported in
//! `generation_status`'s `frame_loop` block — so "we quietly got Fifo back" can
//! never be a silent state. [`report`] is that block; [`capture_layer`] is how
//! the value is obtained.
//!
//! # The unsupported-explicit-mode startup crash, documented and LEFT
//!
//! An **explicit** mode the surface does not support is not a fallback: it is a
//! hard `UnsupportedPresentMode` error inside `Surface::configure`, and the
//! process dies before its first frame. N-2 reproduced it by asking for
//! `Immediate` on this machine, and the error names the caps verbatim
//! (`artifacts/n2/measurements/park_wayland_immediate/gui_stderr.log:12`):
//!
//! ```text
//! In Surface::configure
//!   Requested present mode Immediate is not in the list of supported present
//!   modes: [Mailbox, Fifo]
//! ```
//!
//! Checkpoint O-2 rules that this is **left unguarded**. The shipped paths —
//! interactive and `--mcp` — request only `Auto*` modes, which by construction
//! cannot hit it. The only way to reach it is [`RS_CAM_PRESENT_MODE`], which is
//! a measurement lever, not a product surface, and a rig that asks for an
//! unsupported mode should fail loudly. It is recorded here rather than fixed
//! because a crash nobody has written down is the thing worth avoiding.
//!
//! [`RS_CAM_PRESENT_MODE`]: #the-rig-lever

use std::sync::RwLock;

use egui_wgpu::wgpu::PresentMode;

/// Rig-only override. Unset — which is every real session — the mode comes from
/// [`decide`]'s `--mcp` rule and nothing reads this variable at all.
///
/// # The rig lever
///
/// Introduced by wave N-2 so a measurement could vary exactly one thing. It
/// **outranks** the `--mcp` rule, because a rig that asks for `fifo` under
/// `--mcp` is asking for the control arm and must get it. Accepted values are
/// case-insensitive: `auto_vsync`, `auto_no_vsync`, `fifo`, `fifo_relaxed`,
/// `mailbox`, `immediate`. An unrecognised value falls back to the launch's own
/// default rather than failing the launch.
pub const PRESENT_MODE_ENV: &str = "RS_CAM_PRESENT_MODE";

/// The `log` target `wgpu-core` emits its present-mode negotiation on.
///
/// `api_log!` is `log::trace!` unless wgpu-core is built with its
/// `api_log_info` feature (`wgpu-core-29.0.3/src/lib.rs:172-178`), which this
/// workspace does not enable — so the line is invisible at any normal log
/// level. [`capture_layer`] subscribes to this one target at `TRACE` rather
/// than turning trace logging on globally.
const WGPU_SURFACE_TARGET: &str = "wgpu_core::device::resource";

/// The exact sentence wgpu-core logs when an `Auto*` rule resolves.
///
/// Verbatim from `wgpu-core-29.0.3/src/device/resource.rs:4999`:
/// `"Automatically choosing presentation mode by rule {:?}. Chose {new_mode:?}"`.
const NEGOTIATION_PREFIX: &str = "Automatically choosing presentation mode by rule";

/// Where a launch's requested present mode came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSource {
    /// `--mcp` was passed and [`PRESENT_MODE_ENV`] was not set. Checkpoint O-1.
    McpDefault,
    /// A plain interactive launch. Today's behaviour, unchanged.
    InteractiveDefault,
    /// [`PRESENT_MODE_ENV`] was set and understood.
    RigOverride,
    /// [`PRESENT_MODE_ENV`] was set to something unrecognised; the launch
    /// default was used instead.
    RigOverrideUnrecognised,
}

impl RequestSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::McpDefault => "--mcp default (Checkpoint O-1)",
            Self::InteractiveDefault => "interactive default",
            Self::RigOverride => "RS_CAM_PRESENT_MODE",
            Self::RigOverrideUnrecognised => {
                "launch default (RS_CAM_PRESENT_MODE was set but not understood)"
            }
        }
    }
}

/// The process-wide record. Written twice at most — once by [`decide_and_record`]
/// before the window exists, once per surface configure by [`capture_layer`] —
/// and read from the MCP server thread.
static RECORD: RwLock<Record> = RwLock::new(Record {
    requested: None,
    source: None,
    negotiated: None,
});

#[derive(Debug)]
struct Record {
    requested: Option<PresentMode>,
    source: Option<RequestSource>,
    /// `None` until wgpu has told us. Deliberately **not** defaulted to the
    /// requested mode: for an `Auto*` request those are different questions and
    /// answering the second with the first is the silent-Fifo failure O-2
    /// exists to prevent.
    negotiated: Option<String>,
}

/// Decide the present mode for this launch, without touching global state.
///
/// Separated from [`decide_and_record`] so the ruling itself — `--mcp` gets
/// `AutoNoVsync`, everything else keeps `AutoVsync`, the rig lever outranks
/// both — is testable without a window, a GPU or a process-wide write.
pub fn decide(mcp_mode: bool, env: Option<&str>) -> (PresentMode, RequestSource) {
    let launch_default = if mcp_mode {
        // Checkpoint O-1. `AutoNoVsync` and not an explicit `Mailbox`: an
        // explicit mode the surface lacks is a startup crash (see the module
        // docs), and an agent session that cannot start is worse than one that
        // parks.
        PresentMode::AutoNoVsync
    } else {
        PresentMode::AutoVsync
    };
    let default_source = if mcp_mode {
        RequestSource::McpDefault
    } else {
        RequestSource::InteractiveDefault
    };

    let Some(raw) = env else {
        return (launch_default, default_source);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => (launch_default, default_source),
        "auto_vsync" | "autovsync" | "default" => {
            (PresentMode::AutoVsync, RequestSource::RigOverride)
        }
        "auto_no_vsync" | "autonovsync" => (PresentMode::AutoNoVsync, RequestSource::RigOverride),
        "fifo" => (PresentMode::Fifo, RequestSource::RigOverride),
        "fifo_relaxed" | "fiforelaxed" => (PresentMode::FifoRelaxed, RequestSource::RigOverride),
        "mailbox" => (PresentMode::Mailbox, RequestSource::RigOverride),
        "immediate" => (PresentMode::Immediate, RequestSource::RigOverride),
        _ => (launch_default, RequestSource::RigOverrideUnrecognised),
    }
}

/// [`decide`], plus the startup log line and the record [`report`] publishes.
pub fn decide_and_record(mcp_mode: bool) -> PresentMode {
    let raw = std::env::var(PRESENT_MODE_ENV).ok();
    let (mode, source) = decide(mcp_mode, raw.as_deref());

    if source == RequestSource::RigOverrideUnrecognised {
        tracing::warn!(
            "{PRESENT_MODE_ENV}={raw:?} is not a present mode I know; using {mode:?}. \
             Accepted: auto_vsync, auto_no_vsync, fifo, fifo_relaxed, mailbox, immediate."
        );
    }
    if let Ok(mut slot) = RECORD.write() {
        slot.requested = Some(mode);
        slot.source = Some(source);
    }

    // Checkpoint O-2, half one: logged at startup. The other half is
    // `generation_status`'s `frame_loop.present_mode` block.
    tracing::info!(
        "present mode REQUESTED {mode:?} ({}). For an Auto* rule this is not the mode in force — \
         wgpu resolves it against the surface's capabilities. The negotiated mode is logged next \
         and reported in generation_status's frame_loop.present_mode.",
        source.as_str()
    );
    mode
}

/// Record the mode wgpu says it negotiated. Called from [`capture_layer`].
fn record_negotiated(mode: &str) {
    let changed = match RECORD.read() {
        Ok(slot) => slot.negotiated.as_deref() != Some(mode),
        Err(_) => true,
    };
    if let Ok(mut slot) = RECORD.write() {
        slot.negotiated = Some(mode.to_owned());
    }
    if changed {
        tracing::info!(
            "present mode NEGOTIATED {mode}{}",
            if is_fifo_family(mode) {
                ". This is the FIFO family: on Wayland a hidden, occluded or screen-locked \
                 surface can block the main thread inside the present, which is G-LV.1. \
                 frame_loop.healthy reports it when it happens."
            } else {
                "."
            }
        );
    }
}

/// Whether a negotiated mode name is one of the two that carry the park hazard.
///
/// Name-based rather than typed because the value arrives as wgpu's own `Debug`
/// string, which is the authoritative record and the only one available.
fn is_fifo_family(mode: &str) -> bool {
    matches!(mode, "Fifo" | "FifoRelaxed")
}

/// Pull the chosen mode out of wgpu-core's negotiation line, or `None` if this
/// is some other message on the same target.
fn parse_negotiated(message: &str) -> Option<&str> {
    let message = message.trim();
    if !message.starts_with(NEGOTIATION_PREFIX) {
        return None;
    }
    let chosen = message.rsplit_once(" Chose ")?.1;
    let chosen = chosen.trim().trim_end_matches('.').trim();
    (!chosen.is_empty()).then_some(chosen)
}

/// The `frame_loop.present_mode` block.
///
/// Three fields, and the distinction between the first two is the point:
/// `requested` is what this process asked for, `negotiated` is what the surface
/// gave it, and they differ **by design** for every `Auto*` rule. A `null`
/// `negotiated` means the value was not observed — never that it equals
/// `requested`.
pub fn report() -> serde_json::Value {
    let Ok(slot) = RECORD.read() else {
        return serde_json::json!({
            "requested": serde_json::Value::Null,
            "negotiated": serde_json::Value::Null,
            "note": "present-mode record poisoned; not reported rather than guessed",
        });
    };

    let requested = slot.requested.map(|m| format!("{m:?}"));
    // An explicit mode that launched IS the mode in force: an unsupported
    // explicit mode is a hard error in `Surface::configure` and the process
    // would not be answering this call. An `Auto*` request tells us nothing.
    let explicit = slot
        .requested
        .filter(|m| !matches!(m, PresentMode::AutoVsync | PresentMode::AutoNoVsync));
    let negotiated = slot
        .negotiated
        .clone()
        .or_else(|| explicit.map(|m| format!("{m:?}")));
    let how = if slot.negotiated.is_some() {
        "wgpu-core's own negotiation log"
    } else if explicit.is_some() {
        "explicit request that launched cleanly (an unsupported explicit mode cannot start)"
    } else {
        "NOT OBSERVED — an Auto* rule was requested and wgpu's negotiation line was not seen"
    };

    let park_hazard = negotiated.as_deref().is_some_and(is_fifo_family);
    let mut block = serde_json::json!({
        "requested": requested,
        "requested_source": slot.source.map(RequestSource::as_str),
        "negotiated": negotiated,
        "negotiated_known_from": how,
    });
    if park_hazard && let Some(obj) = block.as_object_mut() {
        obj.insert(
            "park_hazard".to_owned(),
            serde_json::json!(
                "FIFO family: on Wayland a hidden, occluded or screen-locked surface can block \
                 the main thread inside the present (G-LV.1). Relaunch with WAYLAND_DISPLAY unset \
                 to use X11/XWayland, where dispatch survives."
            ),
        );
    }
    block
}

/// A `tracing` layer that reads the negotiated present mode off wgpu-core's own
/// log line, and a filter that switches exactly that one target on.
///
/// **Why a log scrape and not an API call.** Nothing in this stack exposes the
/// configured surface's present mode: `egui_wgpu::RenderState` carries the
/// adapter, device, queue and target format but not the surface
/// (`egui-wgpu-0.34.3/src/lib.rs:69-91`); eframe keeps the `Painter` private;
/// and `Surface::get_capabilities` needs a surface handle nobody hands out.
/// The one hook that *does* receive a `&Surface` is
/// `WgpuSetupCreateNew::native_adapter_selector`, and installing it **replaces**
/// eframe's adapter selection — trading an observability gap for a chance of
/// picking a different GPU, which is not a trade worth making. wgpu's own log
/// line is the value wgpu computed, so it cannot drift from the truth; what it
/// can do is disappear if wgpu rewords it, and then [`report`] says `NOT
/// OBSERVED` rather than guessing.
///
/// **Cost.** The filter names one target, so no other trace callsite is
/// enabled. `log`-originated records are re-checked per record rather than
/// cached by callsite, which costs a target-string comparison each; that is why
/// the caller installs this only under `--mcp`, where the flip is and where the
/// question is asked.
pub fn capture_layer<S>() -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::filter::{LevelFilter, Targets};

    NegotiationCapture
        .with_filter(Targets::new().with_target(WGPU_SURFACE_TARGET, LevelFilter::TRACE))
}

struct NegotiationCapture;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for NegotiationCapture {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = MessageVisitor(None);
        event.record(&mut visitor);
        if let Some(message) = visitor.0
            && let Some(mode) = parse_negotiated(&message)
        {
            record_negotiated(mode);
        }
    }
}

/// Lifts the `message` field out of an event. `log` records arrive with their
/// body in exactly that field (`tracing-log`'s `AsTrace` shim), and `Debug` on
/// `fmt::Arguments` renders the formatted text.
struct MessageVisitor(Option<String>);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" && self.0.is_none() {
            self.0 = Some(format!("{value:?}"));
        }
    }
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

    /// Checkpoint O-1, the shipped rule, both halves.
    #[test]
    fn mcp_launches_request_auto_no_vsync_and_plain_launches_do_not() {
        let (mcp, mcp_source) = decide(true, None);
        assert_eq!(
            mcp,
            PresentMode::AutoNoVsync,
            "Checkpoint O-1: a --mcp launch requests AutoNoVsync so the FIFO park cannot be the \
             shipped agent path"
        );
        assert_eq!(mcp_source, RequestSource::McpDefault);

        let (plain, plain_source) = decide(false, None);
        assert_eq!(
            plain,
            PresentMode::AutoVsync,
            "Checkpoint O-1: a plain interactive launch is UNCHANGED — AutoVsync is what \
             WgpuConfiguration::default() carries"
        );
        assert_eq!(plain_source, RequestSource::InteractiveDefault);
    }

    /// The rig lever outranks the ruling on both launch kinds, including the
    /// control arm (`fifo` under `--mcp`) N-2 needs to keep measuring.
    #[test]
    fn rig_override_outranks_the_launch_default() {
        assert_eq!(
            decide(true, Some("fifo")),
            (PresentMode::Fifo, RequestSource::RigOverride)
        );
        assert_eq!(
            decide(false, Some("Mailbox")),
            (PresentMode::Mailbox, RequestSource::RigOverride)
        );
        assert_eq!(
            decide(true, Some("  AUTO_VSYNC  ")),
            (PresentMode::AutoVsync, RequestSource::RigOverride)
        );
    }

    /// An unset-equivalent or unusable value must not silently move a launch
    /// off its ruled default.
    #[test]
    fn unusable_env_falls_back_to_the_launch_default() {
        assert_eq!(
            decide(true, Some("")),
            (PresentMode::AutoNoVsync, RequestSource::McpDefault)
        );
        assert_eq!(
            decide(true, Some("turbo")),
            (
                PresentMode::AutoNoVsync,
                RequestSource::RigOverrideUnrecognised
            )
        );
        assert_eq!(
            decide(false, Some("turbo")),
            (
                PresentMode::AutoVsync,
                RequestSource::RigOverrideUnrecognised
            )
        );
    }

    /// The string this parser depends on, quoted from
    /// `wgpu-core-29.0.3/src/device/resource.rs:4999` and rendered as wgpu
    /// renders it.
    #[test]
    fn parses_wgpu_core_negotiation_line() {
        assert_eq!(
            parse_negotiated(
                "Automatically choosing presentation mode by rule AutoNoVsync. Chose Mailbox"
            ),
            Some("Mailbox")
        );
        assert_eq!(
            parse_negotiated(
                "Automatically choosing presentation mode by rule AutoVsync. Chose Fifo"
            ),
            Some("Fifo")
        );
        // Other traffic on the same target must not be mistaken for it.
        assert_eq!(parse_negotiated("Device::create_buffer"), None);
        assert_eq!(
            parse_negotiated("Automatically choosing presentation mode by rule AutoVsync."),
            None
        );
    }

    /// O-2's whole point: an `Auto*` request must never be reported as if it
    /// were the answer, and a FIFO answer must say what it costs.
    ///
    /// One test and not three because [`RECORD`] is process-wide and the test
    /// harness runs threads in parallel — splitting these would make them race
    /// each other rather than test anything.
    #[test]
    fn report_distinguishes_requested_from_negotiated() {
        let mut slot = RECORD.write().unwrap();
        slot.requested = Some(PresentMode::AutoNoVsync);
        slot.source = Some(RequestSource::McpDefault);
        slot.negotiated = None;
        drop(slot);

        let block = report();
        assert_eq!(block["requested"], "AutoNoVsync");
        assert!(
            block["negotiated"].is_null(),
            "an unobserved Auto* negotiation is null, never echoed back as the request: {block}"
        );
        assert!(
            block["negotiated_known_from"]
                .as_str()
                .is_some_and(|s| s.contains("NOT OBSERVED")),
            "{block}"
        );

        record_negotiated("Fifo");
        let block = report();
        assert_eq!(block["negotiated"], "Fifo");
        assert!(
            block["park_hazard"].is_string(),
            "a silent Fifo fallback is the state O-2 exists to make visible: {block}"
        );

        record_negotiated("Mailbox");
        let block = report();
        assert_eq!(block["negotiated"], "Mailbox");
        assert!(block.get("park_hazard").is_none(), "{block}");

        // An explicit mode that launched is knowable without the log line,
        // because an unsupported explicit mode cannot launch at all.
        let mut slot = RECORD.write().unwrap();
        slot.requested = Some(PresentMode::Mailbox);
        slot.source = Some(RequestSource::RigOverride);
        slot.negotiated = None;
        drop(slot);

        let block = report();
        assert_eq!(block["negotiated"], "Mailbox");
        assert!(
            block["negotiated_known_from"]
                .as_str()
                .is_some_and(|s| s.contains("launched cleanly")),
            "{block}"
        );
    }
}
