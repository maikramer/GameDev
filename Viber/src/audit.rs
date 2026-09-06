//! Auditoria de assets referenciados pelo XML — corre headless no
//! `viber analyze`: ficheiros ausentes, GLBs com compressão não suportada
//! (Draco/Basis; meshopt é expandido pelo reader da engine), texturas com
//! formato desconhecido ou magia inválida, colliders ausentes em modelos
//! glTF, e refs a scripts/estilos/BGM que não existem em disco.
//!
//! Não abre a engine: lê bytes de cabeçalho (GLB magic + chunk JSON, PNG/JPEG
//! magic) — barato e suficiente para apanhar os casos reais (GLB truncado,
//! HTML guardado como .png, export Draco do DCC, typo no caminho).

use std::path::{Path, PathBuf};

use crate::recipes::{EntityKind, EntitySpec, ParsedWorld};

/// Severidade de um achado: `Missing` vira ERRO com `analyze --strict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Missing,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct AuditIssue {
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct AuditReport {
    /// Referências a ficheiros recolhidas (incl. repetidas).
    pub references: usize,
    pub issues: Vec<AuditIssue>,
    /// Modelos glTF SEM collider (passam através) — nome ou url.
    pub colliderless: Vec<String>,
}

impl AuditReport {
    pub fn missing_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Missing)
            .count()
    }
}

/// Tipo de ficheiro referenciado (dita o sniffing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Glb,
    Texture,
    Heightmap,
    Audio,
    Script,
    Stylesheet,
}

struct AssetRef {
    kind: RefKind,
    /// Caminho absoluto no disco.
    path: PathBuf,
    /// Como foi referenciado (para a mensagem): `<GltfScene name="x">`.
    context: String,
}

/// Audita o mundo: recolhe refs e verifica cada uma. `asset_root` é a pasta
/// que CONTÉM `assets/` (igual ao runtime); `world_dir` resolve
/// scripts/estilos relativos.
/// Paths de áudio carregados por HARDCODE na engine: os clips do registry
/// Lua (`luau::SFX_NAME_REGISTRY` — uma linha por variante, aliases caem no
/// dedup) mais os loops de ambiente (`ambient::AMBIENT_LOOP_FILES`). O audit
/// valida-os a todos porque nunca aparecem no XML — sem isto o `analyze`
/// dizia "ok" com o jogo mudo (foi o caso dos loops de água).
pub(crate) fn engine_audio_files() -> Vec<&'static str> {
    let mut files: Vec<&'static str> = crate::luau::SFX_NAME_REGISTRY
        .iter()
        .map(|(_, clip)| clip.file())
        .collect();
    files.extend(crate::ambient::AMBIENT_LOOP_FILES.iter().copied());
    files.sort_unstable();
    files.dedup();
    files
}

pub fn audit(world: &ParsedWorld, world_dir: &Path, asset_root: &Path) -> AuditReport {
    let mut report = AuditReport::default();
    let mut refs: Vec<AssetRef> = Vec::new();
    collect_entities(
        &world.entities,
        world_dir,
        asset_root,
        false,
        &mut refs,
        &mut report,
    );

    // SFX carregados por path HARDCODED na engine — ver
    // [`engine_audio_files`].
    for file in engine_audio_files() {
        refs.push(AssetRef {
            kind: RefKind::Audio,
            path: asset_root.join(file.trim_start_matches('/')),
            context: "sfx engine".to_string(),
        });
    }

    report.colliderless.sort();
    report.colliderless.dedup();

    // Dedup por (kind, path) mantendo o 1.º contexto + contagem.
    report.references = refs.len();
    let mut unique: Vec<(RefKind, PathBuf, String, usize)> = Vec::new();
    for r in refs {
        if let Some(entry) = unique
            .iter_mut()
            .find(|(kind, path, ..)| *kind == r.kind && *path == r.path)
        {
            entry.3 += 1;
        } else {
            unique.push((r.kind, r.path, r.context, 1));
        }
    }

    for (kind, path, context, count) in unique {
        let uses = if count > 1 {
            format!(" (+{count} refs iguais)")
        } else {
            String::new()
        };
        if !path.is_file() {
            report.issues.push(AuditIssue {
                severity: Severity::Missing,
                message: format!("ausente: {} (usado por {context}{uses})", path.display()),
            });
            continue;
        }
        match kind {
            RefKind::Glb => audit_glb(&path, &context, &mut report.issues),
            RefKind::Texture => audit_texture(&path, &context, &mut report.issues),
            RefKind::Heightmap => {
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"))
                {
                    check_magic(
                        &path,
                        &context,
                        b"\x89PNG\r\n\x1a\n",
                        "PNG",
                        &mut report.issues,
                    );
                }
            }
            RefKind::Audio => {
                if !path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("ogg"))
                {
                    report.issues.push(AuditIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "formato de áudio pouco usual: {} (a engine usa .ogg) — {context}",
                            path.display()
                        ),
                    });
                }
            }
            RefKind::Script | RefKind::Stylesheet => {}
        }
    }
    report
}

