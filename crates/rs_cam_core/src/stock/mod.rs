//! The stock data model beside the simulation engine: the tri-dexel types,
//! the cut record, the triage over it, and the meshes the simulation emits.
//!
//! The engine itself is `dexel_stock`. `collision` lives here because it
//! checks a cutter envelope against simulated material.

pub mod collision;
pub mod dexel;
pub mod dexel_mesh;
pub mod dexel_mesh_mc;
pub mod radial_profile;
pub mod sim_measurability;
pub mod sim_triage;
pub mod simulation_cut;
pub mod stock_mesh;
