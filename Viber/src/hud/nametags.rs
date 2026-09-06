//! World-anchored NPC name tags: a pooled set of pills reassigned every
//! frame to the nearest NPCs, projected through the main camera.
//!
//! O vocabulário é o do minimapa: NPC com quest **disponível ou pronta**
//! ganha o "!" dourado; hostis falam vermelho; e a pílula esbate-se com a
//! distância — cheia até 45 m, desaparecida nos 60 m (nada de etiquetas
//! flutuando sobre o mundo inteiro).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::ai::EnemyCreature;
use crate::player::Player;
use crate::quests::QuestStatus;
use crate::recipes::spawn::DialogueNpc;

/// A world-anchored NPC name tag pill from the pooled set.
#[derive(Component)]
pub struct NameTag;

/// O conteúdo da pílula (fundo, borda, texto) — reestilizado por frame.
#[derive(Component)]
pub struct NameTagPill;

/// O "!" dourado de quest disponível/pronta dentro da pílula.
#[derive(Component)]
pub struct NameTagBang;

/// How many name-tag pills are kept in the pool (reassigned per frame).
pub const NAME_TAG_POOL: usize = 8;
/// Name tags show for NPCs between these distance bounds (meters).
pub const NAME_TAG_MIN_M: f32 = 2.0;
pub const NAME_TAG_MAX_M: f32 = 60.0;
/// A partir daqui a pílula começa a esbater (0 = cheia nos 45 m).
pub const NAME_TAG_FADE_START_M: f32 = 45.0;

/// Alpha da pílula à distância `dist`: 1 até [`NAME_TAG_FADE_START_M`],
/// a descer a linear até 0 em [`NAME_TAG_MAX_M`].
pub fn tag_alpha(dist: f32) -> f32 {
    if dist <= NAME_TAG_FADE_START_M {
        1.0
    } else {
        ((NAME_TAG_MAX_M - dist) / (NAME_TAG_MAX_M - NAME_TAG_FADE_START_M)).clamp(0.0, 1.0)
    }
}

/// Cor do texto de um hostil — o vermelho do HP do hud.css (#df6964), a
/// mesma voz de "isto morde" que a barra de alvo já usa.
fn hostile_color() -> Color {
    Color::srgb(0.875, 0.412, 0.392)
}

/// Cor do texto de um NPC amigável (papel do tema).
fn paper_color() -> Color {
    Color::srgb(0.96, 0.96, 0.92)
}

/// Screen position (UI px, top-left origin) for a world point, if the camera
/// sees it (in front and within the viewport). `world_to_viewport` already
/// returns top-left-origin coordinates (bevy flips Y internally), so the
/// result plugs straight into UI `left`/`top`.
fn world_to_ui(
    camera: &Camera,
    camera_global: &GlobalTransform,
    world_pos: Vec3,
) -> Option<(f32, f32)> {
    // In front of the camera?
    let to_point = world_pos - camera_global.translation();
    if to_point.dot(camera_global.forward().as_vec3()) <= 0.0 {
        return None;
    }
    camera
        .world_to_viewport(camera_global, world_pos)
        .ok()
        .map(|vp| (vp.x, vp.y))
}

/// Um candidato a pílula: onde está, o que diz e como se veste.
struct Candidate {
    dist: f32,
    pos: Vec3,
    label: String,
    hostile: bool,
    /// Quest disponível (NotTaken) ou pronta a entregar (Ready).
    bang: bool,
}

