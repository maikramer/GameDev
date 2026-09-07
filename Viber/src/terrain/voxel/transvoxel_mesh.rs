//! Transvoxel — o mesher das caixas voxel.
//!
//! Marching cubes com **células de transição** (Eric Lengyel): a face de um
//! bloco que confina com um vizinho do dobro da resolução é remalhada numa
//! fiada de células a meia escala, encolhida por
//! `Coordinate::shrink_factor()` (0.15 de célula). É isso, e só isso, que
//! fecha a costura entre duas colunas em LODs diferentes — o surface nets que
//! isto substitui só a tapava com uma cortina vertical (`seal_depth`), que era
//! geometria a mais e colidia como parede dentro do chão.
//!
//! # Convenção de sinal
//!
//! O Viber usa **negativo = sólido** (`VoxelField::density`); o transvoxel usa
//! `Density::inside(t) = self > t`. Alimentamos `-density` com `threshold = 0`.
//! O gradiente que o algoritmo estima é então o de `-density`, e a normal para
//! fora é `normalize(-grad)` — que é exatamente `normalize(∇density)`, a mesma
//! normal contínua que o mesher anterior calculava (`VoxelField::gradient`).
//!
//! # Coordenadas
//!
//! O transvoxel trabalha em coordenadas do mundo (o `Block` carrega a base).
//! As posições saem daqui **relativas à origem da caixa** nos três eixos,
//! porque é a entidade da caixa que carrega o offset.

use bevy::math::{Vec2, Vec3};
use transvoxel::prelude::*;
use transvoxel::structs::grid_point::GridPoint;
use transvoxel::structs::vertex_index::VertexIndex;
use transvoxel::traits::data_field::DataField;
use transvoxel::traits::mesh_builder::MeshBuilder;

use super::super::mesh::{ChunkMeshData, TintParams, tint_vertex_color};

/// Cells per axis in one voxel chunk.
///
/// 32³ é a unidade que o orçamento de frame dita. Um chunk de terreno do
/// `simple-rpg` tem 64 m a passo de 1 m; meshar a coluna toda de uma vez eram
/// ~131 k células num único build inline, e o `plugin.rs` constrói meshes
/// inline de propósito (o crate a montante perdia chunks para sempre num bug
/// de orphan-task). 32³ constrói bem dentro de um frame e dá streaming e cull
/// em **Y** — que as grutas precisam de qualquer maneira.
pub const VOXEL_CHUNK_CELLS: usize = 32;

/// Neutral wall-space value, matching `CliffMask::WALL_NEUTRAL` (128/255).
const WALL_NEUTRAL: f32 = 128.0 / 255.0;

/// Faces verticais de uma caixa, em `[-X, +X, -Z, +Z]` — a ordem que o
/// `VoxelBoxSpec` usa.
pub const LATERAL_FACES: [TransitionSide; 4] = [
    TransitionSide::LowX,
    TransitionSide::HighX,
    TransitionSide::LowZ,
    TransitionSide::HighZ,
];

/// Converte a máscara `[-X, +X, -Z, +Z]` do `VoxelBoxSpec` no conjunto que o
/// transvoxel consome.
pub fn transition_sides(mask: [bool; 4]) -> TransitionSides {
    let mut sides = TransitionSide::none();
    for (on, side) in mask.iter().zip(LATERAL_FACES) {
        if *on {
            sides |= side;
        }
    }
    sides
}

/// Build parameters for one voxel chunk.
#[derive(Debug, Clone)]
pub struct VoxelChunkParams {
    /// World position of the chunk's minimum corner.
    pub origin: Vec3,
    /// Cells per axis.
    pub cells: usize,
    /// Edge length of one cell, in meters.
    pub voxel_size: f32,
    /// World-space UV tile size, matching the heightfield path so ground
    /// textures line up across the flat/volumetric boundary.
    pub texture_tile_size: f32,
    /// Height/slope tint, and the world's peak height it is measured against.
    pub tint: TintParams,
    pub max_height: f32,
    /// Whether this chunk renders with the layer blend material (`layers`
    /// worlds) rather than the stock `StandardMaterial`.
    ///
    /// It decides what the R channel MEANS. With the layer shader, R is wall
    /// space (data the fragment reads) and A the cliff factor. With the stock
    /// PBR material the vertex colour is multiplied as-is, so a wall-space R
    /// of 0.502 darkened every voxel chunk's red — a visible step at every
    /// chunk boundary.
    pub uses_layer_material: bool,
    /// Faces cujo vizinho está ao DOBRO da resolução desta caixa, em
    /// `[-X, +X, -Z, +Z]`. Só as laterais: todas as caixas de uma coluna
    /// partilham o LOD, portanto as faces em Y nunca transicionam.
    pub transitions: [bool; 4],
}

