//! The MCP command route (WP4): wire request in, `Command` out, one
//! reply.
//!
//! `app/mcp.rs` holds ONE dispatch arm for every registry `Command` row
//! the wire reaches. That arm calls the three steps in this module:
//!
//! 1. [`RsCamApp::mcp_before_core`] puts the operator's screen on the
//!    object the command is about. It runs BEFORE the mutation, as the
//!    per-row arms did, so a refused request leaves the same view a
//!    refused request left before.
//! 2. [`RsCamApp::core_command_for`] turns the `rs_cam_mcp` parameter
//!    struct into the row's core `*Args` payload and reads the evidence
//!    the reply needs from before the mutation. A conversion that cannot
//!    build a command answers with the refusal itself.
//! 3. [`RsCamApp::describe_core`] builds the reply and the view writes
//!    that belong after the mutation.
//!
//! The toast is the fourth step and it stands apart:
//! [`RsCamApp::core_toast_for`] reads it off the REQUEST, before the
//! conversion, so a request the conversion refuses still toasts. The
//! per-row arms worked that way too, and a toast built in the describe
//! step would be silent on every conversion refusal — the Suggest door's
//! refusal of a Scallop on a flat end mill among them (UX-R03-003).
//!
//! Both matches are exhaustive and name no wildcard, so a new registry
//! row does not compile until it has an arm here.
//!
//! **The conversion runs on the GUI thread, not on the wire.** Twenty of
//! these rows read the session to build their payload — the heights patch
//! reads the current heights, the spindle policy reads the current post
//! block, the two library rows read a catalog, the import reads a file —
//! and the MCP server thread holds no session. Moving the parse to that
//! thread would also reorder every refusal, because a handler validates
//! the index before it parses the value.

use std::path::{Path, PathBuf};

use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DressupConfig, HeightMode,
};
use rs_cam_core::compute::tool_config::ToolId;
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::session::{
    AddAlignmentPinArgs, AddModelArgs, AddSetupArgs, AddToolArgs, AddToolpathArgs, Command,
    CommandId, Effects, MoveToolpathToSetupArgs, RemoveAlignmentPinArgs, RemoveToolArgs,
    RemoveToolpathArgs, SaveProjectArgs, SessionError, SetBoundaryConfigArgs, SetDressupConfigArgs,
    SetDressupFieldArgs, SetMachineArgs, SetMachineKinematicsArgs, SetPostConfigArgs,
    SetRestAnalysisConfigArgs, SetSetupFaceArgs, SetSetupRotationArgs, SetSimulationResolutionArgs,
    SetStockConfigArgs, SetStockSourceArgs, SetToolParamArgs, SetToolpathEnabledArgs,
    SetToolpathHeightsArgs, SetToolpathModelArgs, SetToolpathParamArgs, SetToolpathToolArgs,
};

use rs_cam_mcp::server::{
    BuiltTool, build_tool_config, coerce_json_container_string, json_str, parse_operation_type,
    resolve_material, text,
};

use crate::app::RsCamApp;
use crate::mcp_bridge::{CoreRequest, McpOutcome, mutation_error_json};
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui_command::UiCommand;

use super::RestAnalysisDials;

/// What one command carries from before the mutation into its reply.
///
/// Every field is read BEFORE the mutation runs, because the mutation
/// moves what the reply describes: a removal re-keys every index above
/// it, a rename replaces the name a toast quotes, and a stock write
/// replaces the flag the reply reports as `auto_from_model_was`.
#[derive(Default)]
pub(crate) struct CoreBefore {
    /// The project diagnostics before the mutation. `mcp_mutation_result`
    /// subtracts these from the diagnostics after it.
    diagnostics: Vec<serde_json::Value>,
    /// The parameter name a refusal names, or `None` when the row's
    /// refusal names no field.
    field: Option<String>,
    /// The index the request named. A row whose request carries no index
    /// leaves this `None` and never reads it.
    index: Option<usize>,
    /// The id of the toolpath at [`Self::index`], read before the
    /// mutation. A removal re-keys the index, so it cannot be read after.
    toolpath_id: Option<crate::state::toolpath::ToolpathId>,
    /// A name a toast or a summary quotes.
    display_name: Option<String>,
    /// A count read before the mutation.
    count: Option<usize>,
    /// A number the reply reports as a "from" value.
    number: Option<f64>,
    /// The row-specific evidence the reply reports: the request's own
    /// echoed fields, and anything the mutation overwrites.
    extra: serde_json::Value,
}

impl CoreBefore {
    fn new(diagnostics: Vec<serde_json::Value>) -> Self {
        Self {
            diagnostics,
            ..Self::default()
        }
    }

    /// The index the request named.
    ///
    /// Only an arm whose own row carries an index calls this; a row that
    /// names none never reads the value.
    fn index(&self) -> usize {
        self.index.unwrap_or_default()
    }

    /// The name a toast or a summary quotes, or the empty string when
    /// the row quotes none.
    fn display_name(&self) -> String {
        self.display_name.clone().unwrap_or_default()
    }

    /// One field of [`Self::extra`], or `Null` when the row wrote none.
    fn extra(&self, key: &str) -> serde_json::Value {
        self.extra
            .get(key)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    /// One field of [`Self::extra`] as a number, or `0.0` when the row
    /// wrote none. Only an arm whose own row wrote the field reads it.
    fn extra_f64(&self, key: &str) -> f64 {
        self.extra
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or_default()
    }

    /// One field of [`Self::extra`] as text, or the empty string.
    fn extra_str(&self, key: &str) -> String {
        self.extra
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }
}

/// What the conversion produced.
pub(crate) enum CorePlan {
    /// Apply this command, then describe it with this evidence.
    Apply(Command, Box<CoreBefore>),
    /// Apply a preparation first, then the row's own command.
    ///
    /// The FIRST command is the preparation. The dispatch applies it,
    /// mirrors its [`rs_cam_core::session::Effects`] into viz state and
    /// describes none of it. The SECOND command is the row's own, and
    /// it is the one [`RsCamApp::describe_core`] answers for.
    ///
    /// One row builds this: `save_project`. The operator's post block
    /// lives in viz state, and it reaches the session BEFORE the file is
    /// written. The conversion step reads, it never writes (WP17), so
    /// the write is a command the dispatch applies.
    ApplyPair(Command, Command, Box<CoreBefore>),
    /// The surface answered without a mutation: a refusal, or a value the
    /// session already carries.
    Answered(String),
}

/// The describe step's answer.
pub(crate) struct CoreReply {
    /// The wire reply.
    pub reply: String,
    /// How the toast classifies the outcome, when the reply cannot say.
    /// A JSON reply classifies itself; a plain-text reply cannot, so its
    /// row states the answer.
    pub outcome: Option<McpOutcome>,
}

impl CoreReply {
    /// A reply that leaves the outcome to the reply document.
    fn quiet(reply: String) -> Self {
        Self {
            reply,
            outcome: None,
        }
    }
}

impl RsCamApp {
    /// Put the operator's screen on the object the command is about.
    ///
    /// Runs before the mutation, as every per-row arm did. A refused
    /// request therefore leaves the same view it left before WP4.
    pub(crate) fn mcp_before_core(&mut self, request: &CoreRequest) {
        // SHL-02: the workspace an MCP mutation moves the view to is a
        // column of the `declare_core_requests!` table, not an arm here.
        // The enum is generated from that table, so a new command cannot
        // reach this function without answering the question.
        if let Some(workspace) = request.workspace_after() {
            self.controller
                .events_mut()
                .push(AppEvent::Ui(UiCommand::SwitchWorkspace(workspace)));
        }

        // What is left is the five rows that ALSO highlight a field or
        // select a row. Each reads the session, so each is a body.
        match request {
            CoreRequest::SetStockConfig(_) => {
                self.mcp_highlight("stock_dimensions".to_owned());
            }
            CoreRequest::SetToolpathParam(p) => {
                let tp_id = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(p.index)
                    .map(|tc| tc.id);
                if let Some(tp_id) = tp_id {
                    self.mcp_highlight(format!("toolpath_{tp_id}_{}", p.param));
                    self.controller.state_mut().selection = Selection::Toolpath(tp_id);
                }
            }
            CoreRequest::SetToolParam(p) => {
                // The tool panel is not a workspace of its own, so this
                // row's table column is `None` and only the selection
                // moves.
                let tools = self.controller.state().session.list_tools();
                let tool_id = tools.get(p.index).map(|t| t.id.0);
                if let Some(tool_id) = tool_id {
                    self.mcp_highlight(format!("tool_{tool_id}_{}", p.param));
                    self.controller.state_mut().selection = Selection::Tool(ToolId(tool_id));
                }
            }
            CoreRequest::SetToolpathTool(p) => self.select_toolpath_for_mcp(p.index),
            CoreRequest::SetToolpathModel(p) => self.select_toolpath_for_mcp(p.index),
            _ => {}
        }
    }

    /// Mark one GUI field as just-changed by MCP, so the panel can flash it.
    fn mcp_highlight(&mut self, key: String) {
        self.controller
            .state_mut()
            .gui
            .mcp_highlights
            .insert(key, std::time::Instant::now());
    }
}

impl RsCamApp {
    /// Apply the preparation of a [`CorePlan::ApplyPair`], and reduce
    /// the plan to the row's own command. Every other plan passes
    /// through unchanged.
    ///
    /// The preparation is a mutation the row needs BEFORE its own
    /// command runs, so the conversion step can stay a reader (WP17).
    /// Its effects reach viz state through the controller's own door,
    /// which is the door the GUI save takes, so the two save routes
    /// adopt one answer. A refused preparation reaches the log alone:
    /// the reply belongs to the row's own command, and that command
    /// still runs.
    pub(crate) fn run_core_preparation(&mut self, plan: CorePlan) -> CorePlan {
        match plan {
            CorePlan::ApplyPair(preparation, command, before) => {
                let applied = self.controller.state_mut().session.apply(preparation);
                match applied {
                    Ok(effects) => {
                        self.controller.adopt_post_effects(&effects);
                    }
                    Err(error) => {
                        tracing::warn!("the command preparation was refused: {error}");
                    }
                }
                CorePlan::Apply(command, before)
            }
            plan => plan,
        }
    }

