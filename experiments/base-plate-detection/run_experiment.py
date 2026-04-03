#!/usr/bin/env python3
"""
Experimento: Detecção de Placas nos Pés (Backing Plates) em Modelos 3D

Gera 5 objetos 3D com text3d e avalia a precisão da detecção de placas
usando MeshInspector do GameDevLab.

Uso:
    cd experiments/base-plate-detection
    source ../../Text3D/.venv/bin/activate
    python run_experiment.py
"""

from __future__ import annotations

import json
import os
import sys
import time
from datetime import datetime
from pathlib import Path

# Configurações do experimento
OUTPUT_DIR = Path(__file__).parent.resolve()
MESHES_DIR = OUTPUT_DIR / "meshes"
VIEWS_DIR = OUTPUT_DIR / "views"
REPORTS_DIR = OUTPUT_DIR / "reports"

# Adicionar paths do projeto
PROJECT_ROOT = OUTPUT_DIR.parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "Shared" / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "Text3D" / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "GameDevLab" / "src"))


# Configurar logging simples
class SimpleLogger:
    def info(self, msg: str) -> None:
        print(f"[INFO] {msg}")

    def warning(self, msg: str) -> None:
        print(f"[WARN] {msg}")

    def error(self, msg: str) -> None:
        print(f"[ERROR] {msg}")


logger = SimpleLogger()

# Prompts variados - alguns mais propensos a placas
EXPERIMENT_OBJECTS = [
    {
        "name": "chair_modern",
        "prompt": "modern minimalist chair with wooden legs, clean design, studio lighting",
        "description": "Cadeira - objetos com pés bem definidos tendem a ter menos placas",
        "aggressive": False,
    },
    {
        "name": "vase_ceramic",
        "prompt": "ceramic vase with a narrow base, smooth surface, decorative pottery",
        "description": "Vaso - base estreita pode criar placas artificiais",
        "aggressive": False,
    },
    {
        "name": "robot_standing",
        "prompt": "robot standing on ground, mechanical legs, industrial design",
        "description": "Robô 'standing' - prompt propenso a placas",
        "aggressive": True,
    },
    {
        "name": "character_floating",
        "prompt": "cartoon character on pedestal, stylized figure, colorful",
        "description": "Personagem 'on pedestal' - propenso a placas",
        "aggressive": True,
    },
    {
        "name": "table_small",
        "prompt": "small side table with four legs, wooden top, furniture",
        "description": "Mesa pequena - móvel com múltiplos pés",
        "aggressive": False,
    },
]


def ensure_directories():
    """Cria diretórios necessários."""
    MESHES_DIR.mkdir(parents=True, exist_ok=True)
    VIEWS_DIR.mkdir(parents=True, exist_ok=True)
    REPORTS_DIR.mkdir(parents=True, exist_ok=True)
    logger.info(f"Diretórios criados: {OUTPUT_DIR}")


def generate_object(obj_config: dict, seed: int | None = None) -> tuple[Path | None, dict]:
    """Gera um objeto 3D usando text3d."""
    from text3d import HunyuanTextTo3DGenerator
    from text3d.utils.export import save_mesh
    from text3d.utils.mesh_repair import repair_mesh

    name = obj_config["name"]
    prompt = obj_config["prompt"]
    output_path = MESHES_DIR / f"{name}.glb"

    logger.info(f"Gerando: {name}")
    logger.info(f"  Prompt: {prompt}")
    logger.info(f"  Seed: {seed if seed else 'random'}")

    try:
        start_time = time.time()

        with HunyuanTextTo3DGenerator(verbose=False) as gen:
            gen_kwargs = {
                "octree_resolution": 256,
                "num_chunks": 8000,
                "num_inference_steps": 24,
            }
            if seed is not None:
                gen_kwargs["t2d_seed"] = seed
                gen_kwargs["hy_seed"] = seed

            mesh = gen.generate(prompt=prompt, **gen_kwargs)

            # Aplicar reparo se necessário
            shadow_mode = "very_aggressive" if obj_config.get("aggressive") else True
            mesh = repair_mesh(
                mesh,
                remove_ground_shadow=True,
                ground_shadow_very_aggressive=(shadow_mode == "very_aggressive"),
            )

            save_mesh(mesh, str(output_path), format="glb")

        elapsed = time.time() - start_time
        logger.info(f"  ✓ Gerado em {elapsed:.1f}s: {output_path}")

        return output_path, {
            "name": name,
            "prompt": prompt,
            "seed": seed,
            "generation_time": elapsed,
            "path": str(output_path),
            "aggressive_repair": obj_config.get("aggressive", False),
        }

    except Exception as e:
        logger.error(f"  ✗ Falha ao gerar {name}: {e}")
        import traceback

        traceback.print_exc()
        return None, {"name": name, "error": str(e)}