impl VoxelChunkParams {
    pub fn new(
        origin: Vec3,
        voxel_size: f32,
        texture_tile_size: f32,
        tint: TintParams,
        max_height: f32,
        uses_layer_material: bool,
    ) -> Self {
        Self {
            origin,
            cells: VOXEL_CHUNK_CELLS,
            voxel_size,
            texture_tile_size,
            tint,
            max_height,
            uses_layer_material,
            transitions: [false; 4],
        }
    }

    /// World-space extent of this chunk.
    pub fn extent(&self) -> f32 {
        self.cells as f32 * self.voxel_size
    }
}

/// [`MeshBuilder`] que escreve direto nos buffers do Viber.
///
/// O transvoxel entrega os dois `GridPoint` de uma aresta e o parâmetro de
/// interpolação; tudo o resto (posição relativa, normal do gradiente, UV
/// world-XZ, tint por altura/declive) é a mesma regra que o mesher anterior
/// aplicava, para o chão não mudar de cor com a troca de algoritmo.
struct ChunkMeshBuilder<'a> {
    params: &'a VoxelChunkParams,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl<'a> ChunkMeshBuilder<'a> {
    fn new(params: &'a VoxelChunkParams) -> Self {
        Self {
            params,
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn finish(self) -> Option<ChunkMeshData> {
        if self.positions.is_empty() || self.indices.is_empty() {
            return None;
        }
        Some(ChunkMeshData {
            positions: self.positions,
            normals: self.normals,
            uvs: self.uvs,
            colors: self.colors,
            indices: self.indices,
        })
    }
}

impl MeshBuilder<f32, f32> for ChunkMeshBuilder<'_> {
    fn add_vertex_between(
        &mut self,
        point_a: GridPoint<f32, f32>,
        point_b: GridPoint<f32, f32>,
        interpolate_toward_b: f32,
    ) -> VertexIndex {
        let t = interpolate_toward_b;
        let a = &point_a.position;
        let b = &point_b.position;
        let world = Vec3::new(
            a.x + t * (b.x - a.x),
            a.y + t * (b.y - a.y),
            a.z + t * (b.z - a.z),
        );

        // Normal do GRADIENTE do campo, não dos triângulos: é contínua nas
        // fronteiras, portanto duas caixas vizinhas iluminam igual sem
        // partilhar estado. O campo alimentado é `-density`, logo a normal
        // para fora é `-grad` (ver docs do módulo).
        let (gax, gay, gaz) = point_a.gradient;
        let (gbx, gby, gbz) = point_b.gradient;
        let grad = Vec3::new(
            gax + t * (gbx - gax),
            gay + t * (gby - gay),
            gaz + t * (gbz - gaz),
        );
        let normal = (-grad).try_normalize().unwrap_or(Vec3::Y);

        let local = world - self.params.origin;
        let tile = if self.params.texture_tile_size > 0.0 {
            self.params.texture_tile_size
        } else {
            1.0
        };

        let index = self.positions.len();
        self.positions.push(local.to_array());
        self.normals.push(normal.to_array());
        self.uvs
            .push(Vec2::new(world.x / tile, world.z / tile).to_array());
        // Mesmo tint de altura/declive do mesher do heightfield, para as duas
        // superfícies lerem como uma só. Com o blend de layers o R passa a ser
        // wall-space (dado que o fragment lê) e o A o fator de cliff, tal como
        // `build_chunk_mesh` faz.
        let mut color =
            tint_vertex_color(world.y, normal.y, self.params.max_height, &self.params.tint);
        if self.params.uses_layer_material {
            color[0] = WALL_NEUTRAL;
            color[3] = 1.0;
        }
        self.colors.push(color);
        VertexIndex(index)
    }

    fn add_triangle(&mut self, v1: VertexIndex, v2: VertexIndex, v3: VertexIndex) {
        // Nada é filtrado aqui, de propósito. Marching cubes emite SLIVERS por
        // construção (a isosuperfície a passar rente a um canto do lattice põe
        // dois vértices interpolados quase no mesmo ponto), e a tentação é
        // deitá-los fora — mas cada sliver partilha as suas arestas com
        // triângulos legítimos: removê-lo deixa arestas com UMA face só, e a
        // malha deixa de ser estanque. Uma gruta com arestas de fronteira é um
        // buraco para o collider e para o teste de watertight.
        //
        // Ficam. Têm área ~0 (não se veem), o `TriMesh` do collider apaga-os
        // com `DELETE_DEGENERATE_TRIANGLES` depois de soldar os vértices — que
        // é onde a remoção é segura — e [`sliver_area2_floor`] serve para os
        // testes de saúde os contarem.
        self.indices.push(v1.0 as u32);
        self.indices.push(v2.0 as u32);
        self.indices.push(v3.0 as u32);
    }
}