/// Recursão pela árvore de entidades recolhendo refs + GLBs sem collider.
fn collect_entities(
    entities: &[EntitySpec],
    world_dir: &Path,
    asset_root: &Path,
    inherited_collider: bool,
    refs: &mut Vec<AssetRef>,
    report: &mut AuditReport,
) {
    for entity in entities {
        let label = entity
            .name
            .clone()
            .or_else(|| entity.tag.clone())
            .unwrap_or_else(|| "sem nome".into());

        let collider_covered = inherited_collider || !entity.physics.is_empty();
        if let Some(script) = &entity.script {
            refs.push(AssetRef {
                kind: RefKind::Script,
                path: world_dir.join("scripts").join(script),
                context: format!("<script=\"{script}\"> {label}"),
            });
        }
        match &entity.kind {
            EntityKind::GltfScene { url } | EntityKind::PlayerGltf { url } => {
                // O herói tem character controller próprio (colisão via
                // Rapier, não via attr); o resto herda collider do ancestral.
                if !collider_covered && matches!(entity.kind, EntityKind::GltfScene { .. }) {
                    report.colliderless.push(url.clone());
                }
                refs.push(AssetRef {
                    kind: RefKind::Glb,
                    path: asset_root.join(url.trim_start_matches('/')),
                    context: format!("<{url}> {label}"),
                });
            }
            EntityKind::Primitive { material, .. } => {
                if let Some(texture) = &material.texture {
                    refs.push(AssetRef {
                        kind: RefKind::Texture,
                        path: asset_root.join(texture.trim_start_matches('/')),
                        context: format!("<texture=\"{texture}\"> {label}"),
                    });
                }
            }
            EntityKind::Terrain { spec } => {
                if let Some(heightmap) = &spec.heightmap {
                    refs.push(AssetRef {
                        kind: RefKind::Heightmap,
                        path: asset_root.join(heightmap.trim_start_matches('/')),
                        context: format!("<heightmap=\"{heightmap}\">"),
                    });
                }
                if let Some(texture) = &spec.texture {
                    refs.push(AssetRef {
                        kind: RefKind::Texture,
                        path: asset_root.join(texture.trim_start_matches('/')),
                        context: format!("<terrain texture=\"{texture}\">"),
                    });
                }
                for layer in &spec.layers {
                    // Lista canónica com buracos (""): slot sem alias
                    // autoral — nada a auditar.
                    if layer.is_empty() {
                        continue;
                    }
                    // Alias do pool (`grass`) ou caminho cru de textura — a
                    // mesma resolução do bootstrap (`splat::pool_albedo`).
                    let path =
                        crate::terrain::splat::pool_albedo(layer).unwrap_or_else(|| layer.clone());
                    refs.push(AssetRef {
                        kind: RefKind::Texture,
                        path: asset_root.join(path.trim_start_matches('/')),
                        context: format!("<terrain layers=\"…{layer}…\">"),
                    });
                }
            }
            EntityKind::Road { spec } => {
                push_ribbon_texture(asset_root, &spec.texture, &label, refs)
            }
            EntityKind::RoadNetwork { spec } => {
                push_ribbon_texture(asset_root, &spec.texture, &label, refs)
            }
            EntityKind::GroundDecal { spec } => {
                if let Some(texture) = &spec.texture {
                    refs.push(AssetRef {
                        kind: RefKind::Texture,
                        path: asset_root.join(texture.trim_start_matches('/')),
                        context: format!("<decal texture=\"{texture}\"> {label}"),
                    });
                }
            }
            EntityKind::Vegetation { spec } => {
                for mesh in &spec.meshes {
                    refs.push(AssetRef {
                        kind: RefKind::Glb,
                        path: asset_root.join(mesh.trim_start_matches('/')),
                        context: format!("<vegetation mesh=\"{mesh}\"> {label}"),
                    });
                }
            }
            EntityKind::MusicLayer { layer, .. } => {
                // Convenção do runtime (spawn.rs): assets/audio/bgm/{layer}.ogg.
                let path = asset_root.join(format!("assets/audio/bgm/{layer}.ogg"));
                refs.push(AssetRef {
                    kind: RefKind::Audio,
                    path,
                    context: format!("<MusicLayer layer=\"{layer}\">"),
                });
            }
            EntityKind::UiStyle { source } => {
                // O parser antecede '@' ao `src` (`@ui/hud.css`).
                let file = source.trim_start_matches('@');
                refs.push(AssetRef {
                    kind: RefKind::Stylesheet,
                    path: world_dir.join(file),
                    context: format!("<UiStyle src=\"{file}\">"),
                });
            }
            _ => {}
        }
        collect_entities(
            &entity.children,
            world_dir,
            asset_root,
            collider_covered,
            refs,
            report,
        );
    }
}