def analyze_with_mesh_inspector(mesh_path: Path) -> dict:
    """Analisa mesh usando MeshInspector do GameDevLab."""
    try:
        from gamedev_lab.mesh_inspector import MeshInspector
    except ImportError as e:
        logger.error(f"MeshInspector não disponível: {e}")
        return {"error": str(e)}

    logger.info(f"Analisando com MeshInspector: {mesh_path.name}")

    try:
        inspector = MeshInspector(str(mesh_path))
        report = inspector.inspect()

        # Extrair dados relevantes
        result = {
            "grade": report.score.grade,
            "passed": report.passed(),
            "topology": {
                "vertices": report.topology.vertices,
                "faces": report.topology.faces,
                "watertight": report.topology.watertight,
                "connected_components": report.topology.connected_components,
            },
            "geometry": {
                "volume_efficiency": report.geometry.volume_efficiency,
                "flatness_ratio": report.geometry.flatness_ratio,
                "thickness_ratio": report.geometry.thickness_ratio,
            },
            "artifacts": {
                "issues": report.artifacts.issues,
                "backing_plates": report.artifacts.backing_plates,
                "backing_plate_detected": len(report.artifacts.backing_plates) > 0,
                "passed": report.artifacts.passed,
            },
        }

        logger.info(f"    Grade: {result['grade']}, Passed: {result['passed']}")
        if result["artifacts"]["backing_plate_detected"]:
            logger.warning(f"    ⚠️  Backing plates detectados: {len(result['artifacts']['backing_plates'])}")
            for plate in result["artifacts"]["backing_plates"]:
                logger.warning(f"       - {plate['axis']}-{plate['side']}: {plate['coverage']:.2f} coverage")

        return result

    except Exception as e:
        logger.error(f"    Erro na inspeção: {e}")
        import traceback

        traceback.print_exc()
        return {"error": str(e)}


def generate_views(mesh_path: Path) -> list[Path]:
    """Gera vistas do mesh usando trimesh (fallback)."""
    import numpy as np

    logger.info(f"Gerando vistas: {mesh_path.name}")

    views = []

    try:
        mesh = trimesh.load(str(mesh_path), force="mesh")

        # Criar subdiretório para este mesh
        mesh_views_dir = VIEWS_DIR / mesh_path.stem
        mesh_views_dir.mkdir(parents=True, exist_ok=True)

        # Definir ângulos de câmera
        camera_configs = [
            ("front", 0, 0),
            ("three_quarter", 45, 30),
            ("right", 90, 0),
            ("back", 180, 0),
            ("low_front", 0, -30),
            ("top", 0, 90),
        ]

        for view_name, azimuth, elevation in camera_configs:
            try:
                # Criar cena
                scene = mesh.scene()
                camera = scene.camera
                camera.fov = (60, 60)

                # Calcular posição da câmera orbital
                distance = mesh.bounding_box.extents.max() * 2.5

                azimuth_rad = np.radians(azimuth)
                elevation_rad = np.radians(elevation)

                x = distance * np.cos(elevation_rad) * np.cos(azimuth_rad)
                y = distance * np.cos(elevation_rad) * np.sin(azimuth_rad)
                z = distance * np.sin(elevation_rad)

                camera.position = [x, y, z]
                camera.look_at = mesh.centroid

                # Renderizar
                png = scene.save_image(resolution=(512, 512), visible=True)

                if png:
                    view_path = mesh_views_dir / f"{view_name}.png"
                    with open(view_path, "wb") as f:
                        f.write(png)
                    views.append(view_path)

            except Exception as e:
                logger.warning(f"    Falha ao gerar vista {view_name}: {e}")

        logger.info(f"    {len(views)} vistas geradas em {mesh_views_dir}")
        return views

    except Exception as e:
        logger.error(f"    Erro ao gerar vistas: {e}")
        return []