    /// Turn one wire request into the command it runs.
    ///
    /// The conversion parses the wire's own vocabulary, validates what
    /// the wire alone can validate, and reads the evidence the reply
    /// needs from before the mutation. A request it cannot turn into a
    /// command answers with its own refusal, in the words the operator
    /// reads today.
    pub(crate) fn core_command_for(&mut self, request: CoreRequest) -> CorePlan {
        let mut before = CoreBefore::new(self.mcp_diagnostic_snapshot());
        match request {
            CoreRequest::AddAlignmentPin(p) => {
                before.count = Some(
                    self.controller
                        .state()
                        .session
                        .stock_config()
                        .alignment_pins
                        .len(),
                );
                before.extra = serde_json::json!({
                    "x": p.x,
                    "y": p.y,
                    "diameter": p.diameter,
                });
                CorePlan::Apply(
                    Command::AddAlignmentPin(AddAlignmentPinArgs {
                        x: p.x,
                        y: p.y,
                        diameter: p.diameter,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::RemoveAlignmentPin(p) => {
                before.index = Some(p.index);
                CorePlan::Apply(
                    Command::RemoveAlignmentPin(RemoveAlignmentPinArgs { index: p.index }),
                    Box::new(before),
                )
            }
            CoreRequest::ImportModel(p) => {
                let file_path = Path::new(&p.path);
                let ext = file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                // C10: one extension table, in `rs_cam_core::io`. This arm
                // used to carry its own copy.
                let Some(kind) = rs_cam_core::io::infer_kind_from_path(file_path) else {
                    return CorePlan::Answered(json_str(serde_json::json!({
                        "error": format!("Unsupported file format '.{ext}'. Use .stl, .dxf, .svg, .step, or .stp")
                    })));
                };
                let imported = crate::io::import::import_model(
                    file_path,
                    0,
                    kind,
                    rs_cam_core::compute::stock_config::ModelUnits::Millimeters,
                );
                let model = match imported {
                    Ok(model) => model,
                    Err(e) => {
                        return CorePlan::Answered(json_str(
                            serde_json::json!({"error": format!("{e}")}),
                        ));
                    }
                };
                // The extension is the reply's `kind` when the import
                // reports none, as it always was.
                before.extra = serde_json::json!({ "ext": ext });
                CorePlan::Apply(
                    Command::AddModel(AddModelArgs {
                        model: Box::new(model),
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::AddSetup(p) => CorePlan::Apply(
                Command::AddSetup(AddSetupArgs {
                    name: p.name,
                    face_up: FaceUp::default(),
                }),
                Box::new(before),
            ),
            CoreRequest::SetSetupFace(p) => {
                let setups = self.controller.state().session.list_setups();
                if setups.get(p.setup_index).is_none() {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Error: Setup index {} not found", p.setup_index),
                        None,
                    ));
                }
                let face = match p.face_up.to_lowercase().as_str() {
                    "top" => FaceUp::Top,
                    "bottom" => FaceUp::Bottom,
                    "front" => FaceUp::Front,
                    "back" => FaceUp::Back,
                    "left" => FaceUp::Left,
                    "right" => FaceUp::Right,
                    _ => {
                        return CorePlan::Answered(mutation_error_json(
                            &format!(
                                "Error: Unknown face '{}'. Use: top, bottom, front, back, left, right",
                                p.face_up
                            ),
                            Some("face_up"),
                        ));
                    }
                };
                before.index = Some(p.setup_index);
                before.extra = serde_json::json!({ "face_up": p.face_up.to_lowercase() });
                CorePlan::Apply(
                    Command::SetSetupFace(SetSetupFaceArgs {
                        setup_index: p.setup_index,
                        face_up: face,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetSetupRotation(p) => {
                let setups = self.controller.state().session.list_setups();
                if setups.get(p.setup_index).is_none() {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Error: Setup index {} not found", p.setup_index),
                        None,
                    ));
                }
                // CLI-09: the wire type is the four-variant enum, so the
                // token is already legal by the time it arrives. The hand
                // parse that used to live here is gone.
                let rotation = ZRotation::from(p.z_rotation);
                before.index = Some(p.setup_index);
                before.extra = serde_json::json!({ "z_rotation": rotation.label() });
                CorePlan::Apply(
                    Command::SetSetupRotation(SetSetupRotationArgs {
                        setup_index: p.setup_index,
                        z_rotation: rotation,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::MoveToolpathToSetup(p) => {
                let session = &self.controller.state().session;
                if session.toolpath_configs().get(p.toolpath_index).is_none() {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Error: Toolpath index {} not found", p.toolpath_index),
                        None,
                    ));
                }
                if session.list_setups().get(p.target_setup_index).is_none() {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Error: Setup index {} not found", p.target_setup_index),
                        None,
                    ));
                }
                before.index = Some(p.toolpath_index);
                before.extra = serde_json::json!({
                    "toolpath_index": p.toolpath_index,
                    "target_setup_index": p.target_setup_index,
                });
                CorePlan::Apply(
                    // No position argument on this tool, so it appends —
                    // the behaviour it has always had.
                    Command::MoveToolpathToSetup(MoveToolpathToSetupArgs {
                        toolpath_index: p.toolpath_index,
                        target_setup_index: p.target_setup_index,
                        target_position: None,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SaveProject(p) => {
                // The viz post block is the operator's, and it reaches
                // the session before the file is written, as the
                // controller's own save door does.
                //
                // WP17: this arm called `set_post_config` here. That
                // broke the contract at the top of this file — the
                // conversion READS — and the setter dropped the session
                // simulation on every save, which made
                // `ProjectSession::start` refuse every
                // `FromRemainingStock` operation. The write is a command
                // of its own now, and it runs only when the two blocks
                // differ.
                let session_post = self.controller.state().gui.post.clone();
                before.display_name = Some(p.path.clone());
                let save = Command::SaveProject(SaveProjectArgs {
                    path: PathBuf::from(p.path),
                });
                if *self.controller.state().session.post_config() == session_post {
                    return CorePlan::Apply(save, Box::new(before));
                }
                CorePlan::ApplyPair(
                    Command::SetPostConfig(SetPostConfigArgs {
                        post: Box::new(session_post),
                    }),
                    save,
                    Box::new(before),
                )
            }
            CoreRequest::SetToolpathParam(p) => {
                // A client that stringifies an array argument would
                // otherwise be met with `invalid type: string
                // "[[2.5,2.5],...]", expected a sequence` (measured
                // 2026-08-19 on a drill `holes` list). The declared schema
                // is the primary fix; this is the fallback.
                let value = coerce_json_container_string(p.value);
                before.index = Some(p.index);
                before.field = Some(p.param.clone());
                before.display_name = Some(p.param.clone());
                CorePlan::Apply(
                    Command::SetToolpathParam(SetToolpathParamArgs {
                        index: p.index,
                        param: p.param,
                        value,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetToolParam(p) => {
                let value = coerce_json_container_string(p.value);
                before.index = Some(p.index);
                before.field = Some(p.param.clone());
                before.display_name = Some(p.param.clone());
                CorePlan::Apply(
                    Command::SetToolParam(SetToolParamArgs {
                        index: p.index,
                        param: p.param,
                        value,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetToolpathTool(p) => {
                before.index = Some(p.index);
                before.count = Some(p.tool_id);
                CorePlan::Apply(
                    Command::SetToolpathTool(SetToolpathToolArgs {
                        index: p.index,
                        tool_id: p.tool_id,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetToolpathModel(p) => {
                before.index = Some(p.index);
                before.count = Some(p.model_id);
                CorePlan::Apply(
                    Command::SetToolpathModel(SetToolpathModelArgs {
                        index: p.index,
                        model_id: p.model_id,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetToolpathHeights(p) => {
                let Some(mut heights) = self
                    .controller
                    .state()
                    .session
                    .get_toolpath_config(p.index)
                    .map(|tc| tc.heights.clone())
                else {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Error: toolpath index {} not found", p.index),
                        None,
                    ));
                };
                if let Some(v) = p.clearance_z {
                    heights.clearance_z = HeightMode::Manual(v);
                }
                if let Some(v) = p.retract_z {
                    heights.retract_z = HeightMode::Manual(v);
                }
                if let Some(v) = p.feed_z {
                    heights.feed_z = HeightMode::Manual(v);
                }
                if let Some(v) = p.top_z {
                    heights.top_z = HeightMode::Manual(v);
                }
                if let Some(v) = p.bottom_z {
                    heights.bottom_z = HeightMode::Manual(v);
                }
                before.index = Some(p.index);
                before.extra = serde_json::json!({
                    "index": p.index,
                    "clearance_z": p.clearance_z,
                    "retract_z": p.retract_z,
                    "feed_z": p.feed_z,
                    "top_z": p.top_z,
                    "bottom_z": p.bottom_z,
                });
                CorePlan::Apply(
                    Command::SetToolpathHeights(SetToolpathHeightsArgs {
                        index: p.index,
                        heights,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::AddToolpath(p) => self.core_add_toolpath(p, before),
            CoreRequest::RemoveToolpath(p) => {
                // The id must be read before the removal: it re-keys every
                // index above the removed one.
                before.toolpath_id = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(p.index)
                    .map(|tc| tc.id);
                before.index = Some(p.index);
                CorePlan::Apply(
                    Command::RemoveToolpath(RemoveToolpathArgs { index: p.index }),
                    Box::new(before),
                )
            }
            CoreRequest::AddTool(p) => self.core_add_tool(&p, before),
            CoreRequest::AddToolFromLibrary(p) => self.core_add_tool_from_library(&p, before),
            CoreRequest::RemoveTool(p) => {
                before.index = Some(p.index);
                CorePlan::Apply(
                    Command::RemoveTool(RemoveToolArgs { index: p.index }),
                    Box::new(before),
                )
            }
            CoreRequest::SetStockConfig(p) => self.core_set_stock_config(&p, before),
            CoreRequest::SetStockSource(p) => {
                // W5 item (b): the schema owns the token list, so an unknown
                // token never reaches this arm. The hand parser and its
                // runtime refusal are gone.
                let parsed = rs_cam_core::compute::config::StockSource::from(p.source);
                before.index = Some(p.index);
                // The CORE enum serialises here, so one owner writes the
                // token the wire echoes.
                before.extra = serde_json::json!({ "stock_source": parsed });
                CorePlan::Apply(
                    Command::SetStockSource(SetStockSourceArgs {
                        index: p.index,
                        source: parsed,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetMachineKinematics(p) => self.core_set_machine_kinematics(&p, before),
            CoreRequest::ImportMachineSettings(p) => self.core_import_machine_settings(&p, before),
            CoreRequest::LoadMachineFromLibrary(p) => {
                let profile = match rs_cam_core::io::machine_library::load(&p.name) {
                    Ok(profile) => profile,
                    Err(e) => {
                        return CorePlan::Answered(json_str(serde_json::json!({
                            "ok": false,
                            "error": format!(
                                "could not load machine '{}' from the library: {e}",
                                p.name
                            ),
                        })));
                    }
                };
                before.display_name = Some(profile.name.clone());
                before.extra = serde_json::json!({ "imported": p.name });
                CorePlan::Apply(
                    Command::SetMachine(SetMachineArgs {
                        machine: Box::new(profile),
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetSpindleStrategy(p) => {
                let parsed = match p.strategy.as_str() {
                    "match_chart" | "MatchChart" | "matchchart" => {
                        rs_cam_core::feeds::SpindleStrategy::MatchChart
                    }
                    "max_speed" | "MaxSpeed" | "maxspeed" => {
                        rs_cam_core::feeds::SpindleStrategy::MaxSpeed
                    }
                    other => {
                        return CorePlan::Answered(mutation_error_json(
                            &format!(
                                "Error: unknown spindle_strategy '{other}'. Expected 'match_chart' or 'max_speed'."
                            ),
                            Some("spindle_strategy"),
                        ));
                    }
                };
                if self
                    .controller
                    .state()
                    .session
                    .post_config()
                    .spindle_strategy
                    == parsed
                {
                    let strategy = &p.strategy;
                    return CorePlan::Answered(self.mcp_mutation_result(
                        format!("Spindle policy already set to '{strategy}'; no change."),
                        serde_json::json!({ "spindle_strategy": strategy, "changed": false }),
                        Vec::new(),
                        &before.diagnostics,
                    ));
                }
                let mut post = self.controller.state().session.post_config().clone();
                post.spindle_strategy = parsed;
                before.display_name = Some(p.strategy.clone());
                before.extra = serde_json::json!({ "spindle_strategy": p.strategy });
                CorePlan::Apply(
                    Command::SetPostConfig(SetPostConfigArgs {
                        post: Box::new(post),
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetBoundaryConfig(p) => self.core_set_boundary_config(&p, before),
            CoreRequest::SetRestAnalysisConfig(p) => self.core_set_rest_analysis_config(&p, before),
            CoreRequest::SetDressupConfig(p) => {
                let dressups: DressupConfig = match serde_json::from_value(p.dressup) {
                    Ok(dc) => dc,
                    Err(e) => {
                        return CorePlan::Answered(mutation_error_json(
                            &format!("Error: Invalid dressup config: {e}"),
                            None,
                        ));
                    }
                };
                before.index = Some(p.index);
                CorePlan::Apply(
                    Command::SetDressupConfig(SetDressupConfigArgs {
                        index: p.index,
                        dressups: Box::new(dressups),
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetDressupField(p) => {
                before.index = Some(p.index);
                before.field = Some(p.key.clone());
                before.display_name = Some(p.key.clone());
                CorePlan::Apply(
                    Command::SetDressupField(SetDressupFieldArgs {
                        index: p.index,
                        key: p.key,
                        value: p.value,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetToolpathEnabled(p) => {
                before.index = Some(p.index);
                before.extra = serde_json::json!({ "enabled": p.enabled });
                CorePlan::Apply(
                    Command::SetToolpathEnabled(SetToolpathEnabledArgs {
                        index: p.index,
                        enabled: p.enabled,
                    }),
                    Box::new(before),
                )
            }
            CoreRequest::SetSimulationResolution(p) => CorePlan::Apply(
                Command::SetSimulationResolution(SetSimulationResolutionArgs {
                    resolution: p.resolution_mm.map_or(
                        rs_cam_core::session::SimulationResolution::Auto,
                        rs_cam_core::session::SimulationResolution::Fixed,
                    ),
                }),
                Box::new(before),
            ),
        }
    }
}

impl RsCamApp {
    /// Build the `add_toolpath` command.
    ///
    /// The operation's defaults come from the one canonical Suggest call
    /// (Roadmap F.5), the depths from the stock (Roadmap B.1-B.3) and the
    /// boundary from the model (Roadmap B.7), exactly as before WP4.
    fn core_add_toolpath(
        &mut self,
        p: rs_cam_mcp::server::AddToolpathParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let op_type = match parse_operation_type(&p.operation_type) {
            Ok(ot) => ot,
            Err(e) => return CorePlan::Answered(mutation_error_json(&format!("Error: {e}"), None)),
        };

        let session = &self.controller.state().session;
        let tools = session.list_tools();
        let tool_raw_id = match tools.get(p.tool_index) {
            Some(info) => info.id.0,
            None => {
                return CorePlan::Answered(mutation_error_json(
                    &format!("Error: Tool index {} not found", p.tool_index),
                    None,
                ));
            }
        };

        // Roadmap B.1–B.3 — pull stock-aware depth defaults so a fresh
        // toolpath added via MCP gets the same sensible per-op depths
        // as a GUI add (drop_cutter min_z = stock_bottom, etc).
        let stock_bbox = session.stock_bbox();
        let stock_padding = session.stock_config().padding;
        let stock_ctx =
            rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(stock_bbox, stock_padding);
        let label = op_type.label();
        let tp_name = p.name.unwrap_or_else(|| label.to_owned());

        // Roadmap F.5 — one-shot canonical suggest call at toolpath creation.
        // New toolpaths get recommended feeds written in once; after that,
        // fields are always user-owned.
        let Some(tool) = session.tools().iter().find(|t| t.id.0 == tool_raw_id) else {
            return CorePlan::Answered(mutation_error_json(
                &format!("Error: Tool index {} not found", p.tool_index),
                None,
            ));
        };
        // R2: the creation-time boundary copies this diameter as its offset.
        let tool_diameter_mm = tool.diameter;
        // Q1: `add_toolpath` takes `model_id` as a required parameter, so
        // the model IS known here. The bbox gates the runtime-sanity
        // stepover back-off in Suggest.
        let model_bbox = session.model_bbox(p.model_id);
        let (op_config, feeds_provenance, feeds_refusal) =
            match rs_cam_core::feeds::suggest::suggest_params(
                rs_cam_core::feeds::suggest::SuggestParamsInput {
                    op_type,
                    tool,
                    machine: session.machine(),
                    material: &session.stock_config().material,
                    lut: rs_cam_core::feeds::embedded_vendor_lut(),
                    stock_ctx: &stock_ctx,
                    spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
                    // Q1: the stock reaches Suggest through `stock_ctx`
                    // above, so `SuggestContext::stock` stays empty rather
                    // than carrying the same value twice.
                    // `upstream_leftover_stock_mm` stays `None`: no lookup
                    // here gives it, and v1 does not read it.
                    context: rs_cam_core::feeds::suggest::SuggestContext {
                        model_bbox: model_bbox.as_ref(),
                        ..rs_cam_core::feeds::suggest::SuggestContext::default()
                    },
                },
            ) {
                Ok(s) => (s.operation, s.provenance, None),
                Err(e @ rs_cam_core::feeds::FeedsError::Unbacked { .. }) => {
                    // Ruling R1 (2026-09-23): no checked basis for a recipe on
                    // this cell. The operation is added with the registry and
                    // stock defaults and no recipe; the reply carries the
                    // refusal text, as the GUI toast does.
                    (
                        rs_cam_core::feeds::suggest::default_operation(op_type, &stock_ctx),
                        rs_cam_core::feeds::FeedsProvenance::default(),
                        Some(e.to_string()),
                    )
                }
                Err(e) => {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Cannot add toolpath: {e}"),
                        None,
                    ));
                }
            };

        // Roadmap B.7 — boundary auto-enable for 3D ops on mesh models.
        // R2: silhouette plus one tool diameter, the same rule the GUI
        // controller applies (`BoundaryConfig::for_3d_op`).
        let has_mesh = session.models().iter().any(|m| m.mesh.is_some());
        let boundary = if op_config.is_3d() && has_mesh {
            BoundaryConfig::for_3d_op(tool_diameter_mm)
        } else {
            BoundaryConfig::default()
        };

        let config = rs_cam_core::session::ToolpathConfig {
            id: rs_cam_core::ToolpathId(0),
            name: tp_name,
            enabled: true,
            operation: op_config,
            dressups: DressupConfig::for_op(op_type),
            heights: crate::state::toolpath::HeightsConfig::default(),
            tool_id: tool_raw_id,
            model_id: p.model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary,
            boundary_inherit: true,
            rest_analysis: crate::state::toolpath::RestAnalysisConfig::default(),
            stock_source: rs_cam_core::compute::config::StockSource::default(),
            coolant: rs_cam_core::gcode::CoolantMode::default(),
            face_selection: None,
            debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance,
            planner_origin: None,
        };

        before.extra = serde_json::json!({
            "operation": label,
            "feeds_refusal": feeds_refusal,
        });
        CorePlan::Apply(
            Command::AddToolpath(AddToolpathArgs {
                setup_index: p.setup_index,
                config: Box::new(config),
            }),
            Box::new(before),
        )
    }

    /// Build the `add_tool` command.
    ///
    /// [`build_tool_config`] refuses when the geometry that defines the
    /// tool type is missing, and names every field it defaulted. The tool
    /// NUMBER is allocated against the numbers the project already holds:
    /// an M6 tool change only re-triggers on a CHANGE of number, so two
    /// tools sharing one collapse into a single change on export.
    fn core_add_tool(
        &mut self,
        spec: &rs_cam_mcp::server::AddToolParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let BuiltTool {
            mut config,
            defaulted,
        } = match build_tool_config(spec) {
            Ok(built) => built,
            Err(e) => return CorePlan::Answered(mutation_error_json(&format!("Error: {e}"), None)),
        };

        let taken: Vec<u32> = self
            .controller
            .state()
            .session
            .tools()
            .iter()
            .map(|t| t.tool_number)
            .collect();
        let (tool_number, conflict) = match spec.tool_number {
            Some(n) => (n, taken.contains(&n)),
            None => (
                taken.iter().copied().max().unwrap_or(0).saturating_add(1),
                false,
            ),
        };
        config.tool_number = tool_number;

        before.display_name = Some(spec.name.clone());
        before.extra = serde_json::json!({
            "tool_type": spec.tool_type.clone(),
            "diameter": spec.diameter,
            "tool_number": tool_number,
            "tool_number_conflict": conflict,
            "defaulted": defaulted,
        });
        CorePlan::Apply(
            Command::AddTool(AddToolArgs {
                tool: Box::new(config),
            }),
            Box::new(before),
        )
    }

    /// Build the `add_tool_from_library` command.
    ///
    /// The catalog READ belongs to this surface; by the time the command
    /// reaches core it carries a finished tool, like `add_tool`.
    fn core_add_tool_from_library(
        &mut self,
        p: &rs_cam_mcp::server::AddToolFromLibraryParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        use rs_cam_core::io::tool_library;
        let cat = match tool_library::load_library(&p.catalog) {
            Ok(c) => c,
            Err(e) => return CorePlan::Answered(mutation_error_json(&format!("Error: {e}"), None)),
        };
        let Some(mut tool) = cat.tools.get(p.index).cloned() else {
            return CorePlan::Answered(mutation_error_json(
                &format!(
                    "Error: catalog '{}' has no tool at index {} (has {} tools)",
                    p.catalog,
                    p.index,
                    cat.tools.len()
                ),
                None,
            ));
        };
        let name = tool.name.clone();
        tool.id = ToolId(0); // session reassigns on insert

        // Catalogs number their tools independently, so importing from
        // two of them (or twice from one) lands duplicate `tool_number`s
        // in the project — and an M6 tool change only re-triggers on a
        // CHANGE of number, so duplicates silently collapse the changes
        // on export. Reallocate on collision and say so.
        let taken: Vec<u32> = self
            .controller
            .state()
            .session
            .tools()
            .iter()
            .map(|t| t.tool_number)
            .collect();
        let catalog_tool_number = tool.tool_number;
        let renumbered = catalog_tool_number == 0 || taken.contains(&catalog_tool_number);
        if renumbered {
            tool.tool_number = taken.iter().copied().max().unwrap_or(0).saturating_add(1);
        }
        let tool_number = tool.tool_number;
        let renumbered_from = if renumbered {
            serde_json::json!(catalog_tool_number)
        } else {
            serde_json::Value::Null
        };

        before.display_name = Some(name.clone());
        before.index = Some(p.index);
        before.extra = serde_json::json!({
            "name": name,
            "source_catalog": p.catalog.clone(),
            "source_index": p.index,
            "tool_number": tool_number,
            "tool_number_renumbered_from": renumbered_from,
            "renumbered": renumbered,
            "catalog_tool_number": catalog_tool_number,
        });
        CorePlan::Apply(
            Command::AddToolFromLibrary(AddToolArgs {
                tool: Box::new(tool),
            }),
            Box::new(before),
        )
    }
}

impl RsCamApp {
    /// Build the `set_stock_config` command from an all-optional patch.
    ///
    /// **Gap 2 (2026-08-19 run log): setting a dimension explicitly
    /// clears `auto_from_model`.** An explicit dimension is an assertion
    /// about the physical stock on the bed, and auto-fit is a *sizing*
    /// convenience that has no business overwriting one. The reply
    /// reports the flag either way.
    ///
    /// Everything that can fail resolves BEFORE anything is written: a
    /// half-applied stock config is worse than a refusal.
    fn core_set_stock_config(
        &mut self,
        spec: &rs_cam_mcp::server::SetStockConfigParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let material = match spec.material.as_deref() {
            Some(name) => match resolve_material(name) {
                Ok(m) => Some(m),
                Err(e) => {
                    return CorePlan::Answered(mutation_error_json(
                        &format!("Error: {e}"),
                        Some("material"),
                    ));
                }
            },
            None => None,
        };
        for (label, value) in [("x", spec.x), ("y", spec.y), ("z", spec.z)] {
            if let Some(v) = value
                && (!v.is_finite() || v <= 0.0)
            {
                return CorePlan::Answered(mutation_error_json(
                    &format!("Error: stock {label} must be a positive number of mm (got {v})."),
                    Some(label),
                ));
            }
        }
        for (label, value) in [
            ("origin_x", spec.origin_x),
            ("origin_y", spec.origin_y),
            ("origin_z", spec.origin_z),
        ] {
            if let Some(v) = value
                && !v.is_finite()
            {
                return CorePlan::Answered(mutation_error_json(
                    &format!("Error: stock {label} must be a finite number of mm (got {v})."),
                    Some(label),
                ));
            }
        }

        let mut stock = self.controller.state().session.stock_config().clone();
        let auto_before = stock.auto_from_model;

        let mut geometry_set: Vec<&'static str> = Vec::new();
        if let Some(v) = spec.x {
            stock.x = v;
            geometry_set.push("x");
        }
        if let Some(v) = spec.y {
            stock.y = v;
            geometry_set.push("y");
        }
        if let Some(v) = spec.z {
            stock.z = v;
            geometry_set.push("z");
        }
        if let Some(v) = spec.origin_x {
            stock.origin_x = v;
            geometry_set.push("origin_x");
        }
        if let Some(v) = spec.origin_y {
            stock.origin_y = v;
            geometry_set.push("origin_y");
        }
        if let Some(v) = spec.origin_z {
            stock.origin_z = v;
            geometry_set.push("origin_z");
        }
        if let Some(m) = material {
            stock.material = m;
        }

        let auto_after = match spec.auto_from_model {
            Some(explicit) => explicit,
            None if !geometry_set.is_empty() => false,
            None => auto_before,
        };
        stock.auto_from_model = auto_after;

        before.extra = serde_json::json!({
            "auto_from_model": auto_after,
            "auto_from_model_was": auto_before,
            "fields_set": geometry_set,
        });
        CorePlan::Apply(
            Command::SetStockConfig(SetStockConfigArgs {
                stock: Box::new(stock),
            }),
            Box::new(before),
        )
    }

    /// Build the `set_machine_kinematics` command.
    ///
    /// Sibling of [`Self::core_import_machine_settings`]: same target,
    /// same library-link break, typed arguments instead of a `$$` paste.
    /// The merge of a partial per-axis triple onto the machine's current
    /// limits belongs here, on the surface that reports the refusal.
    fn core_set_machine_kinematics(
        &mut self,
        spec: &rs_cam_mcp::server::SetMachineKinematicsParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let mut kin = self
            .controller
            .state()
            .session
            .machine()
            .effective_kinematics();
        let had_per_axis = kin.acceleration_xyz_mm_s2.is_some();
        let had_per_axis_rate = kin.max_rate_xyz_mm_min.is_some();

        for (label, value) in [
            ("acceleration_x_mm_s2", spec.acceleration_x_mm_s2),
            ("acceleration_y_mm_s2", spec.acceleration_y_mm_s2),
            ("acceleration_z_mm_s2", spec.acceleration_z_mm_s2),
            ("acceleration_mm_s2", spec.acceleration_mm_s2),
            ("max_rate_x_mm_min", spec.max_rate_x_mm_min),
            ("max_rate_y_mm_min", spec.max_rate_y_mm_min),
            ("max_rate_z_mm_min", spec.max_rate_z_mm_min),
            ("junction_deviation_mm", spec.junction_deviation_mm),
            (
                "max_junction_velocity_mm_min",
                spec.max_junction_velocity_mm_min,
            ),
            ("jerk_mm_s3", spec.jerk_mm_s3),
        ] {
            if let Some(v) = value
                && (!v.is_finite() || v <= 0.0)
            {
                return CorePlan::Answered(json_str(serde_json::json!({
                    "ok": false,
                    "error": format!("{label} must be a positive, finite number (got {v})."),
                })));
            }
        }

        let axes = (
            spec.acceleration_x_mm_s2,
            spec.acceleration_y_mm_s2,
            spec.acceleration_z_mm_s2,
        );
        match axes {
            (Some(ax), Some(ay), Some(az)) => {
                kin.acceleration_xyz_mm_s2 = Some([ax, ay, az]);
                if spec.acceleration_mm_s2.is_none() {
                    kin.acceleration_mm_s2 = (ax + ay + az) / 3.0;
                }
            }
            (None, None, None) => {}
            (ax, ay, az) => {
                let Some(existing) = kin.acceleration_xyz_mm_s2 else {
                    return CorePlan::Answered(json_str(serde_json::json!({
                        "ok": false,
                        "error": "A partial per-axis acceleration set was given, but this machine \
                                  has no per-axis limits yet to patch. Pass all three of \
                                  acceleration_x_mm_s2 / _y_ / _z_, or set the isotropic \
                                  acceleration_mm_s2 instead. Nothing was written.",
                    })));
                };
                let [mut ex, mut ey, mut ez] = existing;
                if let Some(v) = ax {
                    ex = v;
                }
                if let Some(v) = ay {
                    ey = v;
                }
                if let Some(v) = az {
                    ez = v;
                }
                kin.acceleration_xyz_mm_s2 = Some([ex, ey, ez]);
            }
        }

        // Per-axis max rates ($110/$111/$112), same triple / none /
        // partial-patch-or-refuse contract as the accelerations above.
        let rates = (
            spec.max_rate_x_mm_min,
            spec.max_rate_y_mm_min,
            spec.max_rate_z_mm_min,
        );
        match rates {
            (Some(rx), Some(ry), Some(rz)) => {
                kin.max_rate_xyz_mm_min = Some([rx, ry, rz]);
            }
            (None, None, None) => {}
            (rx, ry, rz) => {
                let Some(existing) = kin.max_rate_xyz_mm_min else {
                    return CorePlan::Answered(json_str(serde_json::json!({
                        "ok": false,
                        "error": "A partial per-axis max-rate set was given, but this machine has \
                                  no per-axis rates yet to patch. Pass all three of \
                                  max_rate_x_mm_min / _y_ / _z_. Nothing was written.",
                    })));
                };
                let [mut ex, mut ey, mut ez] = existing;
                if let Some(v) = rx {
                    ex = v;
                }
                if let Some(v) = ry {
                    ey = v;
                }
                if let Some(v) = rz {
                    ez = v;
                }
                kin.max_rate_xyz_mm_min = Some([ex, ey, ez]);
            }
        }

        if let Some(v) = spec.acceleration_mm_s2 {
            kin.acceleration_mm_s2 = v;
        }
        if let Some(v) = spec.junction_deviation_mm {
            kin.junction_deviation_mm = v;
        }
        if let Some(v) = spec.max_junction_velocity_mm_min {
            kin.max_junction_velocity_mm_min = Some(v);
        }
        if let Some(v) = spec.jerk_mm_s3 {
            kin.jerk_mm_s3 = Some(v);
        }

        let per_axis = match kin.acceleration_xyz_mm_s2 {
            Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
            None => serde_json::Value::Null,
        };
        let per_axis_rate = match kin.max_rate_xyz_mm_min {
            Some([rx, ry, rz]) => serde_json::json!([rx, ry, rz]),
            None => serde_json::Value::Null,
        };
        before.extra = serde_json::json!({
            "acceleration_xyz_mm_s2": per_axis,
            "acceleration_mm_s2": kin.acceleration_mm_s2,
            "max_rate_xyz_mm_min": per_axis_rate,
            "junction_deviation_mm": kin.junction_deviation_mm,
            "max_junction_velocity_mm_min": kin.max_junction_velocity_mm_min,
            "jerk_mm_s3": kin.jerk_mm_s3,
            "per_axis_was_set_before": had_per_axis,
            "per_axis_rate_was_set_before": had_per_axis_rate,
        });
        CorePlan::Apply(
            Command::SetMachineKinematics(SetMachineKinematicsArgs {
                kinematics: Box::new(kin),
            }),
            Box::new(before),
        )
    }

    /// Build the `import_machine_settings` command from a GRBL `$$` dump.
    ///
    /// The parse belongs to this surface, which also decides whether the
    /// dump was recognised at all.
    fn core_import_machine_settings(
        &mut self,
        p: &rs_cam_mcp::server::ImportMachineSettingsParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        use rs_cam_core::machine::kinematics::{MachineKinematics, default_junction_deviation_mm};
        let imp = MachineKinematics::from_grbl_settings(&p.dump);
        let recognized = imp.kinematics.acceleration_xyz_mm_s2.is_some()
            || imp.max_feed_mm_min.is_some()
            || imp.arc_tolerance_mm.is_some()
            || imp.max_spindle_rpm.is_some()
            || (imp.kinematics.junction_deviation_mm - default_junction_deviation_mm()).abs()
                > 1e-12;
        if !recognized {
            return CorePlan::Answered(json_str(serde_json::json!({
                "ok": false,
                "error": "No GRBL settings recognised in the dump (expected $N=value lines, \
                          e.g. $11=…, $120=…).",
            })));
        }

        let prev_max_feed = self.controller.state().session.machine().max_feed_mm_min;
        // The parser reports the per-axis rates on the import, not inside
        // `kinematics` — land them on the machine model here (P1).
        let mut kinematics = imp.kinematics;
        kinematics.max_rate_xyz_mm_min = imp.max_rate_xyz_mm_min;

        let per_axis = match imp.kinematics.acceleration_xyz_mm_s2 {
            Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
            None => serde_json::Value::Null,
        };
        let per_axis_rate = match imp.max_rate_xyz_mm_min {
            Some([rx, ry, rz]) => serde_json::json!([rx, ry, rz]),
            None => serde_json::Value::Null,
        };
        before.number = Some(prev_max_feed);
        before.extra = serde_json::json!({
            "acceleration_xyz_mm_s2": per_axis,
            "acceleration_mm_s2": imp.kinematics.acceleration_mm_s2,
            "max_rate_xyz_mm_min": per_axis_rate,
            "junction_deviation_mm": imp.kinematics.junction_deviation_mm,
            "arc_tolerance_mm": imp.arc_tolerance_mm,
            "max_spindle_rpm": imp.max_spindle_rpm,
            "ignored_settings": imp.ignored_count,
        });
        CorePlan::Apply(
            Command::ImportMachineSettings(rs_cam_core::session::ImportMachineSettingsArgs {
                kinematics: Box::new(kinematics),
                max_feed_mm_min: imp.max_feed_mm_min,
            }),
            Box::new(before),
        )
    }

    /// Build the `set_boundary_config` command.
    fn core_set_boundary_config(
        &mut self,
        p: &rs_cam_mcp::server::SetBoundaryConfigParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let boundary_source = match p.source.as_deref() {
            Some("stock") | None => BoundarySource::Stock,
            Some("model_silhouette") => BoundarySource::ModelSilhouette,
            Some("derived_rest_regions") => {
                let Some(raw_id) = p.source_toolpath_id else {
                    return CorePlan::Answered(mutation_error_json(
                        "Error: 'derived_rest_regions' requires source_toolpath_id — the id \
                         of the toolpath whose REST ANALYSIS supplies the boundary \
                         regions (see get_toolpath_params's 'id' field). Any \
                         operation produces them when its rest analysis is enabled \
                         and the project carries a mesh.",
                        Some("source_toolpath_id"),
                    ));
                };
                let source_id = rs_cam_core::ToolpathId(raw_id);
                if self
                    .controller
                    .state()
                    .session
                    .find_toolpath_config_by_id(source_id)
                    .is_none()
                {
                    return CorePlan::Answered(mutation_error_json(
                        &format!(
                            "Error: source_toolpath_id {raw_id} does not match any \
                             toolpath in this project."
                        ),
                        Some("source_toolpath_id"),
                    ));
                }
                BoundarySource::DerivedRestRegions {
                    source_toolpath_id: source_id,
                }
            }
            Some(other) => {
                return CorePlan::Answered(mutation_error_json(
                    &format!(
                        "Error: Unknown boundary source '{other}'. Use 'stock', \
                         'model_silhouette', or 'derived_rest_regions'."
                    ),
                    Some("source"),
                ));
            }
        };

        let boundary_containment = match p.containment.as_deref() {
            Some("center") | None => BoundaryContainment::Center,
            Some("inside") => BoundaryContainment::Inside,
            Some("outside") => BoundaryContainment::Outside,
            Some(other) => {
                return CorePlan::Answered(mutation_error_json(
                    &format!(
                        "Error: Unknown containment '{other}'. Use 'center', 'inside', or 'outside'."
                    ),
                    Some("containment"),
                ));
            }
        };

        let boundary = BoundaryConfig {
            enabled: p.enabled,
            source: boundary_source,
            containment: boundary_containment,
            offset: p.offset.unwrap_or(0.0),
        };

        before.index = Some(p.index);
        before.extra = serde_json::to_value(&boundary).unwrap_or(serde_json::Value::Null);
        CorePlan::Apply(
            Command::SetBoundaryConfig(SetBoundaryConfigArgs {
                index: p.index,
                boundary,
            }),
            Box::new(before),
        )
    }

    /// Build the `set_rest_analysis_config` command.
    fn core_set_rest_analysis_config(
        &mut self,
        p: &rs_cam_mcp::server::SetRestAnalysisConfigParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let resolved_reference_tool_id = match p.reference_tool_id {
            Some(raw_id) => {
                let tool_id = ToolId(raw_id);
                if !self
                    .controller
                    .state()
                    .session
                    .tools()
                    .iter()
                    .any(|t| t.id == tool_id)
                {
                    return CorePlan::Answered(mutation_error_json(
                        &format!(
                            "Error: reference_tool_id {raw_id} does not match any tool in \
                             this project."
                        ),
                        Some("reference_tool_id"),
                    ));
                }
                Some(tool_id)
            }
            None => None,
        };

        let dials = RestAnalysisDials {
            cell_mm: p.cell_mm,
            min_valley_depth: p.min_valley_depth,
            region_margin_mm: p.region_margin_mm,
            offset_stepover_mm: p.offset_stepover_mm,
            num_offset_passes: p.num_offset_passes,
        };
        let defaults = crate::state::toolpath::RestAnalysisConfig::default();
        let rest_analysis = rs_cam_core::compute::config::RestAnalysisConfig {
            enabled: p.enabled,
            reference_tool_id: resolved_reference_tool_id,
            cell_mm: dials.cell_mm.unwrap_or(defaults.cell_mm),
            min_valley_depth: dials.min_valley_depth.unwrap_or(defaults.min_valley_depth),
            region_margin_mm: dials.region_margin_mm.unwrap_or(defaults.region_margin_mm),
            // PR-7 (H2.5): pass the `Option`s STRAIGHT through. Unset is not
            // a missing value to be filled in with a default here — it is
            // the instruction "size this from the reach policy", and only
            // the generation path knows the cutter to size it against.
            offset_stepover_mm: dials.offset_stepover_mm,
            num_offset_passes: dials.num_offset_passes,
        };

        before.index = Some(p.index);
        before.extra = serde_json::to_value(&rest_analysis).unwrap_or(serde_json::Value::Null);
        CorePlan::Apply(
            Command::SetRestAnalysisConfig(SetRestAnalysisConfigArgs {
                index: p.index,
                rest_analysis,
            }),
            Box::new(before),
        )
    }
}

/// A JSON value as a toast quotes it: a string bare, anything else as
/// JSON.
fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl RsCamApp {
    /// The toast one request pushes, or `None` for a row that pushes
    /// none.
    ///
    /// **Read off the REQUEST, before the conversion.** Fourteen of the
    /// twenty-nine rows toast, and their text quotes what the operator
    /// asked for: the parameter and the value, the setup index and the
    /// spelling of the face, the file name, the name of the toolpath or
    /// the tool the request named. All of it is readable before the
    /// mutation, and it must be: a request the CONVERSION refuses —
    /// an unknown face, an unsupported file extension, a tool geometry
    /// `build_tool_config` declines, the Suggest door declining a
    /// Scallop on a flat end mill — never reaches the describe step, and
    /// a toast built there would leave that refusal silent on screen.
    /// That is the defect UX-R03-003 recorded, in the other direction.
    ///
    /// The dispatcher pushes this text once, through `push_mcp_outcome`,
    /// classified against the reply — so the operator reads the refusal
    /// at Warning, and reads the success text only once it is true.
    pub(crate) fn core_toast_for(&self, request: &CoreRequest) -> Option<String> {
        match request {
            CoreRequest::AddSetup(_) => Some("MCP: Adding setup".to_owned()),
            CoreRequest::SetSetupFace(p) => {
                let setup_index = p.setup_index;
                let face_up = &p.face_up;
                Some(format!("MCP: Set setup {setup_index} face to '{face_up}'"))
            }
            CoreRequest::SetSetupRotation(p) => {
                let setup_index = p.setup_index;
                let z_rotation = &p.z_rotation;
                Some(format!(
                    "MCP: Set setup {setup_index} Z rotation to '{z_rotation}'"
                ))
            }
            CoreRequest::MoveToolpathToSetup(p) => {
                let toolpath_index = p.toolpath_index;
                let target_setup_index = p.target_setup_index;
                Some(format!(
                    "MCP: Moving toolpath {toolpath_index} to setup {target_setup_index}"
                ))
            }
            CoreRequest::ImportModel(p) => {
                let name = Path::new(&p.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&p.path);
                Some(format!("MCP: Importing '{name}'"))
            }
            CoreRequest::SaveProject(_) => Some("MCP: Saved project".to_owned()),
            CoreRequest::SetToolpathParam(p) => {
                let param = &p.param;
                let value_str = display_value(&p.value);
                let tp_name = self.mcp_toolpath_name(p.index);
                Some(format!("MCP: Set {param} = {value_str} on '{tp_name}'"))
            }
            CoreRequest::SetToolParam(p) => {
                let param = &p.param;
                let value_str = display_value(&p.value);
                let tools = self.controller.state().session.list_tools();
                let tool_name = tools
                    .get(p.index)
                    .map_or_else(|| format!("#{}", p.index), |t| t.name.clone());
                Some(format!("MCP: Set {param} = {value_str} on '{tool_name}'"))
            }
            CoreRequest::SetToolpathTool(p) => {
                let tool_id = p.tool_id;
                let tp_name = self.mcp_toolpath_name(p.index);
                Some(format!("MCP: Bound tool {tool_id} to '{tp_name}'"))
            }
            CoreRequest::SetToolpathModel(p) => {
                let model_id = p.model_id;
                let tp_name = self.mcp_toolpath_name(p.index);
                Some(format!("MCP: Bound model {model_id} to '{tp_name}'"))
            }
            CoreRequest::AddToolpath(p) => {
                // The request's own words: its `name` when it gave one,
                // and its `operation_type` string when it did not.
                let display_name = p.name.as_deref().unwrap_or(&p.operation_type);
                Some(format!("MCP: Added toolpath '{display_name}'"))
            }
            CoreRequest::RemoveToolpath(p) => {
                let index = p.index;
                Some(format!("MCP: Removed toolpath {index}"))
            }
            CoreRequest::AddTool(p) => {
                let name = &p.name;
                Some(format!("MCP: Added tool '{name}'"))
            }
            CoreRequest::AddToolFromLibrary(p) => {
                let catalog = &p.catalog;
                let index = p.index;
                Some(format!(
                    "MCP: Imported tool from library '{catalog}' #{index}"
                ))
            }
            // These rows pushed no toast before WP4 and push none now.
            // Folding them into a toasting arm would put a message on
            // screen the operator never had.
            CoreRequest::AddAlignmentPin(_)
            | CoreRequest::RemoveAlignmentPin(_)
            | CoreRequest::SetToolpathHeights(_)
            | CoreRequest::RemoveTool(_)
            | CoreRequest::SetStockConfig(_)
            | CoreRequest::SetStockSource(_)
            | CoreRequest::SetMachineKinematics(_)
            | CoreRequest::ImportMachineSettings(_)
            | CoreRequest::LoadMachineFromLibrary(_)
            | CoreRequest::SetSpindleStrategy(_)
            | CoreRequest::SetBoundaryConfig(_)
            | CoreRequest::SetRestAnalysisConfig(_)
            | CoreRequest::SetDressupConfig(_)
            | CoreRequest::SetDressupField(_)
            | CoreRequest::SetToolpathEnabled(_)
            | CoreRequest::SetSimulationResolution(_) => None,
        }
    }

    /// The name of the toolpath at `index`, or `#index` when the index
    /// names none.
    fn mcp_toolpath_name(&self, index: usize) -> String {
        self.controller
            .state()
            .session
            .toolpath_configs()
            .get(index)
            .map_or_else(|| format!("#{index}"), |tc| tc.name.clone())
    }

    /// Stamp `stale_since` on the toolpaths the command dropped, and
    /// report them for the reply's `stale_toolpaths`.
    ///
    /// WP19 (H3): the stamp takes `crate::state::stale::stamp_stale`,
    /// the one helper. The reply is unchanged — the same indices, in
    /// the same order.
    fn core_stale(&mut self, effects: &Effects) -> Vec<usize> {
        crate::state::stale::stamp_stale(self.controller.state_mut(), &effects.stale);
        effects.stale.iter().copied().collect()
    }

    /// The refusal reply for one row.
    ///
    /// Every row answers with `mutation_error_json` and its own field
    /// name; three rows say more. The match is exhaustive so a new row
    /// states its refusal shape rather than inheriting one.
    fn core_error_reply(
        &self,
        id: CommandId,
        error: &SessionError,
        before: &CoreBefore,
    ) -> CoreReply {
        let field = before.field.as_deref();
        match id {
            // Name what WAS valid — an agent that passed a positional
            // index where an id was wanted needs the list, not just the
            // refusal. Only for the tool arm: a bad toolpath index is a
            // different mistake and the tool list would be noise.
            CommandId::SetToolpathTool => {
                let reply = match error {
                    SessionError::ToolNotFound(_) => {
                        let ids: Vec<usize> = self
                            .controller
                            .state()
                            .session
                            .tools()
                            .iter()
                            .map(|t| t.id.0)
                            .collect();
                        mutation_error_json(
                            &format!("Error: {error}. Tool ids in this project: {ids:?}"),
                            Some("tool_id"),
                        )
                    }
                    _ => mutation_error_json(&format!("Error: {error}"), Some("index")),
                };
                CoreReply::quiet(reply)
            }
            // The core setter's own refusal already lists the model ids
            // that exist, so nothing is appended here.
            CommandId::SetToolpathModel => {
                let refused_field = match error {
                    SessionError::MissingGeometry(_) => "model_id",
                    _ => "index",
                };
                CoreReply::quiet(mutation_error_json(
                    &format!("Error: {error}"),
                    Some(refused_field),
                ))
            }
            // A save answers in plain text, so its refusal does too. The
            // sentence carried "Save failed" twice until WP20, because
            // this surface printed the GUI door's wrapper text under a
            // prefix of its own. The MCP route does not take that door:
            // it applies `Command::SaveProject` and reports the session
            // error, which carries no such prefix. One prefix is
            // correct, and the reply now reads the same bytes as the
            // outcome.
            CommandId::SaveProject => CoreReply {
                reply: text(format!("Save failed: {error}")),
                outcome: Some(McpOutcome::Refused(format!("Save failed: {error}"))),
            },
            CommandId::AddModel
            | CommandId::AdoptModelGeometry
            | CommandId::AddSetup
            | CommandId::RemoveSetup
            | CommandId::AddAlignmentPin
            | CommandId::RemoveAlignmentPin
            | CommandId::SetSetupFace
            | CommandId::SetSetupRotation
            | CommandId::SetSetupName
            | CommandId::SetSetupDatum
            | CommandId::SetSetupModels
            | CommandId::SetSetupPauseMessage
            | CommandId::MoveToolpathToSetup
            | CommandId::SetToolpathParam
            | CommandId::AdoptResult
            | CommandId::SetToolParam
            | CommandId::SetToolpathHeights
            | CommandId::SetToolpathDebugOptions
            | CommandId::AddToolpath
            | CommandId::RemoveToolpath
            | CommandId::AddTool
            | CommandId::AddToolFromLibrary
            | CommandId::RemoveTool
            | CommandId::SetStockConfig
            | CommandId::SetStockSource
            | CommandId::SetMachine
            | CommandId::SetMachineKinematics
            | CommandId::ImportMachineSettings
            | CommandId::SetPostConfig
            | CommandId::SetBoundaryConfig
            | CommandId::SetRestAnalysisConfig
            | CommandId::SetDressupConfig
            | CommandId::SetDressupField
            | CommandId::SetToolpathEnabled
            | CommandId::SetSimulationResolution
            | CommandId::ReplaceTool
            | CommandId::ReplaceFixture
            | CommandId::ReplaceKeepOut
            | CommandId::ToolpathCycleTime
            | CommandId::GetOperationSchema
            | CommandId::RestoreToolpathSnapshot
            | CommandId::ReplaceToolpathConfig
            | CommandId::GenerateToolpath
            | CommandId::RecommendClearingStrategy
            | CommandId::PreviewTierMap
            | CommandId::OptimizeToolpath
            | CommandId::AdoptSimulation
            // WP15a rows. Every one declares `mcp: Reach::Skip`, so no
            // wire tool builds one. The GUI dispatches thirteen of them;
            // eleven carry no caller at all.
            | CommandId::ReorderToolpath
            | CommandId::RemoveModel
            | CommandId::SetFaceSelection
            | CommandId::SetAlignmentPinDrillHoles
            | CommandId::SetDrillSelectedHoles
            | CommandId::AddFixture
            | CommandId::RemoveFixture
            | CommandId::AddKeepOut
            | CommandId::RemoveKeepOut
            | CommandId::AutoEnableRestAnalysis
            | CommandId::ForgetResult
            | CommandId::SetToolpathOperation
            | CommandId::InvalidateStock
            | CommandId::InvalidateMachine
            | CommandId::InvalidateTool
            | CommandId::InvalidateModel
            | CommandId::InvalidateToolpathInputs
            | CommandId::UpdateStockFromBbox
            | CommandId::ReplaceTools
            | CommandId::SetFeedsProvenance => {
                CoreReply::quiet(mutation_error_json(&format!("Error: {error}"), field))
            }
        }
    }
}

impl RsCamApp {
    /// Build the reply for one applied command, and the view writes that
    /// belong after the mutation.
    ///
    /// The match is exhaustive over every registry row. A row the wire
    /// cannot reach answers that it is not a wire mutation, so a new row
    /// still has to state what it does here before it compiles.
    pub(crate) fn describe_core(
        &mut self,
        id: CommandId,
        outcome: Result<Effects, SessionError>,
        before: &CoreBefore,
    ) -> CoreReply {
        let effects = match outcome {
            Ok(effects) => effects,
            Err(error) => return self.core_error_reply(id, &error, before),
        };
        match id {
            CommandId::AddAlignmentPin => {
                let pin_count = self
                    .controller
                    .state()
                    .session
                    .stock_config()
                    .alignment_pins
                    .len();
                // The setter dedupes, so a duplicate leaves the count
                // where it was. That is the old `false`.
                let added = before.count.is_some_and(|was| pin_count > was);
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                let x = before.extra_f64("x");
                let y = before.extra_f64("y");
                let diameter = before.extra_f64("diameter");
                let message = if added {
                    format!("Added alignment pin at ({x:.1}, {y:.1}) dia {diameter:.1}mm")
                } else {
                    format!("Pin already present at ({x:.1}, {y:.1}); skipped duplicate")
                };
                CoreReply::quiet(self.mcp_mutation_result(
                    message,
                    serde_json::json!({
                        "added": added,
                        "pin_count": pin_count,
                    }),
                    Vec::new(),
                    &before.diagnostics,
                ))
            }
            CommandId::RemoveAlignmentPin => {
                // A pin edit reaches every toolpath through the stock,
                // and this row reported no stale set before WP4. It
                // still reports none: `set_stock_config`'s own row is
                // the one that answers for the stock, and the two must
                // move together.
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                let pin_count = self
                    .controller
                    .state()
                    .session
                    .stock_config()
                    .alignment_pins
                    .len();
                let index = before.index();
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Removed alignment pin {index}"),
                    serde_json::json!({ "pin_count": pin_count }),
                    Vec::new(),
                    &before.diagnostics,
                ))
            }
            CommandId::AddModel => {
                let (id, name, kind, bbox) = {
                    let models = self.controller.state().session.models();
                    let model = models.last();
                    (
                        model.map(|m| m.id).unwrap_or(0),
                        model.map(|m| m.name.clone()).unwrap_or_default(),
                        model
                            .and_then(|m| m.kind)
                            .map(|k| format!("{k:?}"))
                            .unwrap_or_else(|| before.extra_str("ext")),
                        model.and_then(|m| m.bbox()),
                    )
                };
                // The controller's own import door writes these three
                // after every import; the MCP route takes the same step.
                self.controller.state_mut().selection =
                    Selection::Model(crate::state::job::ModelId(id));
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();

                let mut resp = serde_json::json!({
                    "id": id,
                    "name": name,
                    "kind": kind.to_lowercase(),
                });
                if let Some(bbox) = bbox {
                    // SAFETY: resp is a known JSON object just built here
                    #[allow(clippy::indexing_slicing)]
                    {
                        resp["bbox"] = serde_json::json!({
                            "min": [bbox.min.x, bbox.min.y, bbox.min.z],
                            "max": [bbox.max.x, bbox.max.y, bbox.max.z],
                        });
                        resp["dimensions"] = serde_json::json!({
                            "x": bbox.max.x - bbox.min.x,
                            "y": bbox.max.y - bbox.min.y,
                            "z": bbox.max.z - bbox.min.z,
                        });
                    }
                }
                CoreReply::quiet(json_str(resp))
            }
            CommandId::AddSetup => {
                let index = effects.created.unwrap_or_default();
                let setup = self
                    .controller
                    .state()
                    .session
                    .list_setups()
                    .get(index)
                    .map(|s| (s.id, s.name.clone()));
                let Some((setup_id, name)) = setup else {
                    return CoreReply::quiet(mutation_error_json(
                        "Error: Failed to add setup",
                        None,
                    ));
                };
                // The GUI's own add door selects the new setup; the MCP
                // route takes the same step.
                self.controller.state_mut().selection =
                    Selection::Setup(crate::state::job::SetupId(setup_id));
                self.controller.state_mut().gui.mark_edited();
                let reply = self.mcp_mutation_result(
                    format!("Added setup {index}"),
                    serde_json::json!({
                        "index": index,
                        "id": setup_id,
                        "name": name,
                    }),
                    Vec::new(),
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            // GUI-only row: no MCP tool constructs it. Keep the exhaustive
            // command-reply match honest if that surface changes later.
            CommandId::RemoveSetup => CoreReply::quiet(mutation_error_json(
                "Error: remove_setup is available from the GUI only",
                None,
            )),
            CommandId::SetSetupFace => {
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                let stale = self.core_stale(&effects);
                let setup_index = before.index();
                let face_up = before.extra_str("face_up");
                let reply = self.mcp_mutation_result(
                    format!("Set setup {setup_index} face to {face_up}"),
                    serde_json::json!({
                        "setup_index": setup_index,
                        "face_up": face_up,
                    }),
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::SetSetupRotation => {
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                let stale = self.core_stale(&effects);
                let setup_index = before.index();
                let label = before.extra_str("z_rotation");
                let reply = self.mcp_mutation_result(
                    format!(
                        "Set setup {setup_index} Z rotation to {label}. Regenerate the setup's \
                         toolpaths to apply."
                    ),
                    serde_json::json!({
                        "setup_index": setup_index,
                        "z_rotation": label,
                    }),
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::MoveToolpathToSetup => {
                // §15 ruling 5: the move runs BEFORE the reply now, so
                // `stale_toolpaths` reports the set the move really
                // dropped. The event route replied first and named the
                // moved index alone.
                self.controller.set_pending_upload();
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let toolpath_index = before.index();
                let target_setup_index = before
                    .extra
                    .get("target_setup_index")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default();
                let reply = self.mcp_mutation_result(
                    format!("Moved toolpath {toolpath_index} to setup {target_setup_index}"),
                    serde_json::json!({
                        "toolpath_index": toolpath_index,
                        "target_setup_index": target_setup_index,
                    }),
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::SaveProject => {
                let path = before.display_name();
                self.controller.state_mut().gui.file_path = Some(PathBuf::from(&path));
                self.controller.state_mut().gui.dirty = false;
                CoreReply {
                    reply: text(format!("Project saved to {path}")),
                    outcome: Some(McpOutcome::Succeeded),
                }
            }
            CommandId::SetToolpathParam => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let param = before.display_name();
                let applied = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| {
                        tc.operation
                            .params_value_including_nulls()
                            .get(&param)
                            .cloned()
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .unwrap_or(serde_json::Value::Null);
                let reply = self.mcp_mutation_result(
                    format!("Set toolpath {index} param '{param}'. Regenerate to apply."),
                    applied,
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::SetToolParam => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let param = before.display_name();
                let applied = self
                    .controller
                    .state()
                    .session
                    .tools()
                    .get(index)
                    .and_then(|tool| serde_json::to_value(tool).ok())
                    .and_then(|tool| tool.get(&param).cloned())
                    .unwrap_or(serde_json::Value::Null);
                let reply = self.mcp_mutation_result(
                    format!(
                        "Set tool {index} param '{param}'. Regenerate affected toolpaths to apply."
                    ),
                    applied,
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::SetToolpathTool => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let tool_id = before.count.unwrap_or_default();
                let tool = {
                    let session = &self.controller.state().session;
                    session
                        .tools()
                        .iter()
                        .find(|t| t.id.0 == tool_id)
                        .map(|t| {
                            serde_json::json!({
                                "id": t.id.0,
                                "name": t.name,
                                "tool_type": t.tool_type,
                                "diameter": t.diameter,
                                "size_label": t.size_label(),
                            })
                        })
                        .unwrap_or(serde_json::Value::Null)
                };
                let reply = self.mcp_mutation_result(
                    format!("Bound toolpath {index} to tool id {tool_id}. Regenerate to apply."),
                    serde_json::json!({ "index": index, "tool": tool }),
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::SetToolpathModel => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let model_id = before.count.unwrap_or_default();
                let model = {
                    let session = &self.controller.state().session;
                    session
                        .models()
                        .iter()
                        .find(|m| m.id == model_id)
                        .map(|m| {
                            serde_json::json!({
                                "id": m.id,
                                "name": m.name,
                                "kind": m.kind,
                                "has_mesh": m.mesh.is_some(),
                                "has_polygons": m.polygons.is_some(),
                            })
                        })
                        .unwrap_or(serde_json::Value::Null)
                };
                let reply = self.mcp_mutation_result(
                    format!("Bound toolpath {index} to model id {model_id}. Regenerate to apply."),
                    serde_json::json!({ "index": index, "model": model }),
                    stale,
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::SetToolpathHeights => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Set toolpath {index} heights. Regenerate to apply."),
                    before.extra.clone(),
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::AddToolpath => {
                let index = effects.created.unwrap_or_default();
                // Create the GUI runtime entry for the new toolpath.
                // Read the id and the auto-regen flag before mutating gui.
                let tp_info = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| (tc.id, tc.operation.default_auto_regen()));
                if let Some((id, auto_regen)) = tp_info {
                    self.controller
                        .state_mut()
                        .gui
                        .toolpath_rt
                        .insert(id, crate::state::runtime::ToolpathRuntime::new(auto_regen));
                    // Select the newly added toolpath so its properties
                    // are visible.
                    self.controller.state_mut().selection = Selection::Toolpath(id);
                }
                self.controller.state_mut().gui.mark_edited();
                let label = before.extra_str("operation");
                let feeds_refusal = before.extra_str("feeds_refusal");
                let message = if feeds_refusal.is_empty() {
                    format!("Added toolpath {index} ({label}).")
                } else {
                    format!(
                        "Added toolpath {index} ({label}) without a feeds recipe: {feeds_refusal}"
                    )
                };
                let reply = self.mcp_mutation_result(
                    message,
                    serde_json::json!({
                        "index": index,
                        "operation": label,
                        "feeds_refusal": before.extra("feeds_refusal"),
                    }),
                    vec![index],
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::RemoveToolpath => {
                // The reply keeps its empty stale list. A removal shifts
                // every index above the removed one, so `remove_toolpath`
                // bumps EVERY revision and `Effects::stale` holds every
                // remaining toolpath. That is an index-bookkeeping fact,
                // not a claim that those results need regeneration — the
                // removal re-keys them and keeps them. Stamping them
                // would put STALE on every card in the project.
                if let Some(id) = before.toolpath_id {
                    self.controller.state_mut().gui.toolpath_rt.remove(&id);
                }
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                let index = before.index();
                let reply = self.mcp_mutation_result(
                    format!("Removed toolpath {index}"),
                    serde_json::json!({ "index": index }),
                    Vec::new(),
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::AddTool => {
                let index = effects.created.unwrap_or_default();
                self.controller.state_mut().gui.mark_edited();
                let geometry = self
                    .controller
                    .state()
                    .session
                    .tools()
                    .get(index)
                    .and_then(|tool| serde_json::to_value(tool).ok())
                    .unwrap_or(serde_json::Value::Null);
                let name = before.display_name();
                let tool_number = before
                    .extra
                    .get("tool_number")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default();
                let conflict = before
                    .extra
                    .get("tool_number_conflict")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or_default();
                let defaulted = before.extra("defaulted");
                let defaulted_count = defaulted.as_array().map_or(0, Vec::len);
                let summary = if conflict {
                    format!(
                        "Added tool '{name}' at index {index}, but tool_number {tool_number} is \
                         ALREADY USED by another tool in this project. An M6 tool change only \
                         re-triggers on a change of number, so two tools sharing one collapse \
                         into a single change on export. Pass a distinct tool_number, or omit it \
                         to auto-allocate."
                    )
                } else if defaulted_count == 0 {
                    format!("Added tool '{name}' at index {index} (tool_number {tool_number}).")
                } else {
                    format!(
                        "Added tool '{name}' at index {index} (tool_number {tool_number}). \
                         {defaulted_count} field(s) came from generic defaults, not from your \
                         tool — see `defaulted`."
                    )
                };
                let reply = self.mcp_mutation_result(
                    summary,
                    serde_json::json!({
                        "index": index,
                        "tool_type": before.extra("tool_type"),
                        "diameter": before.extra("diameter"),
                        "tool_number": tool_number,
                        "tool_number_conflict": conflict,
                        "defaulted": defaulted,
                        "geometry": geometry,
                    }),
                    Vec::new(),
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::AddToolFromLibrary => {
                let index = effects.created.unwrap_or_default();
                self.controller.state_mut().gui.mark_edited();
                let name = before.display_name();
                let catalog = before.extra_str("source_catalog");
                let source_index = before.index();
                let tool_number = before
                    .extra
                    .get("tool_number")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default();
                let catalog_tool_number = before
                    .extra
                    .get("catalog_tool_number")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default();
                let renumbered = before
                    .extra
                    .get("renumbered")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or_default();
                let summary = if renumbered {
                    format!(
                        "Imported '{name}' from {catalog} as tool {index}, renumbered from \
                         tool_number {catalog_tool_number} to {tool_number} (the catalog's \
                         number was already in use — duplicate numbers collapse M6 tool changes \
                         on export)."
                    )
                } else {
                    format!(
                        "Imported '{name}' from {catalog} as tool {index} (tool_number \
                         {tool_number})."
                    )
                };
                let reply = self.mcp_mutation_result(
                    summary,
                    serde_json::json!({
                        "index": index,
                        "name": name,
                        "source_catalog": catalog,
                        "source_index": source_index,
                        "tool_number": tool_number,
                        "tool_number_renumbered_from": before
                            .extra("tool_number_renumbered_from"),
                    }),
                    Vec::new(),
                    &before.diagnostics,
                );
                CoreReply::quiet(reply)
            }
            CommandId::RemoveTool => {
                self.controller.state_mut().gui.mark_edited();
                let index = before.index();
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Removed tool {index}"),
                    serde_json::json!({ "index": index }),
                    Vec::new(),
                    &before.diagnostics,
                ))
            }
            CommandId::SetStockConfig => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let (x, y, z, origin_x, origin_y, origin_z, material_label) = {
                    let stock = self.controller.state().session.stock_config();
                    (
                        stock.x,
                        stock.y,
                        stock.z,
                        stock.origin_x,
                        stock.origin_y,
                        stock.origin_z,
                        stock.material.label(),
                    )
                };
                let auto_before = before
                    .extra
                    .get("auto_from_model_was")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or_default();
                let auto_after = before
                    .extra
                    .get("auto_from_model")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or_default();
                let geometry_set = before.extra("fields_set");
                let geometry_was_set = geometry_set.as_array().is_some_and(|v| !v.is_empty());
                let auto_note = if auto_before && !auto_after {
                    Some(
                        "auto_from_model was true and has been CLEARED: it would have re-derived \
                         the stock size and origin from the next imported model's bounding box, \
                         silently overwriting what you just set."
                            .to_owned(),
                    )
                } else if auto_after && geometry_was_set {
                    Some(
                        "auto_from_model is TRUE and you set stock geometry explicitly: the next \
                         import_model will re-derive size and origin from that model's bounding \
                         box and overwrite the values just written."
                            .to_owned(),
                    )
                } else {
                    None
                };
                let summary = format!(
                    "Stock: {x:.1} x {y:.1} x {z:.1} mm at origin ({origin_x:.2}, {origin_y:.2}, \
                     {origin_z:.2}), material {material_label}, \
                     auto_from_model {auto_after}. Regenerate toolpaths and simulation to apply."
                );
                CoreReply::quiet(self.mcp_mutation_result(
                    summary,
                    serde_json::json!({
                        "x": x,
                        "y": y,
                        "z": z,
                        "origin": { "x": origin_x, "y": origin_y, "z": origin_z },
                        "stock_top_z": origin_z + z,
                        "material": material_label,
                        "auto_from_model": auto_after,
                        "auto_from_model_was": auto_before,
                        "fields_set": geometry_set,
                        "note": auto_note,
                    }),
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetStockSource => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let source = before.extra_str("stock_source");
                CoreReply::quiet(self.mcp_mutation_result(
                    format!(
                        "Stock source set to '{source}' on toolpath {index}. Regenerate to apply."
                    ),
                    serde_json::json!({ "index": index, "stock_source": source }),
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetMachine => {
                // `set_machine` clears the simulation itself since WP4,
                // so the `MachineChanged` event this route used to push
                // would only repeat the invalidation. Its other half,
                // `mark_edited`, is kept here.
                self.controller.state_mut().gui.mark_edited();
                let profile_name = before.display_name();
                CoreReply::quiet(json_str(serde_json::json!({
                    "ok": true,
                    "imported": before.extra("imported"),
                    "profile_name": profile_name,
                    "note": "snapshot copy applied to the project's inline machine; verify with inspect_machine",
                })))
            }
            CommandId::SetMachineKinematics => {
                self.controller.state_mut().gui.mark_edited();
                CoreReply::quiet(json_str(serde_json::json!({
                    "ok": true,
                    "applied": before.extra.clone(),
                    "note": "Kinematics applied inline; any machine-library link is cleared. \
                             Acceleration feeds the cycle-time integrator and the \
                             recommend_clearing_strategy verdict; the per-axis max rates cap \
                             every move's cruise velocity, so a Z-dominant move is throttled by \
                             $112 — re-run both if you relied on them. Verify with \
                             inspect_machine.",
                })))
            }
            CommandId::ImportMachineSettings => {
                self.controller.state_mut().gui.mark_edited();
                let new_max_feed = self.controller.state().session.machine().max_feed_mm_min;
                // Built field by field, in the order the reply always
                // carried them: `serde_json` runs with `preserve_order`
                // here, so an object built by patching would move
                // `max_feed_mm_min` to the end.
                let applied = serde_json::json!({
                    "acceleration_xyz_mm_s2": before.extra("acceleration_xyz_mm_s2"),
                    "acceleration_mm_s2": before.extra("acceleration_mm_s2"),
                    "max_rate_xyz_mm_min": before.extra("max_rate_xyz_mm_min"),
                    "junction_deviation_mm": before.extra("junction_deviation_mm"),
                    "max_feed_mm_min": { "from": before.number, "to": new_max_feed },
                    "arc_tolerance_mm": before.extra("arc_tolerance_mm"),
                    "max_spindle_rpm": before.extra("max_spindle_rpm"),
                    "ignored_settings": before.extra("ignored_settings"),
                });
                CoreReply::quiet(json_str(serde_json::json!({
                    "ok": true,
                    "applied": applied,
                    "note": "kinematics applied; machine-library link cleared. Verify with inspect_machine.",
                })))
            }
            CommandId::SetPostConfig => {
                // SHL-01: rebuild the viz mirror through the one door.
                // The GUI's Feeds & Speeds modal reads that copy, not
                // the session's. This arm copied `spindle_strategy`
                // alone, so a second field of the post block set through
                // this route would not have reached the panel.
                self.controller.refresh_post_mirror();
                self.controller.state_mut().gui.mark_edited();
                let strategy = before.display_name();
                // Suggest is the read-side consumer — no toolpaths go
                // stale from this change. An operator or an agent runs
                // get_toolpath_params or re-suggests to see the new
                // recommendations.
                CoreReply::quiet(self.mcp_mutation_result(
                    format!(
                        "Spindle policy set to '{strategy}'. Run Suggest (per toolpath or \
                         project-wide) to see new recommended feeds/RPMs."
                    ),
                    serde_json::json!({ "spindle_strategy": strategy, "changed": true }),
                    Vec::new(),
                    &before.diagnostics,
                ))
            }
            CommandId::SetBoundaryConfig => {
                self.controller.state_mut().gui.mark_edited();
                let mut stale = self.core_stale(&effects);
                let index = before.index();
                // P2 pencil-panel consolidation: `set_boundary_config`
                // may have just auto-enabled rest analysis on the SOURCE
                // toolpath (`ProjectSession::auto_enable_rest_analysis_for_source`,
                // the demand-driven producer hook — see its doc comment).
                // Surface that toolpath as stale too so a live GUI session
                // watching this MCP-driven change sees it needs
                // regeneration, mirroring the GUI boundary picker's own
                // handling in `properties/mod.rs`. Slightly conservative:
                // fires whenever the boundary points at an already-enabled
                // source too (harmless — just an extra "needs regen" nudge).
                self.core_stale_rest_source(index, &mut stale);
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Boundary set on toolpath {index}. Regenerate to apply."),
                    before.extra.clone(),
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetRestAnalysisConfig => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Rest analysis set on toolpath {index}. Regenerate to apply."),
                    before.extra.clone(),
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetDressupConfig => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let applied = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .and_then(|tc| serde_json::to_value(&tc.dressups).ok())
                    .unwrap_or(serde_json::Value::Null);
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Dressup config set on toolpath {index}. Regenerate to apply."),
                    applied,
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetDressupField => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.core_stale(&effects);
                let index = before.index();
                let key = before.display_name();
                let applied = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .and_then(|tc| serde_json::to_value(&tc.dressups).ok())
                    .and_then(|dressups| dressups.get(&key).cloned())
                    .unwrap_or(serde_json::Value::Null);
                CoreReply::quiet(self.mcp_mutation_result(
                    format!("Dressup field '{key}' set on toolpath {index}. Regenerate to apply."),
                    applied,
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetSimulationResolution => {
                self.controller.state_mut().gui.mark_edited();
                if effects.simulation_cleared {
                    self.controller.invalidate_simulation();
                }
                let stale = self.core_stale(&effects);
                let report = crate::controller::generate_all::ResolutionReport::of(
                    &self.controller.state().session,
                    true,
                );
                CoreReply::quiet(self.mcp_mutation_result(
                    format!(
                        "Simulation resolution set to {:.3} mm ({})",
                        report.mm, report.mode
                    ),
                    serde_json::json!({ "simulation_resolution": report }),
                    stale,
                    &before.diagnostics,
                ))
            }
            CommandId::SetToolpathEnabled => {
                self.controller.state_mut().gui.mark_edited();
                // The toggle keeps its OWN result: a re-enable must not
                // cost a regeneration. Only the results built on the
                // stock it leaves are dropped, so `Effects::stale` is the
                // downstream set and `index` is not in it.
                let stale = self.core_stale(&effects);
                let index = before.index();
                let enabled = before
                    .extra
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or_default();
                CoreReply::quiet(self.mcp_mutation_result(
                    format!(
                        "Toolpath {index} {}",
                        if enabled { "enabled" } else { "disabled" }
                    ),
                    serde_json::json!({ "index": index, "enabled": enabled }),
                    stale,
                    &before.diagnostics,
                ))
            }
            // Rows the wire cannot reach. `CoreRequest` names no variant
            // for any of them, so this arm answers a request that cannot
            // be built rather than a request that can.
            //
            // Two of them ARE dispatched, just not from the wire.
            // `SetToolpathDebugOptions` is applied by the
            // `generate_toolpath` MCP arm immediately before it starts the
            // job (WP11b), and `GenerateToolpath` is a `Job` that arm runs
            // through the controller. Neither is a wire MUTATION, which is
            // what this arm answers about.
            CommandId::AdoptResult
            | CommandId::AdoptModelGeometry
            | CommandId::SetSetupName
            | CommandId::SetSetupDatum
            | CommandId::SetSetupModels
            | CommandId::SetSetupPauseMessage
            | CommandId::SetToolpathDebugOptions
            | CommandId::ReplaceTool
            | CommandId::ReplaceFixture
            | CommandId::ReplaceKeepOut
            | CommandId::ToolpathCycleTime
            | CommandId::GetOperationSchema
            | CommandId::RestoreToolpathSnapshot
            | CommandId::ReplaceToolpathConfig
            | CommandId::GenerateToolpath
            | CommandId::RecommendClearingStrategy
            | CommandId::PreviewTierMap
            | CommandId::OptimizeToolpath
            | CommandId::AdoptSimulation
            // WP15a rows. Every one declares `mcp: Reach::Skip`, so
            // `CoreRequest` names no variant for any of them either. The
            // GUI dispatches thirteen of them; eleven carry no caller.
            | CommandId::ReorderToolpath
            | CommandId::RemoveModel
            | CommandId::SetFaceSelection
            | CommandId::SetAlignmentPinDrillHoles
            | CommandId::SetDrillSelectedHoles
            | CommandId::AddFixture
            | CommandId::RemoveFixture
            | CommandId::AddKeepOut
            | CommandId::RemoveKeepOut
            | CommandId::AutoEnableRestAnalysis
            | CommandId::ForgetResult
            | CommandId::SetToolpathOperation
            | CommandId::InvalidateStock
            | CommandId::InvalidateMachine
            | CommandId::InvalidateTool
            | CommandId::InvalidateModel
            | CommandId::InvalidateToolpathInputs
            | CommandId::UpdateStockFromBbox
            | CommandId::ReplaceTools
            | CommandId::SetFeedsProvenance => CoreReply::quiet(mutation_error_json(
                &format!(
                    "Error: '{}' is not a wire mutation; no MCP tool dispatches it.",
                    id.wire_name()
                ),
                None,
            )),
        }
    }

    /// Mark the rest-analysis SOURCE toolpath of a derived-rest boundary
    /// stale, and add it to the reply's set.
    fn core_stale_rest_source(&mut self, index: usize, stale: &mut Vec<usize>) {
        let source = {
            let state = self.controller.state();
            let Some(tc) = state.session.get_toolpath_config(index) else {
                return;
            };
            if !tc.boundary.enabled {
                return;
            }
            let BoundarySource::DerivedRestRegions { source_toolpath_id } = &tc.boundary.source
            else {
                return;
            };
            let source_toolpath_id = *source_toolpath_id;
            state
                .session
                .find_toolpath_config_by_id(source_toolpath_id)
                .map(|(idx, source_tc)| {
                    let is_rest_depth_pencil = matches!(
                        &source_tc.operation,
                        rs_cam_core::compute::catalog::OperationConfig::Pencil(cfg)
                            if cfg.detector
                                == rs_cam_core::finish::pencil::PencilDetector::RestDepth
                    );
                    (idx, source_toolpath_id, is_rest_depth_pencil)
                })
        };
        if let Some((source_index, source_toolpath_id, false)) = source {
            if let Some(rt) = self
                .controller
                .state_mut()
                .gui
                .toolpath_rt
                .get_mut(&source_toolpath_id)
            {
                rt.stale_since = Some(std::time::Instant::now());
            }
            if !stale.contains(&source_index) {
                stale.push(source_index);
            }
        }
    }
}