fn push_ribbon_texture(
    asset_root: &Path,
    texture: &Option<String>,
    label: &str,
    refs: &mut Vec<AssetRef>,
) {
    if let Some(texture) = texture {
        refs.push(AssetRef {
            kind: RefKind::Texture,
            path: asset_root.join(texture.trim_start_matches('/')),
            context: format!("<ribbon texture=\"{texture}\"> {label}"),
        });
    }
}

// ---------------------------------------------------------------- GLB

/// GLB: magic + chunk JSON → extensões usadas. Draco/Basis não são
/// suportados pela engine; meshopt é expandido pelo asset reader.
fn audit_glb(path: &Path, context: &str, issues: &mut Vec<AuditIssue>) {
    const GLB_MAGIC: &[u8; 4] = b"glTF";
    let Ok(bytes) = std::fs::read(path) else {
        return; // ausência já foi reportada
    };
    if bytes.len() < 12 || &bytes[0..4] != GLB_MAGIC {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "glb inválido: {} não começa com a magia \"glTF\" (truncado ou não é GLB?) — {context}",
                path.display()
            ),
        });
        return;
    }
    // Cabeçalho truncado entre a magia e o chunk JSON (12–19 bytes): ler o
    // chunk len ou fatiar [20..end] era index out of bounds — o analyze
    // panicava em vez de falhar limpo.
    if bytes.len() < 20 {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "glb inválido: {} truncado antes do chunk JSON — {context}",
                path.display()
            ),
        });
        return;
    }
    let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let end = (20 + json_len).min(bytes.len());
    let Ok(json) = std::str::from_utf8(&bytes[20..end]) else {
        return;
    };
    // Draco/Basis: a engine não lê — falha o load com warn em runtime.
    for (extension, hint) in [
        (
            "KHR_draco_mesh_compression",
            "a engine (Bevy 0.19) não lê Draco",
        ),
        (
            "KHR_texture_basisu",
            "a engine (Bevy 0.19) não lê Basis/KTX2",
        ),
    ] {
        if json.contains(extension) {
            issues.push(AuditIssue {
                severity: Severity::Warning,
                message: format!(
                    "compressão não suportada: {extension} em {} ({hint}) — {context}",
                    path.display()
                ),
            });
        }
    }
}

// ---------------------------------------------------------------- texturas

const TEXTURE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "ktx2", "hdr", "tga"];

fn audit_texture(path: &Path, context: &str, issues: &mut Vec<AuditIssue>) {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let Some(extension) = extension else {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "textura sem extensão: {} (esperado png/jpg/webp/ktx2/hdr) — {context}",
                path.display()
            ),
        });
        return;
    };
    if !TEXTURE_EXTENSIONS.contains(&extension.as_str()) {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "formato de textura não suportado: .{extension} em {} (esperado png/jpg/webp/ktx2/hdr/tga) — {context}",
                path.display()
            ),
        });
        return;
    }
    match extension.as_str() {
        "png" => check_magic(path, context, b"\x89PNG\r\n\x1a\n", "PNG", issues),
        "jpg" | "jpeg" => check_magic(path, context, b"\xff\xd8\xff", "JPEG", issues),
        "ktx2" => audit_ktx2(path, context, issues),
        "webp" => check_webp(path, context, issues),
        "hdr" => check_magic(path, context, b"#?", "HDR", issues),
        // .tga não tem magia fiável (o campo de ID é opcional e a assinatura
        // opcional vive no FIM do ficheiro) — a extensão basta.
        "tga" => {}
        _ => {}
    }
}