def run_experiment():
    """Executa o experimento completo."""
    print("=" * 70)
    print("EXPERIMENTO: Detecção de Placas nos Pés (Backing Plates)")
    print("=" * 70)
    print(f"Data/Hora: {datetime.now().isoformat()}")
    print(f"Diretório: {OUTPUT_DIR}")
    print("")

    ensure_directories()

    results = []

    # Fase 1: Geração
    print("-" * 70)
    print("FASE 1: Geração de 5 Objetos 3D")
    print("-" * 70)
    print("")

    for obj_config in EXPERIMENT_OBJECTS:
        mesh_path, metadata = generate_object(obj_config, seed=42)

        if mesh_path is None:
            logger.error(f"Falha na geração de {obj_config['name']} - pulando análise")
            results.append(
                {
                    "config": obj_config,
                    "metadata": metadata,
                    "error": "Geração falhou",
                }
            )
            continue

        # Fase 2: Análise com MeshInspector
        print("")
        print(f"FASE 2: Análise GameDevLab MeshInspector - {obj_config['name']}")
        inspection_result = analyze_with_mesh_inspector(mesh_path)

        # Fase 3: Geração de Vistas
        print("")
        print(f"FASE 3: Geração de Vistas - {obj_config['name']}")
        views = generate_views(mesh_path)

        # Consolidar resultados
        results.append(
            {
                "config": obj_config,
                "metadata": metadata,
                "gamedevlab_inspection": inspection_result,
                "views_count": len(views),
                "views_dir": str(VIEWS_DIR / obj_config["name"]) if views else None,
            }
        )

        print("")
        print("=" * 70)

    # Fase 4: Relatório Comparativo
    print("")
    print("-" * 70)
    print("FASE 4: Relatório Comparativo")
    print("-" * 70)

    generate_report(results)

    # Salvar resultados completos
    results_path = REPORTS_DIR / "experiment_results.json"
    with open(results_path, "w") as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResultados salvos em: {results_path}")

    print("")
    print("=" * 70)
    print("EXPERIMENTO CONCLUÍDO")
    print("=" * 70)
    print(f"Meshes: {MESHES_DIR}")
    print(f"Vistas: {VIEWS_DIR}")
    print(f"Relatórios: {REPORTS_DIR}")


