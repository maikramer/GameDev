#!/usr/bin/env python3
"""
Teste comparativo: Prompt Enhancer Original vs v3 vs Ultra

Gera o mesmo objeto com diferentes estratégias de prompt enhancement
para ver qual produz menos placas.
"""

from __future__ import annotations

import json
import sys
import time
from datetime import datetime
from pathlib import Path

# Paths
PROJECT_ROOT = Path(__file__).parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "Shared" / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "Text3D" / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "GameDevLab" / "src"))

from text3d import HunyuanTextTo3DGenerator
from text3d.utils.export import save_mesh
from text3d.utils.mesh_repair import repair_mesh
from gamedev_lab.mesh_inspector import MeshInspector

# Import nossos enhancers
from prompt_enhancer_v3 import enhance_prompt_v3

OUTPUT_DIR = Path(__file__).parent / "prompt_test"
OUTPUT_DIR.mkdir(exist_ok=True)

TEST_OBJECT = {
    "name": "chair_modern",
    "prompt": "modern minimalist chair with wooden legs, clean design, studio lighting",
}

ENHANCER_VERSIONS = [
    ("original", lambda p: p),  # Sem enhancement
    ("text3d_default", None),  # Usará o prompt_enhance do Text3D
    ("v3_standard", lambda p: enhance_prompt_v3(p, mode="standard")),
    ("v3_ultra", lambda p: enhance_prompt_v3(p, mode="ultra")),
]


def generate_with_prompt(name: str, prompt: str, seed: int = 42) -> dict:
    """Gera um modelo e retorna métricas."""
    output_path = OUTPUT_DIR / f"{name}.glb"

    print(f"  Prompt: {prompt[:100]}...")

    try:
        start = time.time()
        with HunyuanTextTo3DGenerator(verbose=False) as gen:
            # Desativar o prompt enhancement interno se estivermos usando o nosso
            mesh = gen.generate(
                prompt=prompt,
                t2d_seed=seed,
                hy_seed=seed,
                octree_resolution=256,
                num_chunks=8000,
                num_inference_steps=24,
                optimize_prompt=False,  # Controlamos nós mesmos
            )

            mesh = repair_mesh(mesh, remove_ground_shadow=True)
            save_mesh(mesh, str(output_path), format="glb")

        gen_time = time.time() - start

        # Inspecionar
        inspector = MeshInspector(str(output_path))
        report = inspector.inspect()

        return {
            "success": True,
            "prompt_length": len(prompt),
            "generation_time": gen_time,
            "grade": report.score.grade,
            "has_plate": len(report.artifacts.backing_plates) > 0,
            "plates": report.artifacts.backing_plates,
            "volume_efficiency": report.geometry.volume_efficiency,
            "flatness_ratio": report.geometry.flatness_ratio,
            "thickness_ratio": report.geometry.thickness_ratio,
            "path": str(output_path),
        }

    except Exception as e:
        return {"success": False, "error": str(e)}


def run_comparison():
    """Executa comparação entre diferentes enhancers."""
    print("=" * 80)
    print("TESTE COMPARATIVO: Prompt Enhancers")
    print("=" * 80)
    print(f"Objeto: {TEST_OBJECT['name']}")
    print(f"Prompt base: {TEST_OBJECT['prompt']}")
    print(f"Seed: 42 (fixo para todos)")
    print()

    results = []
    base_prompt = TEST_OBJECT["prompt"]

    for version_name, enhancer_func in ENHANCER_VERSIONS:
        print(f"\n--- Testando: {version_name} ---")

        # Preparar prompt
        if version_name == "text3d_default":
            # Usar o Text3D com seu enhancement padrão
            from text3d.utils.prompt_enhance import create_optimized_prompt

            final_prompt = create_optimized_prompt(base_prompt, aggressive=True)
        elif enhancer_func:
            final_prompt = enhancer_func(base_prompt)
        else:
            final_prompt = base_prompt

        print(f"  Enhancer: {version_name}")

        # Gerar
        result = generate_with_prompt(f"{TEST_OBJECT['name']}_{version_name}", final_prompt, seed=42)

        result["version"] = version_name
        result["enhanced_prompt"] = final_prompt
        results.append(result)

        # Print resultado
        if result["success"]:
            plate_status = "✅ SEM PLACA" if not result["has_plate"] else f"❌ PLACA ({len(result['plates'])}x)"
            print(f"  Resultado: {plate_status}, Grade: {result['grade']}, Tempo: {result['generation_time']:.1f}s")
            if result["has_plate"]:
                for p in result["plates"]:
                    print(f"    - {p['axis']}-{p['side']}: {p['coverage']:.2f}")
        else:
            print(f"  ❌ Erro: {result.get('error')}")

    # Relatório final
    print("\n" + "=" * 80)
    print("RESULTADO COMPARATIVO")
    print("=" * 80)

    print(f"\n{'Versão':<20} {'Tamanho':<10} {'Grade':<8} {'Placa?':<10} {'V-Eff':<8} {'Flat':<8}")
    print("-" * 80)

    for r in results:
        if r["success"]:
            plate = "NÃO ✅" if not r["has_plate"] else f"SIM ({len(r['plates'])})"
            print(
                f"{r['version']:<20} {r['prompt_length']:<10} {r['grade']:<8} {plate:<10} "
                f"{r['volume_efficiency']:.3f}    {r['flatness_ratio']:.3f}"
            )
        else:
            print(f"{r['version']:<20} {'ERRO':<10} {'N/A':<8} {'N/A':<10} {'N/A':<8} {'N/A':<8}")

    # Melhor estratégia
    successful = [r for r in results if r["success"]]
    without_plate = [r for r in successful if not r["has_plate"]]

    print("\n" + "-" * 80)
    if without_plate:
        print(f"✅ MELHOR(ES): {', '.join(r['version'] for r in without_plate)} - Nenhuma placa detectada!")
    else:
        # Ordenar por coverage total
        sorted_plates = sorted(
            [r for r in successful if r["has_plate"]], key=lambda x: sum(p["coverage"] for p in x["plates"])
        )
        if sorted_plates:
            print(f"⚠️  MENOR PLACA: {sorted_plates[0]['version']} (menor coverage total)")

    # Salvar resultados
    results_file = OUTPUT_DIR / "comparison_results.json"
    with open(results_file, "w") as f:
        json.dump(
            {
                "test_object": TEST_OBJECT,
                "results": results,
                "timestamp": datetime.now().isoformat(),
            },
            f,
            indent=2,
            default=str,
        )

    print(f"\nResultados salvos em: {results_file}")
    print(f"Meshes em: {OUTPUT_DIR}")


if __name__ == "__main__":
    run_comparison()
