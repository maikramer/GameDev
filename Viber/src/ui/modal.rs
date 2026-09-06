//! Modals, tab groups and scrolling — the three things a game menu needs that
//! a plain element tree does not give for free.
//!
//! All three are declarative:
//!
//! ```xml
//! <UiModal id="menu" key="F1" class="overlay">
//!   <UiPanel class="frame">
//!     <UiRow class="tabbar">
//!       <UiButton class="tab" tab-group="menu" tab="quests"><UiText>Missões</UiText></UiButton>
//!       <UiButton class="tab" tab-group="menu" tab="bag"><UiText>Mochila</UiText></UiButton>
//!     </UiRow>
//!     <UiPanel class="page" tab-group="menu" tab="quests" scroll="y"> … </UiPanel>
//!     <UiPanel class="page" tab-group="menu" tab="bag" scroll="y"> … </UiPanel>
//!   </UiPanel>
//! </UiModal>
//! ```
//!
//! The active tab button gets the `active` class, so the stylesheet decides
//! what "selected" looks like; inactive pages are `display: none`, which keeps
//! them out of layout entirely instead of merely invisible.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::runtime::{UiClasses, UiId, UiStyleDirty};

/// A full-screen overlay toggled by a key.
#[derive(Debug, Component)]
pub struct UiModal {
    /// Key that opens and closes it.
    pub key: KeyCode,
    /// Open right now?
    pub open: bool,
    /// Escape also closes it (true unless the author says otherwise).
    pub escape_closes: bool,
}

/// Which modals are open — gameplay systems read this to stop stealing input.
#[derive(Debug, Default, Resource)]
pub struct UiModalsOpen {
    pub open: Vec<String>,
}

impl UiModalsOpen {
    pub fn any(&self) -> bool {
        !self.open.is_empty()
    }

    pub fn is_open(&self, id: &str) -> bool {
        self.open.iter().any(|o| o == id)
    }
}

/// A tab button: selects `tab` inside `group`.
#[derive(Debug, Component)]
pub struct UiTabButton {
    pub group: String,
    pub tab: String,
}

/// Next tab in a group, wrapping.
///
/// `tabs` is the group's buttons in document order; `delta` is +1 / -1.
pub fn cycle(tabs: &[String], current: Option<&str>, delta: i32) -> Option<String> {
    if tabs.is_empty() {
        return None;
    }
    let index = current
        .and_then(|c| tabs.iter().position(|t| t == c))
        .unwrap_or(0) as i32;
    let count = tabs.len() as i32;
    Some(tabs[(index + delta).rem_euclid(count) as usize].clone())
}

/// A tab page: visible only while `tab` is the selection of `group`.
#[derive(Debug, Component)]
pub struct UiTabPage {
    pub group: String,
    pub tab: String,
}

/// Selected tab per group; the first page authored in a group wins by default.
#[derive(Debug, Default, Resource)]
pub struct UiTabs {
    pub active: std::collections::HashMap<String, String>,
}

impl UiTabs {
    pub fn selected(&self, group: &str) -> Option<&str> {
        self.active.get(group).map(String::as_str)
    }

    /// Selects a tab; `true` when it actually changed.
    pub fn select(&mut self, group: &str, tab: &str) -> bool {
        if self.selected(group) == Some(tab) {
            return false;
        }
        self.active.insert(group.to_string(), tab.to_string());
        true
    }
}

/// A scrollable viewport; `speed` is pixels per wheel line.
#[derive(Debug, Component)]
pub struct UiScroll {
    pub vertical: bool,
    pub speed: f32,
}

impl Default for UiScroll {
    fn default() -> Self {
        Self {
            vertical: true,
            speed: 26.0,
        }
    }
}