def generate_report(results: list[dict]):
    """Gera relatório comparativo dos resultados."""
    print("")
    print("TABELA COMPARATIVA: Detecção de Placas")
    print("")
    print(f"{'Objeto':<25} {'Grade':<8} {'V-Eff':<8} {'Flat':<8} {'Thick':<8} {'Placas?':<10} {'Pass?':<8}")
    print("-" * 85)

    plate_detections = []

    for r in results:
        if "error" in r:
            print(f"{r['config']['name']:<25} {'ERRO':<8} {'N/A':<8} {'N/A':<8} {'N/A':<8} {'N/A':<10} {'N/A':<8}")
            continue

        name = r["config"]["name"]
        inspection = r.get("gamedevlab_inspection", {})

        if "error" in inspection:
            print(f"{name:<25} {'ERR':<8} {'ERR':<8} {'ERR':<8} {'ERR':<8} {'ERR':<10} {'ERR':<8}")
            continue

        grade = inspection.get("grade", "?")
        vol_eff = inspection.get("geometry", {}).get("volume_efficiency", 0)
        flat = inspection.get("geometry", {}).get("flatness_ratio", 0)
        thick = inspection.get("geometry", {}).get("thickness_ratio", 0)
        has_plate = inspection.get("artifacts", {}).get("backing_plate_detected", False)
        passed = inspection.get("passed", False)

        plate_detections.append(
            {
                "name": name,
                "expected_plate": r["config"].get("aggressive", False),  # Esperamos placas em aggressive
                "detected_plate": has_plate,
                "prompt": r["config"]["prompt"],
            }
        )

        print(
            f"{name:<25} {grade:<8} {vol_eff:<8.3f} {flat:<8.3f} {thick:<8.3f} {'SIM' if has_plate else 'NÃO':<10} {'SIM' if passed else 'NÃO':<8}"
        )

    print("")
    print("-" * 70)
    print("ANÁLISE DE PRECISÃO DA DETECÇÃO")
    print("-" * 70)
    print("")
    print(f"{'Objeto':<25} {'Esperado Placa':<15} {'Detectado':<12} {'Resultado':<15}")
    print("-" * 70)

    true_positives = 0
    true_negatives = 0
    false_positives = 0
    false_negatives = 0

    for det in plate_detections:
        expected = det["expected_plate"]
        detected = det["detected_plate"]

        if expected and detected:
            result = "✓ Verdadeiro Pos"
            true_positives += 1
        elif not expected and not detected:
            result = "✓ Verdadeiro Neg"
            true_negatives += 1
        elif not expected and detected:
            result = "✗ Falso Positivo"
            false_positives += 1
        else:
            result = "✗ Falso Negativo"
            false_negatives += 1

        print(f"{det['name']:<25} {'SIM' if expected else 'NÃO':<15} {'SIM' if detected else 'NÃO':<12} {result:<15}")

    print("")
    total = len(plate_detections)
    if total > 0:
        accuracy = (true_positives + true_negatives) / total
        precision = true_positives / (true_positives + false_positives) if (true_positives + false_positives) > 0 else 0
        recall = true_positives / (true_positives + false_negatives) if (true_positives + false_negatives) > 0 else 0
        f1 = 2 * (precision * recall) / (precision + recall) if (precision + recall) > 0 else 0

        print(f"Métricas:")
        print(f"  Total de amostras: {total}")
        print(f"  Verdadeiros Positivos: {true_positives}")
        print(f"  Verdadeiros Negativos: {true_negatives}")
        print(f"  Falsos Positivos: {false_positives}")
        print(f"  Falsos Negativos: {false_negatives}")
        print(f"  Acurácia: {accuracy:.1%}")
        print(f"  Precisão: {precision:.1%}")
        print(f"  Recall: {recall:.1%}")
        print(f"  F1-Score: {f1:.2f}")

    print("")
    print("-" * 70)
    print("CONCLUSÕES")
    print("-" * 70)
    print("")

    # Análise de conclusões
    if false_negatives > 0:
        print("⚠️  Falsos Negativos detectados: MeshInspector pode estar perdendo algumas placas.")
        print("    Isso significa que modelos com placas podem passar desapercebidos.")

    if false_positives > 0:
        print("⚠️  Falsos Positivos detectados: MeshInspector pode estar sendo muito sensível.")
        print("    Isso pode gerar retrys desnecessários.")

    if accuracy >= 0.8:
        print("✓ Acurácia alta! A detecção está confiável para uso em retry automático.")
    elif accuracy >= 0.6:
        print("~ Acurácia moderada. Considere ajustar thresholds ou combinar múltiplas métricas.")
    else:
        print("✗ Acurácia baixa. A detecção precisa de melhorias antes de ser usada em retry.")

    print("")
    print("Recomendações para retry:")
    print("  - Use backing_plate_detected como sinal principal")
    print("  - Considere volume_efficiency < 0.15 como sinal secundário")
    print("  - Considere flatness_ratio < 0.12 como sinal secundário")
    print("  - Retry apenas quando grade < B E (placa detectada OU múltiplos artefatos)")


if __name__ == "__main__":
    import trimesh  # Import aqui para ter certeza que está disponível

    run_experiment()
