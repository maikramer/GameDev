#!/usr/bin/env python3
"""
Teste de Retry: Quantas tentativas para obter um modelo sem placa?

Gera o mesmo objeto múltiplas vezes com seeds diferentes e verifica
quantos retries são necessários em média para obter um modelo limpo.

Uso:
    source ../../Text3D/.venv/bin/activate
    python test_retry.py
"""

from __future__ import annotations

import json
import time
from datetime import datetime
from pathlib import Path

import sys

PROJECT_ROOT = Path(__file__).parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "Shared" / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "Text3D" / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "GameDevLab" / "src"))

from text3d import HunyuanTextTo3DGenerator
from text3d.utils.export import save_mesh
from text3d.utils.mesh_repair import repair_mesh
from gamedev_lab.mesh_inspector import MeshInspector

# Configurações
OUTPUT_DIR = Path(__file__).parent / "retry_test"
OUTPUT_DIR.mkdir(exist_ok=True)

TEST_CONFIG = {
    "name": "chair_modern",
    "prompt": "modern minimalist chair with wooden legs, clean design, studio lighting",
    "max_attempts": 10,  # Máximo de tentativas
    "seeds": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],  # Seeds para testar
}


def test_single_generation(prompt: str, seed: int, output_path: Path) -> dict:
    """Gera um modelo e retorna o relatório de inspeção."""
    try:
        with HunyuanTextTo3DGenerator(verbose=False) as gen:
            mesh = gen.generate(
                prompt=prompt,
                t2d_seed=seed,
                hy_seed=seed,
                octree_resolution=256,
                num_chunks=8000,
                num_inference_steps=24,
            )

            # Aplicar reparo padrão
            mesh = repair_mesh(mesh, remove_ground_shadow=True)

            save_mesh(mesh, str(output_path), format="glb")

        # Inspecionar
        inspector = MeshInspector(str(output_path))
        report = inspector.inspect()

        return {
            "success": True,
            "seed": seed,
            "path": str(output_path),
            "grade": report.score.grade,
            "passed": report.passed(),
            "has_plate": len(report.artifacts.backing_plates) > 0,
            "plates": [
                {"axis": p["axis"], "side": p["side"], "coverage": p["coverage"]}
                for p in report.artifacts.backing_plates
            ],
            "volume_efficiency": report.geometry.volume_efficiency,
            "thickness_ratio": report.geometry.thickness_ratio,
        }

    except Exception as e:
        return {
            "success": False,
            "seed": seed,
            "error": str(e),
        }


def run_retry_test():
    """Executa o teste de retry."""
    print("=" * 70)
    print("TESTE DE RETRY: Quantas tentativas para modelo limpo?")
    print("=" * 70)
    print(f"Objeto: {TEST_CONFIG['name']}")
    print(f"Prompt: {TEST_CONFIG['prompt']}")
    print(f"Máximo de tentativas: {TEST_CONFIG['max_attempts']}")
    print(f"Seeds: {TEST_CONFIG['seeds']}")
    print("")

    results = []
    success_without_plate = None

    for i, seed in enumerate(TEST_CONFIG["seeds"], 1):
        print(f"\n--- Tentativa {i}/{TEST_CONFIG['max_attempts']} (seed={seed}) ---")

        output_path = OUTPUT_DIR / f"{TEST_CONFIG['name']}_seed{seed}.glb"

        start_time = time.time()
        result = test_single_generation(TEST_CONFIG["prompt"], seed, output_path)
        elapsed = time.time() - start_time

        result["attempt"] = i
        result["time"] = elapsed
        results.append(result)

        if result["success"]:
            print(f"  ✓ Gerado em {elapsed:.1f}s")
            print(f"  Grade: {result['grade']}, Has Plate: {result['has_plate']}")

            if result["has_plate"]:
                print(f"  ⚠️  Placas detectadas:")
                for plate in result["plates"]:
                    print(f"     - {plate['axis']}-{plate['side']}: {plate['coverage']:.2f}")
            else:
                print(f"  ✅ SEM PLACA! Modelo limpo encontrado!")
                if success_without_plate is None:
                    success_without_plate = i
        else:
            print(f"  ✗ Falha: {result.get('error', 'unknown')}")

    # Relatório final
    print("\n" + "=" * 70)
    print("RESULTADO DO TESTE DE RETRY")
    print("=" * 70)

    successful = [r for r in results if r["success"]]
    with_plate = [r for r in successful if r["has_plate"]]
    without_plate = [r for r in successful if not r["has_plate"]]

    print(f"\nTotal de tentativas: {len(results)}")
    print(f"Gerações bem-sucedidas: {len(successful)}")
    print(f"Com placa: {len(with_plate)}")
    print(f"Sem placa: {len(without_plate)}")

    if success_without_plate:
        print(f"\n✅ SUCESSO na tentativa {success_without_plate}!")
        print(f"   Seed vencedor: {results[success_without_plate - 1]['seed']}")
        print(f"   Grade: {results[success_without_plate - 1]['grade']}")
    else:
        print(f"\n❌ NENHUM modelo sem placa encontrado em {len(results)} tentativas")

    print(f"\nTabela de resultados:")
    print(f"{'Tent':<6} {'Seed':<6} {'Grade':<8} {'Placa?':<8} {'Coverage':<12} {'Tempo':<8}")
    print("-" * 60)

    for r in results:
        if r["success"]:
            plate_info = "SIM" if r["has_plate"] else "NÃO"
            coverage = max([p["coverage"] for p in r["plates"]], default=0)
            print(
                f"{r['attempt']:<6} {r['seed']:<6} {r['grade']:<8} {plate_info:<8} {coverage:<12.2f} {r['time']:<8.1f}s"
            )
        else:
            print(f"{r['attempt']:<6} {r['seed']:<6} {'ERRO':<8} {'N/A':<8} {'N/A':<12} {'N/A':<8}")

    # Salvar resultados
    results_file = OUTPUT_DIR / "retry_results.json"
    with open(results_file, "w") as f:
        json.dump(
            {
                "config": TEST_CONFIG,
                "results": results,
                "summary": {
                    "total_attempts": len(results),
                    "successful": len(successful),
                    "with_plate": len(with_plate),
                    "without_plate": len(without_plate),
                    "first_success_attempt": success_without_plate,
                },
                "timestamp": datetime.now().isoformat(),
            },
            f,
            indent=2,
            default=str,
        )

    print(f"\nResultados salvos em: {results_file}")

    return results


if __name__ == "__main__":
    run_retry_test()