/// Parses a key name from a `key="…"` attribute.
///
/// Only the keys a menu realistically binds; an unknown name is reported and
/// the modal falls back to F1 rather than becoming unopenable.
pub fn parse_key(name: &str) -> KeyCode {
    match name.trim().to_ascii_lowercase().as_str() {
        "q" => KeyCode::KeyQ,
        "e" => KeyCode::KeyE,
        "i" => KeyCode::KeyI,
        "m" => KeyCode::KeyM,
        "k" => KeyCode::KeyK,
        "j" => KeyCode::KeyJ,
        "l" => KeyCode::KeyL,
        "b" => KeyCode::KeyB,
        "c" => KeyCode::KeyC,
        "tab" => KeyCode::Tab,
        "escape" | "esc" => KeyCode::Escape,
        "f1" => KeyCode::F1,
        "f2" => KeyCode::F2,
        "f3" => KeyCode::F3,
        other => {
            if !other.is_empty() {
                warn!("ui: unknown modal key `{other}` — falling back to F1");
            }
            KeyCode::F1
        }
    }
}

/// Opens and closes modals from the keyboard and mirrors the state into
/// `Visibility` and [`UiModalsOpen`].
///
/// While an `<UiInput>` holds the keyboard, its keys are PROSE, not commands:
/// the toggle keys are ignored (Escape included — it blurs the field instead),
/// so typing "Menu" in a text field never flings the menu open.
pub fn drive_ui_modals(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Res<super::widgets::UiFocusedInput>,
    mut open: ResMut<UiModalsOpen>,
    mut modals: Query<(&mut UiModal, &mut Visibility, Option<&UiId>)>,
) {
    let escape = typing.0.is_none() && keys.just_pressed(KeyCode::Escape);
    for (mut modal, mut visibility, id) in &mut modals {
        let toggled = typing.0.is_none() && keys.just_pressed(modal.key);
        // Escape only ever closes: opening every modal with one key would be a
        // surprise, closing them all with it is the expected thing.
        let closed_by_escape = modal.open && modal.escape_closes && escape;
        if toggled || closed_by_escape {
            modal.open = if closed_by_escape { false } else { !modal.open };
        }
        let wanted = if modal.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if let Some(id) = id {
            let listed = open.open.iter().position(|o| *o == id.0);
            match (modal.open, listed) {
                (true, None) => open.open.push(id.0.clone()),
                (false, Some(index)) => {
                    open.open.remove(index);
                }
                _ => {}
            }
        }
    }
}

/// Seeds each group's selection from the first page authored in it, so a menu
/// is never opened on a blank body.
pub fn seed_ui_tabs(
    mut tabs: ResMut<UiTabs>,
    pages: Query<(&UiTabPage, Option<&super::runtime::UiOrder>), Added<UiTabPage>>,
) {
    // Lowest sibling index wins, so "the first tab the author wrote" is the
    // default no matter what order the ECS reports the new pages in.
    let mut best: std::collections::HashMap<&str, (usize, &str)> = std::collections::HashMap::new();
    for (page, order) in &pages {
        let order = order.map(|o| o.0).unwrap_or(usize::MAX);
        let entry = best
            .entry(page.group.as_str())
            .or_insert((order, page.tab.as_str()));
        if order < entry.0 {
            *entry = (order, page.tab.as_str());
        }
    }
    for (group, (_, tab)) in best {
        tabs.active
            .entry(group.to_string())
            .or_insert_with(|| tab.to_string());
    }
}

/// Applies clicks to tab groups, then syncs button classes and page display.
#[allow(clippy::type_complexity)]
pub fn drive_ui_tabs(
    mut commands: Commands,
    mut tabs: ResMut<UiTabs>,
    clicked: Query<(&Interaction, &UiTabButton), Changed<Interaction>>,
    mut buttons: Query<(Entity, &UiTabButton, &mut UiClasses)>,
    mut pages: Query<(&UiTabPage, &mut Node)>,
) {
    for (interaction, button) in &clicked {
        if *interaction == Interaction::Pressed {
            tabs.select(&button.group, &button.tab);
        }
    }
    for (entity, button, mut classes) in &mut buttons {
        let active = tabs.selected(&button.group) == Some(button.tab.as_str());
        let changed = if active {
            classes.add("active")
        } else {
            classes.remove("active")
        };
        if changed {
            commands.entity(entity).insert(UiStyleDirty);
        }
    }
    for (page, mut node) in &mut pages {
        // `display: none` rather than `Visibility::Hidden`: an inactive page
        // must not take part in layout, or the panel sizes itself to the
        // tallest tab and every page floats in a too-large box.
        let wanted = if tabs.selected(&page.group) == Some(page.tab.as_str()) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != wanted {
            node.display = wanted;
        }
    }
}

