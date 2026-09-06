//! Testes headless do debug bridge: App mínima + bridge real sobre HTTP
//! (loopback) — sem janela, sem render. Os testes e2e correm num único
//! `#[test]` para não competir pela mesma porta.

use super::client::BridgeClient;
use super::*;
use bevy::MinimalPlugins;
use std::thread::JoinHandle;

const TEST_PORT: u16 = 35702;

/// Bombeia frames até a chamada responder (a resposta só é escrita quando
/// `app.update()` processa o pedido em RemoteLast).
fn settle(app: &mut App, handle: JoinHandle<Result<Value, String>>) -> Value {
    for _ in 0..600 {
        app.update();
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    handle.join().unwrap().expect("bridge call responde")
}

/// Variante para chamadas que DEVEM falhar (devolve a mensagem de erro BRP).
fn settle_err(app: &mut App, handle: JoinHandle<Result<Value, String>>) -> String {
    for _ in 0..600 {
        app.update();
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    handle.join().unwrap().expect_err("chamada devia falhar")
}

fn call_async(method: &'static str, params: Value) -> JoinHandle<Result<Value, String>> {
    call_async_on(TEST_PORT, method, params)
}

/// Variante com porta explícita — cada App de teste tem a sua, porque os
/// testes correm em paralelo no mesmo binário.
fn call_async_on(
    port: u16,
    method: &'static str,
    params: Value,
) -> JoinHandle<Result<Value, String>> {
    std::thread::spawn(move || {
        BridgeClient::localhost(port)
            .call(method, params)
            .map_err(|error| error.to_string())
    })
}

#[test]
fn test_bridge_end_to_end_headless() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BridgePlugin { port: TEST_PORT });
    app.world_mut()
        .spawn((Name::new("hero"), Transform::default()));
    // Sem WindowPlugin (headless), registamos o message de cursor à mão.
    app.add_message::<CursorMoved>();
    app.update(); // Startup: liga o servidor HTTP

    // ping
    let pong = settle(&mut app, call_async(METHOD_PING, serde_json::json!({})));
    let _ = "ping responde";
    assert_eq!(pong["pong"], serde_json::json!(true));
    assert!(pong["version"].is_string());

    // tree: hero presente
    let tree = settle(&mut app, call_async(METHOD_TREE, serde_json::json!({})));
    let _ = "tree responde";
    let entries = tree.as_array().expect("tree é lista");
    assert!(
        entries
            .iter()
            .any(|n| n.get("name").and_then(Value::as_str) == Some("hero")),
        "hero na árvore: {tree}"
    );

    // input.key → evento KeyboardInput + ButtonInput
    settle(
        &mut app,
        call_async(METHOD_KEY, serde_json::json!({ "key": "KeyW" })),
    );
    let keyboard = app.world().resource::<Messages<KeyboardInput>>();
    assert!(
        keyboard.len() >= 2,
        "press+release enviados: {}",
        keyboard.len()
    );

    // input.text
    let text = settle(
        &mut app,
        call_async(METHOD_TEXT, serde_json::json!({ "text": "aB" })),
    );
    assert_eq!(text["chars"], serde_json::json!(2));

    // input.click + input.move → CursorMoved
    let click = call_async(METHOD_CLICK, serde_json::json!({ "x": 10.0, "y": 20.0 }));
    let mouse_move = call_async(METHOD_MOVE, serde_json::json!({ "x": 30.0, "y": 40.0 }));
    for _ in 0..600 {
        app.update();
        if click.is_finished() && mouse_move.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    click.join().unwrap().expect("input.click responde");
    mouse_move.join().unwrap().expect("input.move responde");
    // O clique é injectado no PreUpdate do frame SEGUINTE (press agora,
    // release depois — ver `deferred_mouse_release`): dá-lhe dois frames.
    app.update();
    app.update();
    let cursor = app.world().resource::<Messages<CursorMoved>>();
    assert!(
        cursor.len() >= 2,
        "CursorMoved de click+move: {}",
        cursor.len()
    );

    // screenshot: pedido em fila fica pending sem render
    let shot = settle(
        &mut app,
        call_async(METHOD_SCREENSHOT, serde_json::json!({})),
    );
    let id = shot["id"].as_u64().expect("capture id");

    let status = settle(
        &mut app,
        call_async(METHOD_SCREENSHOT_STATUS, serde_json::json!({ "id": id })),
    );
    assert_eq!(status["status"], serde_json::json!("pending"));

    let error = settle_err(
        &mut app,
        call_async(METHOD_SCREENSHOT_STATUS, serde_json::json!({ "id": 99999 })),
    );
    assert!(error.contains("unknown capture id"), "erro: {error}");
}

#[test]
fn test_normalize_key_aliases() {
    use super::client::normalize_key;
    assert_eq!(normalize_key("w"), "KeyW");
    assert_eq!(normalize_key("7"), "Digit7");
    assert_eq!(normalize_key("space"), "Space");
    assert_eq!(normalize_key("esc"), "Escape");
    assert_eq!(normalize_key("up"), "ArrowUp");
    assert_eq!(normalize_key("ctrl"), "ControlLeft");
    assert_eq!(normalize_key("KeyW"), "KeyW");
    assert_eq!(normalize_key("F5"), "F5");
}

#[test]
fn test_keycode_for_char_mapping() {
    assert_eq!(keycode_for_char('a'), (KeyCode::KeyA, false));
    assert_eq!(keycode_for_char('A'), (KeyCode::KeyA, true));
    assert_eq!(keycode_for_char('5'), (KeyCode::Digit5, false));
    assert_eq!(keycode_for_char('!'), (KeyCode::Digit1, true));
    assert_eq!(keycode_for_char(' '), (KeyCode::Space, false));
}

/// Porta própria: não pode colidir com `TEST_PORT` do e2e do bridge nem com
/// outros testes Luau (correm em paralelo no mesmo binário).
const LUA_TEST_PORT: u16 = 35703;
const LUA2_TEST_PORT: u16 = 35704;
const LUA3_TEST_PORT: u16 = 35705;
const LUA4_TEST_PORT: u16 = 35706;

/// App headless com runtime Luau + bridge: base para os testes `viber.lua`.
fn lua_app(port: u16) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(crate::luau::LuauScriptPlugin::default())
        .add_plugins(BridgePlugin { port });
    // Assets para os testes de introspeção (sem AssetPlugin na app mínima).
    app.init_resource::<bevy::asset::Assets<bevy::mesh::Mesh>>();
    app.init_resource::<bevy::asset::Assets<bevy::pbr::StandardMaterial>>();
    app.world_mut().spawn((
        Name::new("player"),
        crate::player::Player::default(),
        Transform::from_xyz(0.0, 1.0, 0.0),
        crate::vitals::Health::default(),
        crate::vitals::Xp::default(),
    ));
    app.world_mut().spawn((
        Name::new("goblin"),
        Transform::from_xyz(5.0, 0.0, 0.0),
        crate::luau::LuaScriptRef {
            path: "ghost.lua".into(),
        },
    ));
    app.world_mut().spawn((
        Name::new("dummy"),
        Transform::from_xyz(0.0, 0.0, 50.0),
        crate::vitals::Health::default(),
    ));
    // Entidade "rich" com mesh + material + collider + corpo rígido: alvo
    // dos testes de introspeção (info/mesh/material/collider/components).
    let mut meshes = app
        .world_mut()
        .get_resource_mut::<bevy::asset::Assets<bevy::mesh::Mesh>>()
        .unwrap();
    let cube = meshes.add(bevy::mesh::Mesh::from(bevy::math::primitives::Cuboid::new(
        1.0, 1.0, 1.0,
    )));
    let mut materials = app
        .world_mut()
        .get_resource_mut::<bevy::asset::Assets<bevy::pbr::StandardMaterial>>()
        .unwrap();
    let material = materials.add(bevy::pbr::StandardMaterial {
        base_color: bevy::color::Color::srgb(0.8, 0.2, 0.1),
        ..Default::default()
    });
    drop(materials);
    app.world_mut().spawn((
        Name::new("rich"),
        Transform::from_xyz(1.0, 2.0, 3.0).with_scale(Vec3::splat(2.0)),
        bevy::render::mesh::Mesh3d(cube),
        bevy::pbr::MeshMaterial3d(material),
        bevy_rapier3d::prelude::Collider::cuboid(0.5, 0.5, 0.5),
        bevy_rapier3d::prelude::RigidBody::Fixed,
        bevy::light::PointLight {
            intensity: 1200.0,
            shadow_maps_enabled: true,
            ..Default::default()
        },
    ));
    app.update(); // Startup: liga o servidor HTTP
    app
}

