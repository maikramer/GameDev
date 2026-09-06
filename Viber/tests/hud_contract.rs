//! The shipped HUD must remain addressable by its script and engine bindings.
use std::{collections::HashSet, path::Path};

use viber::{
    ui::bind::UiData,
    xml::{self, XmlNode},
};

fn visit<'a>(nodes: &'a [XmlNode], out: &mut Vec<&'a XmlNode>) {
    for node in nodes {
        out.push(node);
        visit(&node.children, out);
    }
}

#[test]
fn shipped_hud_has_unique_ids_and_valid_bindings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/simple-rpg");
    let mut ids = HashSet::new();
    for file in ["world/hud.xml", "world/menu.xml", "qa-hud.xml"] {
        let doc = xml::parse_file(&root.join(file)).unwrap();
        let mut nodes = Vec::new();
        visit(&doc.children, &mut nodes);
        for node in nodes {
            if let Some(id) = node.attr("id") {
                assert!(ids.insert(id.to_owned()), "duplicate id {id}");
            }
            if file == "world/hud.xml" {
                if let Some(binding) = node.attr("bind") {
                    assert!(
                        UiData::default().get(binding).is_some(),
                        "unknown binding {binding}"
                    );
                }
                if node.tag == "UiIcon" {
                    let src = node.attr("src").expect("icon source");
                    assert!(
                        root.join(src.trim_start_matches('/')).exists(),
                        "missing icon {src}"
                    );
                }
            }
        }
    }
    for id in [
        "vitals",
        "hp-bar",
        "xp-bar",
        "quest",
        "dock",
        "cd-dash",
        "cd-heal",
        "cd-strike",
        "vial-potion",
        "vial-antidote",
        "vial-bomb",
        "prompt",
        "notification",
        "notification-text",
        "open-journal",
        "menu",
    ] {
        assert!(ids.contains(id), "missing HUD element {id}");
    }
}

#[test]
fn hud_script_updates_danger_cooldowns_stock_and_opens_journal() {
    let lua = mlua::Lua::new();
    lua.load(
        r#"
        values = {health=1, ["cd.dash"]=0, ["cd.heal"]=0.5, ["cd.strike"]=0,
                  potion=2, antidote=0, bomb=1}
        classes = {}
        state = {}
        click = false
        viber = {state=function() return state end, ui={
            number=function(key) return values[key] or 0 end,
            get=function(key) return "" end,
            toggle_class=function(id, class, value) classes[id .. ":" .. class] = value end,
            clicked=function(id) return id == "open-journal" and click end,
            open=function(id) opened = id end,
            set_anim=function() end
        }}
    "#,
    )
    .exec()
    .unwrap();
    lua.load(include_str!("../examples/simple-rpg/scripts/ui/hud.lua"))
        .exec()
        .unwrap();
    lua.load(
        r#"
        on_update(0.016)
        assert(classes["vitals:danger"] == false)
        assert(classes["cd-dash:ready"] == true)
        assert(classes["cd-heal:ready"] == false)
        assert(classes["vial-potion:out-of-stock"] == false)
        assert(classes["vial-antidote:out-of-stock"] == true)
        assert(classes["vial-bomb:out-of-stock"] == false)
        values.health = 0.2
        values.potion = 0
        values["cd.heal"] = 0
        click = true
        on_update(0.016)
        assert(classes["vitals:danger"] == true)
        assert(classes["vial-potion:out-of-stock"] == true)
        assert(classes["cd-heal:ready"] == true)
        assert(opened == "menu")
        values.health = 1
        on_update(0.016)
        assert(classes["vitals:danger"] == false)
    "#,
    )
    .exec()
    .unwrap();
}