/// Lê só os primeiros `n` bytes (cabeçalho) — o audit não abre texturas
/// inteiras.
fn read_header(path: &Path, n: u64) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(n).read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

/// KTX2: magia de 12 bytes + cabeçalho de 80; `vkFormat` (u32 LE @12) e
/// `supercompressionScheme` (u32 LE @44; 0 none / 1 BasisLZ / 2 Zstandard —
/// discriminantes conferidos na crate `ktx2` 0.5).
const KTX2_MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

/// O `.ktx2` merece sniffing próprio: ETC1S/BasisLZ é FATAL na engine (o
/// Bevy 0.19 não descomprime BasisLZ — regra "nunca etc1s") e entrava como
/// falso negativo, ao contrário do análogo GLB (Draco/Basis verificados).
fn audit_ktx2(path: &Path, context: &str, issues: &mut Vec<AuditIssue>) {
    let Some(bytes) = read_header(path, 48) else {
        return;
    };
    if bytes.len() < 12 || bytes[..12] != KTX2_MAGIC {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "ktx2 inválido: {} não tem a magia KTX2 (truncado ou não é KTX2?) — {context}",
                path.display()
            ),
        });
        return;
    }
    let vk_format = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    // O scheme só se lê com os 48 bytes do cabeçalho presentes.
    let scheme = (bytes.len() >= 48)
        .then(|| u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]));
    let problem = if vk_format == 0 {
        // VK_FORMAT_UNDEFINED: Basis Universal — o formato real decide-se na
        // transcodificação; é o caminho do etc1s/BasisLZ.
        Some(
            "Basis Universal (vkFormat 0 — ETC1S/BasisLZ) — a engine (Bevy 0.19) não descomprime BasisLZ; reexporta em UASTC (`text3d finish`)"
                .to_string(),
        )
    } else if (147..=156).contains(&vk_format) {
        // Família ETC2/EAC crua: VK_FORMAT_ETC2_R8G8B8_UNORM_BLOCK
        // (147 = 0x93) até VK_FORMAT_EAC_R11G11_SNORM_BLOCK (156 = 0x9C) —
        // discriminantes conferidos na crate `ktx2` 0.5, enum `Format`.
        // UASTC costuma declarar VK_FORMAT_R8G8B8A8_* (37) e passa.
        Some(format!(
            "ETC2/EAC cru (vkFormat {vk_format}) — desktop não amostra ETC2; reexporta em UASTC (`text3d finish`)"
        ))
    } else if scheme == Some(1) {
        Some(
            "supercompressão BasisLZ (scheme 1) — a engine (Bevy 0.19) não descomprime BasisLZ; reexporta em UASTC (`text3d finish`)"
                .to_string(),
        )
    } else {
        None
    };
    if let Some(problem) = problem {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "ktx2 não suportado: {} usa {problem} — {context}",
                path.display()
            ),
        });
    }
}

/// WebP é um contentor RIFF: "RIFF" + tamanho (4 bytes) + "WEBP" @8.
fn check_webp(path: &Path, context: &str, issues: &mut Vec<AuditIssue>) {
    let Some(bytes) = read_header(path, 12) else {
        return;
    };
    if bytes.len() < 12 || !bytes.starts_with(b"RIFF") || !bytes[8..12].starts_with(b"WEBP") {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "webp inválido: {} não tem a magia RIFF….WEBP (corrompido ou PNG/JPEG guardado com a extensão errada?) — {context}",
                path.display()
            ),
        });
    }
}