#[test]
fn test_bridge_lua_end_to_end_headless() {
    let mut app = lua_app(LUA_TEST_PORT);

    // `return` devolve o valor; conversão mlua → JSON.
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "return 1 + 1" }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    assert_eq!(response["result"], serde_json::json!(2));

    // player() lê o snapshot; teleport aplica a op no mesmo frame.
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({
                "code": "viber.debug.teleport(10, 5, -3)\nreturn viber.debug.player().x"
            }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    assert_eq!(
        response["result"].as_f64(),
        Some(0.0),
        "snapshot é do início da chamada"
    );
    assert_eq!(response["applied"], serde_json::json!(1));

    let mut query = app
        .world_mut()
        .query_filtered::<(&Name, &Transform), bevy::ecs::query::With<crate::player::Player>>();
    let (_, transform) = query.single(app.world()).expect("player com Transform");
    assert_eq!(transform.translation, Vec3::new(10.0, 5.0, -3.0));

    // find por nome + set_pos; pos() lê o snapshot do INÍCIO da chamada
    // (antes da op aplicar) — o read-back vem no chunk seguinte.
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({
                "code": "viber.debug.set_pos('goblin', 1, 2, 3)\nreturn { viber.debug.pos('goblin') }"
            }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let pos = response["result"].as_array().expect("pos é xyz");
    assert_eq!(pos[0].as_f64(), Some(5.0), "posição pré-op");

    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "return { viber.debug.pos('goblin') }" }),
        ),
    );
    // Leitura vem do snapshot — a op do chunk anterior já aplicou.
    let pos = response["result"].as_array().expect("pos é xyz");
    assert_eq!(pos[1].as_f64(), Some(2.0), "y do set_pos anterior");

    // Erro Luau → ok:false com a mensagem; a engine continua viva.
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "error('boom')" }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(false));
    assert!(
        response["error"]
            .as_str()
            .is_some_and(|e| e.contains("boom")),
        "erro: {response}"
    );

    // disable() insere `Disabled` — a entidade sai das queries normais.
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "viber.debug.disable('goblin') return true" }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let mut disabled = app
        .world_mut()
        .query_filtered::<&Name, bevy::ecs::query::With<bevy::ecs::entity_disabling::Disabled>>();
    let found = disabled
        .iter(app.world())
        .any(|name| name.as_str() == "goblin");
    assert!(found, "goblin devia estar Disabled");

    // enable() devolve-o ao mundo (mesmo escondido das queries enquanto
    // estava desativado, o snapshot via-o via iter_entities).
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "viber.debug.enable('goblin') return true" }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let mut still_disabled = app
        .world_mut()
        .query_filtered::<&Name, bevy::ecs::query::With<bevy::ecs::entity_disabling::Disabled>>();
    assert!(
        !still_disabled
            .iter(app.world())
            .any(|name| name.as_str() == "goblin"),
        "goblin devia estar ativo"
    );

    // Globals persistem entre chamadas (REPL).
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "repl_hits = (repl_hits or 0) + 1 return true" }),
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let response = settle(
        &mut app,
        call_async_on(
            LUA_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": "return repl_hits" }),
        ),
    );
    assert_eq!(response["result"], serde_json::json!(1));

    // code ausente → erro BRP de params (fim do e2e: só há uma App nesta
    // porta — testes em paralelo não podem competir pela mesma).
    let error = settle_err(
        &mut app,
        call_async_on(LUA_TEST_PORT, METHOD_LUA, serde_json::json!({})),
    );
    assert!(error.contains("invalid params"), "erro: {error}");
}

