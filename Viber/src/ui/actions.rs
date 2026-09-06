//! Menu actions: what a button in the declarative UI actually *does*.
//!
//! Scripts raise an action by name — `viber.ui.action("learn", "vitality1")` —
//! and one handler per domain applies it. Keeping them here rather than in the
//! scripting module means the whole menu (structure, look, behaviour and the
//! gameplay it triggers) lives in `src/ui`, and a world can add a button
//! without touching the sandbox.
//!
//! Actions are messages, not direct mutation: a script runs mid-frame with the
//! Lua VM borrowed, so anything touching gameplay resources has to happen
//! afterwards.

use bevy::prelude::*;

use crate::economy::Vault;
use crate::luau::ScriptToast;
use crate::menus::{ShopAction, shop_apply, shop_catalog};

/// One action raised by the UI.
#[derive(Debug, Clone, bevy::ecs::message::Message)]
pub struct UiAction {
    /// Action name (`learn`, `buy`, `sell`, `save`, `load`).
    pub name: String,
    /// Its argument — a skill id, an item id, or empty.
    pub arg: String,
}

/// Index of `item` in the shop catalogue for the given direction.
///
/// The catalogue is `(label, price, item, qty)` and holds both a buy and a
/// sell line for different items, so the direction disambiguates.
pub fn shop_index(item: &str, buying: bool) -> Option<usize> {
    shop_catalog()
        .iter()
        .position(|(_, price, id, _)| *id == item && (*price > 0) == buying)
}

/// Turns a [`ShopAction`] into the line the player sees.
pub fn shop_message(action: &ShopAction) -> String {
    match action {
        ShopAction::Bought { item, price } => {
            format!(
                "Comprado {} por {price} de ouro.",
                crate::ui::menu_data::item_label(item)
            )
        }
        ShopAction::Sold { item, earned } => {
            format!(
                "Vendido {} por {earned} de ouro.",
                crate::ui::menu_data::item_label(item)
            )
        }
        ShopAction::OutOfStock { item } => {
            format!(
                "Sem {} para vender.",
                crate::ui::menu_data::item_label(item)
            )
        }
        ShopAction::CannotAfford { item, price } => format!(
            "{} custa {price} de ouro — não chega.",
            crate::ui::menu_data::item_label(item)
        ),
        ShopAction::Nothing => "Nada a negociar.".to_string(),
    }
}

/// Applies the economy actions (`buy` / `sell`).
pub fn apply_shop_actions(
    mut actions: bevy::ecs::message::MessageReader<UiAction>,
    mut vault: ResMut<Vault>,
    mut toasts: bevy::ecs::message::MessageWriter<ScriptToast>,
) {
    for action in actions.read() {
        let buying = match action.name.as_str() {
            "buy" => true,
            "sell" => false,
            _ => continue,
        };
        let Some(index) = shop_index(&action.arg, buying) else {
            warn!("ui: `{}` is not in the shop catalogue", action.arg);
            continue;
        };
        let outcome = shop_apply(&mut vault, index);
        toasts.write(ScriptToast(shop_message(&outcome)));
    }
}

/// Applies the skill actions (`learn`).
#[allow(clippy::type_complexity)]
pub fn apply_skill_actions(
    mut actions: bevy::ecs::message::MessageReader<UiAction>,
    // Resource, não componente do herói — ver a nota em `menu_data.rs`.
    mut tree: Option<ResMut<crate::skills::SkillTree>>,
    mut stats: Option<ResMut<crate::skills::PlayerStatsResource>>,
    mut heroes: Query<(&mut crate::vitals::Health, &mut crate::player::Player)>,
    mut toasts: bevy::ecs::message::MessageWriter<ScriptToast>,
) {
    for action in actions.read() {
        if action.name != "learn" {
            continue;
        }
        let Some(tree) = tree.as_deref_mut() else {
            continue;
        };
        let label = crate::skills::SKILLS
            .iter()
            .find(|s| s.id == action.arg)
            .map(|s| s.label)
            .unwrap_or("talento");
        match tree.learn(&action.arg) {
            Some(gained) => {
                if let Some(stats) = stats.as_deref_mut() {
                    // Passivas vivas: o delta (anterior → novo) aplica-se
                    // TAMBÉM ao herói (HP máx + speed) — sem isto,
                    // vitalidade/agilidade compradas por click não faziam
                    // NADA na sessão (só ao carregar um save).
                    let previous = stats.0;
                    stats.0 = gained;
                    if let Ok((mut health, mut player)) = heroes.single_mut() {
                        crate::skills::apply_passive_delta(
                            &mut health,
                            &mut player,
                            &previous,
                            &gained,
                        );
                    }
                }
                toasts.write(ScriptToast(format!("Aprendeste: {label}.")));
            }
            None => {
                toasts.write(ScriptToast(
                    "Sem pontos de talento ou requisitos por cumprir.".into(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shop_index_distinguishes_buying_from_selling() {
        // The catalogue buys potions and sells wood; asking for the wrong
        // direction must not silently trade the other way.
        assert!(shop_index("potion", true).is_some());
        assert!(shop_index("potion", false).is_none());
        assert!(shop_index("wood", false).is_some());
        assert!(shop_index("wood", true).is_none());
        assert!(shop_index("dragon", true).is_none());
    }

    #[test]
    fn test_shop_messages_name_the_item_in_portuguese() {
        let bought = shop_message(&ShopAction::Bought {
            item: "potion".into(),
            price: 25,
        });
        assert!(bought.contains("Poção"), "{bought}");
        assert!(bought.contains("25"), "{bought}");
        let broke = shop_message(&ShopAction::CannotAfford {
            item: "bomb".into(),
            price: 40,
        });
        assert!(broke.contains("Bomba") && broke.contains("40"), "{broke}");
        assert!(!shop_message(&ShopAction::Nothing).is_empty());
    }
}