/// Keyboard navigation for open modals.
///
/// Two ways in, because a six-tab menu wants both: the **digits 1-9** jump
/// straight to a tab, and **`,` / `.`** step through them.
///
/// Tab and the arrow keys are deliberately *not* used: Bevy's focus navigation
/// consumes them before an `Update` system sees them, so a menu bound to Tab
/// silently does nothing. Digits are free here because the hotbar stops
/// listening while a menu is open.
pub fn drive_ui_tab_keys(
    keys: Res<ButtonInput<KeyCode>>,
    open: Res<UiModalsOpen>,
    mut tabs: ResMut<UiTabs>,
    buttons: Query<(&UiTabButton, Option<&super::runtime::UiOrder>)>,
) {
    if !open.any() {
        return;
    }
    let step = if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::BracketRight) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Comma) || keys.just_pressed(KeyCode::BracketLeft) {
        Some(-1)
    } else {
        None
    };
    let jump = DIGIT_KEYS.iter().position(|key| keys.just_pressed(*key));
    if step.is_none() && jump.is_none() {
        return;
    }
    for (group, names) in tabs_by_group(&buttons) {
        if let Some(index) = jump {
            if let Some(tab) = names.get(index) {
                tabs.select(&group, tab);
            }
            continue;
        }
        if let Some(next) = cycle(&names, tabs.selected(&group), step.unwrap_or(0)) {
            tabs.select(&group, &next);
        }
    }
}

/// Digits 1-9, in order.
const DIGIT_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// Tab names per group, in the order the author wrote them.
///
/// The order comes from [`crate::ui::runtime::UiOrder`] (the sibling index),
/// never from entity ids: Bevy recycles those, and sorting by them silently
/// reversed the whole tab bar.
fn tabs_by_group(
    buttons: &Query<(&UiTabButton, Option<&super::runtime::UiOrder>)>,
) -> Vec<(String, Vec<String>)> {
    let mut by_group: std::collections::HashMap<&str, Vec<(usize, &str)>> =
        std::collections::HashMap::new();
    for (button, order) in buttons {
        by_group.entry(button.group.as_str()).or_default().push((
            order.map(|o| o.0).unwrap_or(usize::MAX),
            button.tab.as_str(),
        ));
    }
    by_group
        .into_iter()
        .map(|(group, mut entries)| {
            entries.sort_by_key(|(order, _)| *order);
            (
                group.to_string(),
                entries
                    .into_iter()
                    .map(|(_, tab)| tab.to_string())
                    .collect(),
            )
        })
        .collect()
}