/// Reassign the pooled name-tag pills to the nearest NPCs: "<name> <d> m".
#[allow(clippy::type_complexity)]
pub fn hud_nametags_update(
    cameras: Query<(&Camera, &GlobalTransform), Without<Player>>,
    players: Query<&GlobalTransform, With<Player>>,
    npcs: Query<
        (&GlobalTransform, &DialogueNpc, Option<&Name>),
        (Without<Player>, Without<EnemyCreature>),
    >,
    hostiles: Query<
        (&GlobalTransform, Option<&Name>),
        (With<EnemyCreature>, Without<Player>, Without<DialogueNpc>),
    >,
    quests: Option<Res<crate::quests::QuestLog>>,
    vault: Option<Res<crate::economy::Vault>>,
    mut tags: Query<
        (&mut Node, &mut Visibility, &Children),
        (With<NameTag>, Without<NameTagPill>, Without<NameTagBang>),
    >,
    mut pills: Query<
        (
            &mut BackgroundColor,
            &mut BorderColor,
            &mut TextColor,
            &mut Text,
            &Children,
        ),
        (
            With<NameTagPill>,
            Without<NameTag>,
            Without<NameTagBang>,
        ),
    >,
    mut bangs: Query<
        (&mut Visibility, &mut ImageNode),
        (With<NameTagBang>, Without<NameTag>, Without<NameTagPill>),
    >,
) {
    let Ok((camera, camera_global)) = cameras.single() else {
        return;
    };
    let Ok(player) = players.single() else {
        return;
    };
    let player_pos = player.translation();

    let mut candidates: Vec<Candidate> = Vec::new();
    for (global, npc, name) in &npcs {
        let pos = global.translation();
        let dist = pos.distance(player_pos);
        if !(NAME_TAG_MIN_M..NAME_TAG_MAX_M).contains(&dist) {
            continue;
        }
        // O "!" só existe para dialogue-ids que são mesmo quests — um NPC
        // sem quest nenhuma não é um anúncio permanente.
        let bang = quests
            .as_deref()
            .filter(|log| log.def(&npc.dialogue_id).is_some())
            .is_some_and(|log| {
                matches!(
                    log.status(&npc.dialogue_id, vault.as_deref()),
                    QuestStatus::NotTaken | QuestStatus::Ready
                )
            });
        let label = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| npc.dialogue_id.clone());
        candidates.push(Candidate {
            dist,
            pos,
            label,
            hostile: false,
            bang,
        });
    }
    for (global, name) in &hostiles {
        let pos = global.translation();
        let dist = pos.distance(player_pos);
        if !(NAME_TAG_MIN_M..NAME_TAG_MAX_M).contains(&dist) {
            continue;
        }
        candidates.push(Candidate {
            dist,
            pos,
            label: name
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Inimigo".to_string()),
            hostile: true,
            bang: false,
        });
    }
    candidates.sort_by(|a, b| a.dist.partial_cmp(&b.dist).unwrap_or(std::cmp::Ordering::Equal));

    for (index, (mut node, mut visibility, children)) in tags.iter_mut().enumerate() {
        let Some(candidate) = candidates.get(index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        // Anchor ~2 m above the NPC's feet (head height).
        let Some((x, y)) = world_to_ui(camera, camera_global, candidate.pos + Vec3::Y * 2.1) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        node.left = Val::Px(x);
        node.top = Val::Px(y);
        *visibility = Visibility::Visible;

        let fade = tag_alpha(candidate.dist);
        let Some(&pill) = children.first() else {
            continue;
        };
        let Ok((mut bg, mut border, mut color, mut text, pill_children)) = pills.get_mut(pill)
        else {
            continue;
        };
        bg.0 = Color::srgba(0.02, 0.02, 0.02, 0.78 * fade);
        *border = BorderColor::all(Color::srgba(1.0, 0.96, 0.85, 0.14 * fade));
        color.0 = if candidate.hostile {
            hostile_color().with_alpha(0.95 * fade)
        } else {
            paper_color().with_alpha(fade)
        };
        let next = format!("{} {} m", candidate.label, candidate.dist.round() as i32);
        if text.0 != next {
            text.0 = next;
        }
        // O "!" vive numa entidade própria dentro da pílula (1.º filho dela).
        if let Some(bang_entity) = pill_children.first().copied() {
            if let Ok((mut bang_visibility, mut image)) = bangs.get_mut(bang_entity) {
                let wanted = if candidate.bang {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *bang_visibility != wanted {
                    *bang_visibility = wanted;
                }
                image.color = Color::WHITE.with_alpha(fade);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_alpha_fades_after_forty_five_meters() {
        // Perto: cheio.
        assert_eq!(tag_alpha(2.0), 1.0);
        assert_eq!(tag_alpha(NAME_TAG_FADE_START_M), 1.0);
        // A meio do esbate: a meio do alpha.
        let mid = tag_alpha((NAME_TAG_FADE_START_M + NAME_TAG_MAX_M) / 2.0);
        assert!((mid - 0.5).abs() < 1e-4, "mid={mid}");
        // No corte dos 60 m: desaparecido, e além também (clamp).
        assert_eq!(tag_alpha(NAME_TAG_MAX_M), 0.0);
        assert_eq!(tag_alpha(120.0), 0.0);
    }
}