fn check_magic(
    path: &Path,
    context: &str,
    magic: &[u8],
    format: &str,
    issues: &mut Vec<AuditIssue>,
) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    if !bytes.starts_with(magic) {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            message: format!(
                "{format} inválido: {} não tem a magia {format} (corrompido ou HTML/JSON guardado com a extensão errada?) — {context}",
                path.display()
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{ColliderShape, PhysicsSpec};
    use crate::recipes::{EntitySpec, MaterialSpec, TransformSpec};

    fn entity(name: &str, kind: EntityKind, collider: ColliderShape) -> EntitySpec {
        EntitySpec {
            name: Some(name.into()),
            tag: None,
            script: None,
            destructible: None,
            transform: TransformSpec::default(),
            physics: PhysicsSpec {
                collider,
                body: Default::default(),
                mass: None,
                gravity_scale: None,
            },
            kind,
            children: vec![],
        }
    }

    /// GLB mínimo válido: header + chunk JSON com a extensão dada.
    fn glb_bytes(extension: Option<&str>) -> Vec<u8> {
        let json = match extension {
            Some(ext) => format!(r#"{{"extensionsUsed":["{ext}"]}}"#),
            None => "{}".into(),
        };
        let mut out = Vec::from(&b"glTF"[..]);
        out.extend_from_slice(&2u32.to_le_bytes()); // version
        out.extend_from_slice(&(12 + json.len() as u32).to_le_bytes()); // total
        out.extend_from_slice(&(json.len() as u32).to_le_bytes()); // chunk len
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(json.as_bytes());
        out
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let world_dir = dir.path().join("mundo");
        let asset_root = world_dir.clone();
        std::fs::create_dir_all(asset_root.join("assets/meshes")).unwrap();
        std::fs::create_dir_all(asset_root.join("assets/textures")).unwrap();
        std::fs::create_dir_all(asset_root.join("scripts")).unwrap();
        // O fixture cria-os vazios: o que se testa aqui é a recolha de refs
        // e o reporte dos assets do MUNDO, não a existência dos clips.
        for rel in engine_audio_files() {
            let path = asset_root.join(rel.trim_start_matches('/'));
            std::fs::create_dir_all(path.parent().expect("parent")).unwrap();
            std::fs::File::create(&path).unwrap();
        }
        (dir, world_dir, asset_root)
    }

    #[test]
    fn test_missing_glb_is_reported() {
        let (_dir, world_dir, asset_root) = setup();
        let world = ParsedWorld {
            entities: vec![entity(
                "heroi",
                EntityKind::PlayerGltf {
                    url: "/assets/meshes/fantasma.glb".into(),
                },
                Default::default(),
            )],
            ..Default::default()
        };
        let report = audit(&world, &world_dir, &asset_root);
        // 1 ref do mundo + os SFX da engine (fixture cria-os, nenhum falha).
        assert_eq!(report.references, 1 + engine_audio_files().len());
        assert_eq!(report.missing_count(), 1);
        assert!(
            report.issues[0].message.contains("ausente")
                && report.issues[0].message.contains("fantasma.glb"),
            "{:?}",
            report.issues
        );
        // PlayerGLTF NUNCA entra na lista de colliderless: o herói tem
        // character controller próprio (colisão via Rapier no spawn).
        assert!(report.colliderless.is_empty());
    }

    #[test]
    fn test_colliderless_and_inherited_collider() {
        let (_dir, world_dir, asset_root) = setup();
        std::fs::write(asset_root.join("assets/meshes/a.glb"), glb_bytes(None)).unwrap();
        std::fs::write(asset_root.join("assets/meshes/b.glb"), glb_bytes(None)).unwrap();
        let glb = |url: &str| EntityKind::GltfScene { url: url.into() };
        // "cobre": Group com collider → filhos GltfScene NÃO são colliderless.
        let mut group = entity("muro", EntityKind::Group, ColliderShape::Auto);
        group.children = vec![entity(
            "painel",
            glb("/assets/meshes/a.glb"),
            Default::default(),
        )];
        let world = ParsedWorld {
            entities: vec![
                group,
                entity("nu", glb("/assets/meshes/b.glb"), Default::default()),
            ],
            ..Default::default()
        };
        let report = audit(&world, &world_dir, &asset_root);
        assert_eq!(
            report.colliderless,
            vec!["/assets/meshes/b.glb".to_string()]
        );
    }

    #[test]
    fn test_draco_is_flagged_and_meshopt_is_not() {
        let (_dir, world_dir, asset_root) = setup();
        std::fs::write(
            asset_root.join("assets/meshes/draco.glb"),
            glb_bytes(Some("KHR_draco_mesh_compression")),
        )
        .unwrap();
        std::fs::write(
            asset_root.join("assets/meshes/meshopt.glb"),
            glb_bytes(Some("EXT_meshopt_compression")),
        )
        .unwrap();
        let glb_scene = |url: &str, name: &str| {
            entity(
                name,
                EntityKind::GltfScene { url: url.into() },
                ColliderShape::Box {
                    size: Default::default(),
                    offset: Default::default(),
                },
            )
        };
        let world = ParsedWorld {
            entities: vec![
                glb_scene("/assets/meshes/draco.glb", "d"),
                glb_scene("/assets/meshes/meshopt.glb", "m"),
            ],
            ..Default::default()
        };
        let report = audit(&world, &world_dir, &asset_root);
        assert_eq!(report.missing_count(), 0);
        assert_eq!(report.issues.len(), 1, "só o Draco: {:?}", report.issues);
        assert!(report.issues[0].message.contains("Draco"));
        assert!(report.colliderless.is_empty(), "com collider explícito");
    }

    #[test]
    fn test_invalid_glb_magic_and_texture_checks() {
        let (_dir, world_dir, asset_root) = setup();
        std::fs::write(
            asset_root.join("assets/meshes/falso.glb"),
            b"<html>404</html>",
        )
        .unwrap();
        std::fs::write(
            asset_root.join("assets/textures/ok.png"),
            [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0],
        )
        .unwrap();
        std::fs::write(
            asset_root.join("assets/textures/falso.png"),
            b"{\"json\": true}",
        )
        .unwrap();
        std::fs::write(asset_root.join("assets/textures/vector.svg"), b"<svg/>").unwrap();
        let with_texture = |name: &str, tex: &str, _file: &str| {
            entity(
                name,
                EntityKind::Primitive {
                    shape: crate::recipes::Shape::Cuboid {
                        half_size: [0.5; 3],
                    },
                    material: MaterialSpec {
                        texture: Some(tex.into()),
                        ..Default::default()
                    },
                },
                ColliderShape::Auto,
            )
        };
        let world = ParsedWorld {
            entities: vec![
                with_texture("a", "/assets/textures/falso.png", "x"),
                with_texture("b", "/assets/textures/vector.svg", "y"),
            ],
            ..Default::default()
        };
        // Referenciar também o glb falso e o png ok (via GltfScene + decal).
        let mut world = world;
        world.entities.push(entity(
            "c",
            EntityKind::GltfScene {
                url: "/assets/meshes/falso.glb".into(),
            },
            ColliderShape::Auto,
        ));
        // O png ok existe mas não é referenciado — acrescentar 2.ª textura ok
        // via 3.ª entidade para provar que NÃO gera issue.
        world
            .entities
            .push(with_texture("d", "/assets/textures/ok.png", "z"));
        let report = audit(&world, &world_dir, &asset_root);
        assert_eq!(report.missing_count(), 0);
        assert_eq!(report.issues.len(), 3, "{:?}", report.issues);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.message.contains("glb inválido"))
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.message.contains("PNG inválido"))
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.message.contains("formato de textura não suportado: .svg"))
        );
    }

    #[test]
    fn test_scripts_styles_and_music() {
        let (_dir, world_dir, asset_root) = setup();
        std::fs::write(
            world_dir.join("scripts/lobo.lua"),
            "function on_update(dt) end",
        )
        .unwrap();
        std::fs::create_dir_all(asset_root.join("assets/audio/bgm")).unwrap();
        std::fs::write(asset_root.join("assets/audio/bgm/explore.ogg"), b"ogg").unwrap();
        let mut lobo = entity(
            "lobo",
            EntityKind::GltfScene {
                url: "/assets/meshes/lobo.glb".into(),
            },
            ColliderShape::Auto,
        );
        lobo.script = Some("lobo.lua".into());
        let world = ParsedWorld {
            entities: vec![
                lobo,
                entity(
                    "fantasma",
                    EntityKind::GltfScene {
                        url: "/assets/meshes/fantasma.glb".into(),
                    },
                    ColliderShape::Auto,
                ),
                entity(
                    "musica",
                    EntityKind::MusicLayer {
                        layer: "explore".into(),
                        base_volume: 0.2,
                    },
                    Default::default(),
                ),
                entity(
                    "musica2",
                    EntityKind::MusicLayer {
                        layer: "inexistente".into(),
                        base_volume: 0.2,
                    },
                    Default::default(),
                ),
            ],
            ..Default::default()
        };
        let report = audit(&world, &world_dir, &asset_root);
        // Ausentes: lobo.glb (não criado), fantasma.glb, bgm/inexistente.ogg.
        assert_eq!(report.missing_count(), 3, "{:?}", report.issues);
        // Script existente não gera issue; audio ok não gera.
        assert!(
            report
                .issues
                .iter()
                .all(|i| i.severity == Severity::Missing)
        );
    }

    fn write_bytes(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("escrever fixture");
        path
    }

    /// Cabeçalho KTX2 mínimo (12 magia + 32 campos + scheme @44 = 48 bytes).
    fn ktx2_header(vk_format: u32, scheme: u32) -> Vec<u8> {
        let mut bytes = KTX2_MAGIC.to_vec();
        bytes.extend_from_slice(&vk_format.to_le_bytes()); // vkFormat @12
        bytes.extend_from_slice(&1u32.to_le_bytes()); // typeSize
        bytes.extend_from_slice(&256u32.to_le_bytes()); // pixelWidth
        bytes.extend_from_slice(&256u32.to_le_bytes()); // pixelHeight
        bytes.extend_from_slice(&1u32.to_le_bytes()); // pixelDepth
        bytes.extend_from_slice(&1u32.to_le_bytes()); // layerCount
        bytes.extend_from_slice(&1u32.to_le_bytes()); // faceCount
        bytes.extend_from_slice(&1u32.to_le_bytes()); // levelCount
        bytes.extend_from_slice(&scheme.to_le_bytes()); // supercompressionScheme @44
        bytes
    }

    #[test]
    fn test_ktx2_etc1s_and_etc2_flagged_uastc_passes() {
        let dir = tempfile::tempdir().expect("tmpdir");
        // ETC1S/BasisLZ: vkFormat 0 (UNDEFINED — Basis Universal) → warn.
        let mut issues = Vec::new();
        audit_texture(
            &write_bytes(&dir, "etc1s.ktx2", &ktx2_header(0, 1)),
            "teste",
            &mut issues,
        );
        assert_eq!(issues.len(), 1, "etc1s tem de ser apanhado: {issues:?}");
        // ETC2 cru (147 = VK_FORMAT_ETC2_R8G8B8_UNORM_BLOCK) → warn.
        let mut issues = Vec::new();
        audit_texture(
            &write_bytes(&dir, "etc2.ktx2", &ktx2_header(147, 0)),
            "teste",
            &mut issues,
        );
        assert_eq!(issues.len(), 1, "ETC2 cru tem de ser apanhado: {issues:?}");
        // UASTC (37 = VK_FORMAT_R8G8B8A8_UNORM, scheme Zstandard) → limpo.
        let mut issues = Vec::new();
        audit_texture(
            &write_bytes(&dir, "uastc.ktx2", &ktx2_header(37, 2)),
            "teste",
            &mut issues,
        );
        assert!(issues.is_empty(), "UASTC passa limpo: {issues:?}");
        // Magia errada (PNG guardado como .ktx2) → warn.
        let mut issues = Vec::new();
        audit_texture(
            &write_bytes(&dir, "falso.ktx2", b"\x89PNG\r\n\x1a\n"),
            "teste",
            &mut issues,
        );
        assert_eq!(issues.len(), 1, "magia errada tem de ser apanhada");
    }

    #[test]
    fn test_webp_and_hdr_magic() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let mut webp = b"RIFF\x24\x00\x00\x00".to_vec();
        webp.extend_from_slice(b"WEBP");
        let mut issues = Vec::new();
        audit_texture(&write_bytes(&dir, "ok.webp", &webp), "teste", &mut issues);
        assert!(issues.is_empty(), "webp válido passa: {issues:?}");
        let mut issues = Vec::new();
        audit_texture(
            &write_bytes(&dir, "falso.webp", b"\x89PNG\r\n\x1a\n"),
            "teste",
            &mut issues,
        );
        assert_eq!(issues.len(), 1, "webp sem RIFF/WEBP tem de ser apanhado");
        let mut issues = Vec::new();
        audit_texture(
            &write_bytes(&dir, "ok.hdr", b"#?RADIANCE\n\n-Y 4 3\n"),
            "teste",
            &mut issues,
        );
        assert!(issues.is_empty(), "hdr válido passa: {issues:?}");
    }
}
