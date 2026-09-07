//! Voxel terrain — the 3D shape layer over the heightfield.
//!
//! The heightfield terrain is 2.5D by construction: one height per XZ. That is
//! the right representation for the 95% of a world that is ground, and the
//! wrong one for the 5% that is rock — a cliff carved into a height grid can
//! never be vertical, never undercut, and never have anything under it.
//!
//! This module adds the missing dimension **without replacing** what works:
//!
//! * [`field::VoxelField`] is the shape authority. Its base term is the same
//!   `BrushGrid` every carve already writes to, so a column with no 3D feature
//!   over it resolves to the heightfield value at the heightfield's cost.
//! * [`mods`] are the 3D solids — a cliff body, a talus cone, a cave tube.
//!   Unlike a carve, a mod stays an object for the life of the world, so no
//!   downstream system has to re-derive "is this rock?" from the numbers.
//! * [`index::ModIndex`] answers *does anything 3D touch this column?* in O(1),
//!   which is what keeps the cost proportional to the authored area instead of
//!   to the world.
//! * [`transvoxel_mesh`] meshes the chunks that need it — marching cubes
//!   com células de transição, que é o que fecha a costura entre LODs.

pub mod arch;
pub mod bridge;
pub mod cave;
pub mod cliff;
pub mod field;
pub mod index;
pub mod mods;
pub mod riverbank;
pub mod scatter;
pub mod spawn;
pub mod transvoxel_mesh;

pub use arch::ArchSpec;
pub use bridge::{BridgeSpec, BridgeStyle};
pub use cave::CaveSpec;
pub use cliff::{CliffBand, build_cliff_mods, profile_offset};
pub use field::{Span, VoxelField};
pub use index::{ChunkClass, ModIndex};
pub use mods::{
    ArchMod, Bounds3, BoxMod, CapsuleMod, EllipsoidMod, ModOp, OrientedBoxMod, RoundConeMod,
    VoxelMod,
};
pub use scatter::{RockFeaturesSpec, ScatterGuards, ScatterResult, ScatterStats, TakenDisc};
pub use spawn::{
    VoxelBoxSpec, VoxelChunk, VoxelLodShape, VoxelSpawnStats, build_box_mesh, column_boxes,
    lod_shape, spawn_voxel_columns,
};
pub(crate) use spawn::{spawn_box_entity, spawn_column};
pub use transvoxel_mesh::{
    VOXEL_CHUNK_CELLS, VoxelChunkParams, build_voxel_mesh, transition_sides,
};