/// Segundo lote de features: move_to/rotate/set_scale/kill/set_hp,
/// leituras distance/vault/quests/prof/fps, e ops com warning (câmara,
/// relógio — sem recursos nesta App mínima).
#[test]
fn test_bridge_lua_features_round2() {
    let mut app = lua_app(LUA2_TEST_PORT);
    let lua = |code: &'static str| {
        call_async_on(
            LUA2_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": code }),
        )
    };

    // move_to: qualquer entidade, XZ absolutos (sem terreno → Y fica).
    let response = settle(
        &mut app,
        lua("viber.debug.move_to('goblin', 20, 5) return true"),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, bevy::ecs::query::Without<crate::player::Player>>();
    let goblin_pos: Vec3 = query
        .iter(app.world())
        .find(|t| t.translation.x == 20.0)
        .map(|t| t.translation)
        .expect("goblin movido");
    assert_eq!(goblin_pos.z, 5.0);

    // rotate +90°: yaw inicial identidade → π/2.
    let response = settle(
        &mut app,
        lua("viber.debug.rotate('goblin', 90) return true"),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, bevy::ecs::query::Without<crate::player::Player>>();
    let yaw = query
        .iter(app.world())
        .find(|t| t.translation.x == 20.0)
        .map(|t| t.rotation.to_euler(bevy::math::EulerRot::YXZ).0)
        .expect("goblin");
    assert!(
        (yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
        "yaw devia ser 90°, foi {}",
        yaw.to_degrees()
    );

    // set_scale uniforme.
    let response = settle(
        &mut app,
        lua("viber.debug.set_scale('goblin', 2) return true"),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, bevy::ecs::query::Without<crate::player::Player>>();
    let scale = query
        .iter(app.world())
        .find(|t| t.translation.x == 20.0)
        .map(|t| t.scale.x)
        .expect("goblin");
    assert_eq!(scale, 2.0);

    // set_hp absoluto no player + kill cru no dummy (Health a zero).
    let response = settle(
        &mut app,
        lua("viber.debug.set_hp(7) viber.debug.kill('dummy') return true"),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    assert_eq!(response["applied"], serde_json::json!(2));
    let mut player_q = app
        .world_mut()
        .query_filtered::<&crate::vitals::Health, bevy::ecs::query::With<crate::player::Player>>();
    let player_hp = player_q
        .single(app.world())
        .expect("Health do player")
        .current;
    let mut dummy_q = app
        .world_mut()
        .query_filtered::<&crate::vitals::Health, bevy::ecs::query::Without<crate::player::Player>>(
        );
    let dummy_hp = dummy_q
        .single(app.world())
        .expect("Health do dummy")
        .current;
    assert_eq!(player_hp, 7.0, "set_hp");
    assert_eq!(dummy_hp, 0.0, "kill");

    // Ops sem recurso correspondente → warnings, não erro BRP.
    let response = settle(
        &mut app,
        lua(
            "viber.debug.set_camera{distance = 9} viber.debug.set_clock(1380) viber.debug.clear_markers() return true",
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let warnings = response["warnings"].as_array().expect("warnings é lista");
    assert_eq!(warnings.len(), 2, "câmara e relógio avisam: {warnings:?}");

    // Leituras: distance, vault (nil), quests (vazio), prof/fps.
    let response = settle(
        &mut app,
        lua(
            "return { d = viber.debug.distance('player', 'goblin'), tem_vault = viber.debug.vault() ~= nil, quests = #viber.debug.quests() }",
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let result = &response["result"];
    let expected = ((20.0f32 * 20.0 + 1.0 + 5.0 * 5.0).sqrt()) as f64;
    assert!(
        (result["d"].as_f64().unwrap() - expected).abs() < 1e-3,
        "distance"
    );
    assert_eq!(
        result["tem_vault"],
        serde_json::json!(false),
        "vault ausente"
    );
    assert_eq!(
        result["quests"],
        serde_json::json!(0),
        "sem QuestLog → vazio"
    );

    let response = settle(
        &mut app,
        lua("return { entities = viber.debug.prof().entities, fps = viber.debug.fps() }"),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let result = &response["result"];
    assert!(
        result["entities"].as_u64().is_some(),
        "prof.entities presente"
    );
    assert!(result["fps"].is_null(), "fps sem DiagnosticsStore → nil");
}

/// Terceiro lote: introspeção — info/transform/mesh/material/collider/
/// components sobre a entidade "rich" (mesh+material+collider+corpo).
#[test]
fn test_bridge_lua_introspection() {
    let mut app = lua_app(LUA3_TEST_PORT);
    let lua = |code: &'static str| {
        call_async_on(
            LUA3_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": code }),
        )
    };

    // components() lista os componentes por nome.
    let response = settle(
        &mut app,
        lua("local c = viber.debug.components('rich') table.sort(c) return c"),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let components: Vec<&str> = response["result"]
        .as_array()
        .expect("lista de componentes")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    for expected in ["Transform", "Mesh3d", "Collider", "Name", "RigidBody"] {
        assert!(
            components
                .iter()
                .any(|c| c.ends_with(&format!("::{expected}")) || *c == expected),
            "'{expected}' em {components:?}"
        );
    }

    // mesh(): vértices + UVs do cubo (24 verts, uvs 0..1).
    let response = settle(
        &mut app,
        lua(
            "local m = viber.debug.mesh('rich') return { m.vertices, m.has_uvs, m.uv_min[1], m.uv_max[1] }",
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true));
    let result = response["result"].as_array().expect("lista");
    assert_eq!(result[0], serde_json::json!(24), "cubo tem 24 vértices");
    assert_eq!(result[1], serde_json::json!(true));
    assert_eq!(result[2].as_f64(), Some(0.0), "uv_min");
    assert_eq!(result[3].as_f64(), Some(1.0), "uv_max");

    // material(): cor base e sem textura.
    let response = settle(
        &mut app,
        lua(
            "local m = viber.debug.material('rich') return { r = m.base_color[1], tex = m.base_color_texture }",
        ),
    );
    let result = &response["result"];
    assert!((result["r"].as_f64().unwrap() - 0.8).abs() < 1e-3, "r≈0.8");
    assert!(result["tex"].is_null(), "sem textura base_color");

    // collider(): cuboid com half-extents 0.5 + rigidbody Fixed.
    let response = settle(
        &mut app,
        lua("local c = viber.debug.collider('rich') return { c.shape, c.hx, c.hy, c.hz }"),
    );
    let result = response["result"].as_array().expect("lista");
    assert_eq!(result[0], serde_json::json!("cuboid"));
    assert_eq!(result[1], serde_json::json!(0.5));

    // transform(): translation, scale e euler.
    let response = settle(
        &mut app,
        lua("local t = viber.debug.transform('rich') return { t.x, t.y, t.z, t.sx }"),
    );
    let result = response["result"].as_array().expect("lista");
    assert_eq!(result[0].as_f64(), Some(1.0));
    assert_eq!(result[1].as_f64(), Some(2.0));
    assert_eq!(result[2].as_f64(), Some(3.0));
    assert_eq!(result[3].as_f64(), Some(2.0), "scale uniforme");

    // info(): tabela agregada com tudo.
    let response = settle(
        &mut app,
        lua(
            "local i = viber.debug.info('rich') return { i.name, i.rigidbody, i.collider ~= nil, i.mesh ~= nil, i.material ~= nil, #i.components > 0 }",
        ),
    );
    let result = response["result"].as_array().expect("lista");
    assert_eq!(result[0], serde_json::json!("rich"));
    assert_eq!(result[1], serde_json::json!("Fixed"));
    assert_eq!(result[2], serde_json::json!(true));
    assert_eq!(result[3], serde_json::json!(true));
    assert_eq!(result[4], serde_json::json!(true));
    assert_eq!(result[5], serde_json::json!(true));

    // Entidade sem mesh → erro Luau claro (ok:false).
    let response = settle(&mut app, lua("return viber.debug.mesh('player')"));
    assert_eq!(response["ok"], serde_json::json!(false));
    assert!(
        response["error"].as_str().unwrap().contains("Mesh3d"),
        "erro: {response}"
    );
}

/// Quarto lote: bulk/profiling — stats, colliders, lights, around, physics.
#[test]
fn test_bridge_lua_bulk_profiling() {
    let mut app = lua_app(LUA4_TEST_PORT);
    let lua = |code: &'static str| {
        call_async_on(
            LUA4_TEST_PORT,
            METHOD_LUA,
            serde_json::json!({ "code": code }),
        )
    };

    // stats(): agregados do mundo inteiro.
    let response = settle(
        &mut app,
        lua(
            "local s = viber.debug.stats() return { s.entities, s.meshes, s.colliders, s.lights_point, s.lights_with_shadows, s.rigidbodies }",
        ),
    );
    assert_eq!(response["ok"], serde_json::json!(true), "resp: {response}");
    let result = response["result"].as_array().expect("lista");
    assert!(result[0].as_u64().unwrap() >= 4, "pelo menos 4 entidades");
    assert_eq!(result[1], serde_json::json!(1), "1 mesh (rich)");
    assert_eq!(result[2], serde_json::json!(1), "1 collider (rich)");
    assert_eq!(result[3], serde_json::json!(1), "1 PointLight");
    assert_eq!(result[4], serde_json::json!(1), "luz com sombras");
    assert_eq!(result[5], serde_json::json!(1), "1 RigidBody");

    // colliders(): o rich aparece com shape cuboid.
    let response = settle(
        &mut app,
        lua("local c = viber.debug.colliders() return { #c, c[1].shape, c[1].name }"),
    );
    let result = response["result"].as_array().expect("lista");
    assert_eq!(result[0], serde_json::json!(1));
    assert_eq!(result[1], serde_json::json!("cuboid"));
    assert_eq!(result[2], serde_json::json!("rich"));

    // lights(): kind/shadows/intensity.
    let response = settle(
        &mut app,
        lua(
            "local l = viber.debug.lights() return { #l, l[1].kind, l[1].shadows, l[1].intensity }",
        ),
    );
    let result = response["result"].as_array().expect("lista");
    assert_eq!(result[0], serde_json::json!(1));
    assert_eq!(result[1], serde_json::json!("point"));
    assert_eq!(result[2], serde_json::json!(true));
    assert_eq!(result[3], serde_json::json!(1200));

    // around(): player e rich no raio; dummy (z=50) fora.
    let response = settle(
        &mut app,
        lua("local a = viber.debug.around(10) return { #a, a[1].name ~= nil, a[2] ~= nil }"),
    );
    assert_eq!(response["ok"], serde_json::json!(true), "resp: {response}");
    let result = response["result"].as_array().expect("lista");
    assert_eq!(
        result[0].as_u64().unwrap(),
        3,
        "player + goblin + rich num raio de 10"
    );
    // dummy está a ~50 m: raio pequeno exclui.
    let response = settle(&mut app, lua("local a = viber.debug.around(100) return #a"));
    assert!(
        response["result"].as_u64().unwrap() >= 4,
        "raio grande inclui o dummy"
    );

    // physics(): sem RapierContext nesta app → nil (não pode panicar).
    let response = settle(&mut app, lua("return viber.debug.physics() == nil"));
    assert_eq!(response["ok"], serde_json::json!(true));
    assert_eq!(response["result"], serde_json::json!(true));
}