/// Campo com o lattice regular do bloco pré-amostrado.
///
/// `CacheNothing` é obrigatório (ver [`build_voxel_mesh`]), mas sem cache
/// nenhum o transvoxel reavalia cada canto de célula até oito vezes: uma
/// caixa 32³ passa de ~39 k para ~260 k avaliações do SDF, e meshar uma caixa
/// de LOD 0 subia de ~6 ms para ~28 ms — cinco vezes o orçamento de frame,
/// com o `BrushGrid` (Catmull-Rom monótono) a pagar a conta.
///
/// Isto pré-calcula o lattice `(cells+1)³` — exatamente as amostras que o
/// mesher anterior fazia — e serve tudo o que caia fora dele (as meias-células
/// das faces de transição, uma fração da superfície) direto do campo. O cache
/// é do bloco, não partilhado, portanto não tem o problema de indexação que
/// obriga a evitar o `CacheCentralBlockOnly`.
struct LatticeField<'a> {
    density: &'a dyn Fn(Vec3) -> f32,
    base: Vec3,
    cell: f32,
    cells: usize,
    /// `-density` no lattice, em ordem x-rápido → y → z.
    samples: Vec<f32>,
}

impl<'a> LatticeField<'a> {
    fn new(density: &'a dyn Fn(Vec3) -> f32, base: Vec3, cell: f32, cells: usize) -> Self {
        let pts = cells + 1;
        let mut samples = Vec::with_capacity(pts * pts * pts);
        for iz in 0..pts {
            for iy in 0..pts {
                for ix in 0..pts {
                    let p = base + Vec3::new(ix as f32 * cell, iy as f32 * cell, iz as f32 * cell);
                    samples.push(-(density)(p));
                }
            }
        }
        Self {
            density,
            base,
            cell,
            cells,
            samples,
        }
    }

    /// Índice do lattice para uma coordenada, ou `None` se cair entre pontos.
    fn lattice_index(&self, v: f32, base: f32) -> Option<usize> {
        let f = (v - base) / self.cell;
        let i = f.round();
        // Tolerância relativa à célula: as posições vêm de
        // `base + size * (index/subdivisions)`, com o erro de vírgula
        // flutuante dessa divisão.
        if (f - i).abs() > 1e-3 || i < 0.0 || i > self.cells as f32 {
            return None;
        }
        Some(i as usize)
    }
}

impl DataField<f32, f32> for LatticeField<'_> {
    fn get_data(&self, x: f32, y: f32, z: f32) -> f32 {
        if let (Some(ix), Some(iy), Some(iz)) = (
            self.lattice_index(x, self.base.x),
            self.lattice_index(y, self.base.y),
            self.lattice_index(z, self.base.z),
        ) {
            let pts = self.cells + 1;
            return self.samples[(iz * pts + iy) * pts + ix];
        }
        -(self.density)(Vec3::new(x, y, z))
    }
}