/// Scrolls the hovered viewport with the mouse wheel.
///
/// `Interaction` only lands on the element directly under the cursor, so the
/// walk up the hierarchy is what makes scrolling work when the pointer is over
/// a row rather than over the viewport itself.
pub fn drive_ui_scroll(
    mut wheel: bevy::ecs::message::MessageReader<MouseWheel>,
    hovered: Query<(Entity, &Interaction)>,
    parents: Query<&ChildOf>,
    scrolls: Query<&UiScroll>,
    mut positions: Query<&mut ScrollPosition>,
) {
    let mut delta = 0.0;
    for event in wheel.read() {
        delta += match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 26.0,
        };
    }
    if delta == 0.0 {
        return;
    }
    let Some((target, _)) = hovered
        .iter()
        .find(|(_, interaction)| **interaction != Interaction::None)
    else {
        return;
    };
    let mut current = target;
    for _ in 0..32 {
        if let Ok(scroll) = scrolls.get(current) {
            if let Ok(mut position) = positions.get_mut(current) {
                let step = delta * scroll.speed;
                if scroll.vertical {
                    position.0.y = (position.0.y - step).max(0.0);
                } else {
                    position.0.x = (position.0.x - step).max(0.0);
                }
            }
            return;
        }
        let Ok(parent) = parents.get(current) else {
            return;
        };
        current = parent.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_covers_the_menu_keys_and_falls_back_loudly() {
        assert_eq!(parse_key("q"), KeyCode::KeyQ);
        assert_eq!(parse_key("F1"), KeyCode::F1);
        assert_eq!(parse_key("Tab"), KeyCode::Tab);
        assert_eq!(parse_key("esc"), KeyCode::Escape);
        // Unknown or empty never yields an unopenable modal.
        assert_eq!(parse_key("hyperspace"), KeyCode::F1);
        assert_eq!(parse_key(""), KeyCode::F1);
    }

    #[test]
    fn test_tabs_select_reports_real_changes() {
        let mut tabs = UiTabs::default();
        assert_eq!(tabs.selected("menu"), None);
        assert!(tabs.select("menu", "quests"));
        assert_eq!(tabs.selected("menu"), Some("quests"));
        assert!(!tabs.select("menu", "quests"), "re-selecting is a no-op");
        assert!(tabs.select("menu", "bag"));
        // Groups are independent.
        assert!(tabs.select("shop", "buy"));
        assert_eq!(tabs.selected("menu"), Some("bag"));
    }

    #[test]
    fn test_cycle_wraps_in_both_directions() {
        let tabs: Vec<String> = ["quests", "bag", "skills"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(cycle(&tabs, Some("quests"), 1).as_deref(), Some("bag"));
        assert_eq!(cycle(&tabs, Some("skills"), 1).as_deref(), Some("quests"));
        assert_eq!(cycle(&tabs, Some("quests"), -1).as_deref(), Some("skills"));
        // An unknown or absent selection starts from the first tab.
        assert_eq!(cycle(&tabs, None, 1).as_deref(), Some("bag"));
        assert_eq!(cycle(&tabs, Some("nope"), -1).as_deref(), Some("skills"));
        assert_eq!(cycle(&[], Some("quests"), 1), None);
    }

    use super::super::runtime::UiOrder;

    /// World with one open modal and a tab bar authored in the given order.
    fn tabbed_world(tabs_in_order: &[&str]) -> World {
        let mut world = World::new();
        world.init_resource::<UiTabs>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.insert_resource(UiModalsOpen {
            open: vec!["menu".into()],
        });
        // Spawned in REVERSE author order on purpose: entity ids are recycled
        // by Bevy, so only `UiOrder` may decide what "the next tab" means.
        for (index, tab) in tabs_in_order.iter().enumerate().rev() {
            world.spawn((
                UiTabButton {
                    group: "menu".into(),
                    tab: (*tab).into(),
                },
                UiOrder(index),
            ));
        }
        world
    }

    fn run_tab_keys(world: &mut World) {
        #[allow(clippy::type_complexity)]
        let mut state: bevy::ecs::system::SystemState<(
            Res<ButtonInput<KeyCode>>,
            Res<UiModalsOpen>,
            ResMut<UiTabs>,
            Query<(&UiTabButton, Option<&UiOrder>)>,
        )> = bevy::ecs::system::SystemState::new(world);
        let (keys, open, tabs, buttons) = state.get_mut(world).expect("system state");
        drive_ui_tab_keys(keys, open, tabs, buttons);
    }

    /// One fresh key press. `ButtonInput::press` only records `just_pressed`
    /// for a key that was not already down, so the previous press has to be
    /// released first — exactly what a real frame does.
    fn press(world: &mut World, key: KeyCode) {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        keys.press(key);
    }

    #[test]
    fn test_digit_keys_jump_straight_to_the_nth_tab() {
        let mut world = tabbed_world(&["quests", "bag", "skills", "shop", "controls", "system"]);
        press(&mut world, KeyCode::Digit3);
        run_tab_keys(&mut world);
        assert_eq!(world.resource::<UiTabs>().selected("menu"), Some("skills"));
        press(&mut world, KeyCode::Digit1);
        run_tab_keys(&mut world);
        assert_eq!(world.resource::<UiTabs>().selected("menu"), Some("quests"));
        press(&mut world, KeyCode::Digit6);
        run_tab_keys(&mut world);
        assert_eq!(world.resource::<UiTabs>().selected("menu"), Some("system"));
        // A digit past the last tab leaves the selection alone.
        press(&mut world, KeyCode::Digit9);
        run_tab_keys(&mut world);
        assert_eq!(world.resource::<UiTabs>().selected("menu"), Some("system"));
    }

    #[test]
    fn test_step_keys_walk_the_tabs_in_author_order() {
        let mut world = tabbed_world(&["quests", "bag", "skills"]);
        press(&mut world, KeyCode::Digit1);
        run_tab_keys(&mut world);
        press(&mut world, KeyCode::Period);
        run_tab_keys(&mut world);
        assert_eq!(
            world.resource::<UiTabs>().selected("menu"),
            Some("bag"),
            "`.` moves forward through the AUTHORED order, not entity order"
        );
        press(&mut world, KeyCode::Comma);
        run_tab_keys(&mut world);
        assert_eq!(world.resource::<UiTabs>().selected("menu"), Some("quests"));
        // Wraps backwards off the first tab.
        press(&mut world, KeyCode::Comma);
        run_tab_keys(&mut world);
        assert_eq!(world.resource::<UiTabs>().selected("menu"), Some("skills"));
    }

    #[test]
    fn test_tab_keys_do_nothing_while_every_modal_is_closed() {
        let mut world = tabbed_world(&["quests", "bag"]);
        world.resource_mut::<UiModalsOpen>().open.clear();
        press(&mut world, KeyCode::Digit2);
        run_tab_keys(&mut world);
        assert_eq!(
            world.resource::<UiTabs>().selected("menu"),
            None,
            "a closed menu must not swallow the player's number keys"
        );
    }

    #[test]
    fn test_modals_open_tracks_ids() {
        let mut open = UiModalsOpen::default();
        assert!(!open.any());
        open.open.push("menu".into());
        assert!(open.any());
        assert!(open.is_open("menu"));
        assert!(!open.is_open("shop"));
    }

    #[test]
    fn test_seed_ui_tabs_picks_the_first_authored_page() {
        use super::super::runtime::UiOrder;
        let mut world = World::new();
        world.init_resource::<UiTabs>();
        // Spawned out of author order on purpose: entity ids are recycled by
        // Bevy, so the seed must key on `UiOrder`, not on spawn order.
        world.spawn((
            UiTabPage {
                group: "menu".into(),
                tab: "bag".into(),
            },
            UiOrder(3),
        ));
        world.spawn((
            UiTabPage {
                group: "menu".into(),
                tab: "quests".into(),
            },
            UiOrder(1),
        ));
        #[allow(clippy::type_complexity)]
        let mut state: bevy::ecs::system::SystemState<(
            ResMut<UiTabs>,
            Query<(&UiTabPage, Option<&UiOrder>), Added<UiTabPage>>,
        )> = bevy::ecs::system::SystemState::new(&mut world);
        let (tabs, pages) = state.get_mut(&mut world).expect("system state");
        seed_ui_tabs(tabs, pages);
        assert_eq!(
            world.resource::<UiTabs>().selected("menu"),
            Some("quests"),
            "the lowest-ordered page is the default"
        );
    }
}