/// Piso de área² abaixo do qual um triângulo conta como sliver de marching
/// cubes — a métrica dos testes de saúde, não um filtro (ver `add_triangle`).
///
/// A área de um triângulo saudável escala com `voxel_size²`; a área² com
/// `voxel_size⁴`. O piso é 0,1 % da face de uma célula — a 1 m dá 1e-6 m⁴,
/// que é a mesma tolerância que o surface nets exigia a 100 %.
pub fn sliver_area2_floor(voxel_size: f32) -> f32 {
    let area = 1e-3 * voxel_size * voxel_size;
    area * area
}

/// Builds one voxel chunk mesh from a density function.
///
/// `density` must be negative inside solid. Returns `None` when the chunk
/// holds no surface — the common case, and the reason a volumetric column
/// costs only the boxes that actually straddle the ground.
pub fn build_voxel_mesh(
    density: &dyn Fn(Vec3) -> f32,
    params: &VoxelChunkParams,
) -> Option<ChunkMeshData> {
    if params.cells == 0 || !(params.voxel_size.is_finite() && params.voxel_size > 0.0) {
        return None;
    }
    let block = Block::new(
        [params.origin.x, params.origin.y, params.origin.z],
        params.extent(),
        params.cells,
    );
    // `-density`: o transvoxel considera dentro o que está ACIMA do threshold.
    // O `LatticeField` faz a inversão e serve o lattice regular de cache.
    let field = LatticeField::new(density, params.origin, params.voxel_size, params.cells);
    let builder = extract_from_field(
        &field,
        // NÃO trocar por `CacheCentralBlockOnly`. Esse cache é indexado à
        // resolução do bloco central, e as células de transição amostram o
        // campo a MEIA célula — leem a entrada errada e os vértices da face
        // caem a metade da altura certa. Numa caixa de 64 m com o chão a 40 m
        // o resultado era uma cortina de 20 m pendurada em cada face de
        // transição: um paredão a cortar o horizonte em toda a banda de LOD, e
        // o frame de 10 ms a passar a 25 ms de overdraw. Coberto por
        // `test_a_transition_face_keeps_a_flat_plane_flat`.
        FieldCaching::CacheNothing,
        block,
        transition_sides(params.transitions),
        0.0f32,
        ChunkMeshBuilder::new(params),
    );
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::mesh::TintParams;

    fn tint() -> TintParams {
        TintParams {
            color_low: [0.3, 0.5, 0.2, 1.0],
            color_mid: [0.5, 0.45, 0.3, 1.0],
            color_high: [0.6, 0.6, 0.6, 1.0],
            height_blend_strength: 1.0,
            slope_threshold: 0.6,
            slope_softness: 0.1,
            snow_height: 0.9,
            ..TintParams::default()
        }
    }

    fn params(origin: Vec3, cells: usize, voxel_size: f32) -> VoxelChunkParams {
        VoxelChunkParams {
            origin,
            cells,
            voxel_size,
            texture_tile_size: 8.0,
            tint: tint(),
            max_height: 64.0,
            uses_layer_material: false,
            transitions: [false; 4],
        }
    }

    /// Plano horizontal a y = 16: negativo (sólido) abaixo.
    fn flat(y: f32) -> impl Fn(Vec3) -> f32 {
        move |p: Vec3| p.y - y
    }

    #[test]
    fn test_a_box_with_no_surface_meshes_to_nothing() {
        let p = params(Vec3::new(0.0, 100.0, 0.0), 8, 1.0);
        assert!(build_voxel_mesh(&flat(16.0), &p).is_none(), "só céu");
        let p = params(Vec3::new(0.0, -100.0, 0.0), 8, 1.0);
        assert!(build_voxel_mesh(&flat(16.0), &p).is_none(), "só rocha");
    }

    #[test]
    fn test_a_flat_surface_lands_on_the_plane_with_up_normals() {
        let p = params(Vec3::new(0.0, 8.0, 0.0), 16, 1.0);
        let data = build_voxel_mesh(&flat(16.0), &p).expect("o plano atravessa a caixa");
        assert!(!data.positions.is_empty() && !data.indices.is_empty());
        assert_eq!(data.positions.len(), data.normals.len());
        assert_eq!(data.positions.len(), data.uvs.len());
        assert_eq!(data.positions.len(), data.colors.len());
        assert_eq!(data.indices.len() % 3, 0);
        for pos in &data.positions {
            // Posições são relativas à origem da caixa: y local 8 = y mundo 16.
            let world_y = p.origin.y + pos[1];
            assert!(
                (world_y - 16.0).abs() < 0.05,
                "vértice fora do plano: y={world_y}"
            );
        }
        for n in &data.normals {
            assert!(
                n[1] > 0.99,
                "normal do chão tem de apontar para cima: {n:?}"
            );
        }
        for i in &data.indices {
            assert!(
                (*i as usize) < data.positions.len(),
                "índice fora do buffer"
            );
        }
    }

    #[test]
    fn test_a_transition_face_changes_the_mesh() {
        // A face de transição encolhe a fiada exterior e remalha-a a meia
        // escala: tem de produzir geometria DIFERENTE da caixa sem transição,
        // senão o conjunto de lados não está a chegar ao algoritmo.
        let plain = params(Vec3::new(0.0, 8.0, 0.0), 16, 1.0);
        let mut with_side = plain.clone();
        with_side.transitions = [true, false, false, false];
        let a = build_voxel_mesh(&flat(16.0), &plain).expect("mesh base");
        let b = build_voxel_mesh(&flat(16.0), &with_side).expect("mesh com transição");
        assert_ne!(
            a.positions.len(),
            b.positions.len(),
            "LowX ligado não mudou nada — as transições não estão a ser passadas"
        );
    }

    /// Um plano horizontal continua um plano horizontal com transições
    /// ligadas — o teste que apanha o cache de campo errado.
    ///
    /// Com `FieldCaching::CacheCentralBlockOnly` as células de transição liam
    /// densidades da entrada errada do cache e punham os vértices da face a
    /// METADE da altura local certa: uma cortina vertical pendurada em cada
    /// face de transição, visível em jogo como um paredão a cortar o horizonte
    /// ao longo de toda a banda de LOD.
    #[test]
    fn test_a_transition_face_keeps_a_flat_plane_flat() {
        // Inclui a geometria real do LOD 1 do simple-rpg (64 m, 32 células).
        let cases = [
            (Vec3::new(0.0, 8.0, 0.0), 16usize, 1.0f32, 16.0f32),
            (Vec3::new(0.0, 8.0, 0.0), 8, 2.0, 16.0),
            (Vec3::new(0.0, 8.0, 0.0), 16, 1.0, 20.0),
            (Vec3::new(0.0, 0.0, 0.0), 32, 2.0, 40.0),
        ];
        for (origin, cells, voxel_size, plane) in cases {
            for transitions in [
                [false; 4],
                [true, false, false, false],
                [true, true, true, true],
            ] {
                let mut p = params(origin, cells, voxel_size);
                p.transitions = transitions;
                let d = build_voxel_mesh(&flat(plane), &p).expect("o plano atravessa a caixa");
                let expected = plane - origin.y;
                for pos in &d.positions {
                    assert!(
                        (pos[1] - expected).abs() < 0.05,
                        "cells={cells} vs={voxel_size} tr={transitions:?}: vértice a y={} \
                         em vez de {expected} — a face de transição está a cair",
                        pos[1]
                    );
                }
            }
        }
    }

    #[test]
    fn test_transition_sides_maps_the_face_mask_in_order() {
        assert_eq!(transition_sides([false; 4]), TransitionSide::none());
        let sides = transition_sides([true, false, true, false]);
        assert!(sides.contains(TransitionSide::LowX));
        assert!(sides.contains(TransitionSide::LowZ));
        assert!(!sides.contains(TransitionSide::HighX));
        assert!(!sides.contains(TransitionSide::HighZ));
        // Em Y nunca — as caixas de uma coluna partilham o LOD.
        assert!(!sides.contains(TransitionSide::LowY));
        assert!(!sides.contains(TransitionSide::HighY));
    }
}
