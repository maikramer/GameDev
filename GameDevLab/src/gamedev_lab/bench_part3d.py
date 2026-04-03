"""Bancada Part3D: matrizes de quantização com VRAM."""

from __future__ import annotations

import json
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from gamedev_shared.subprocess_utils import resolve_binary
from gamedev_shared.vram_monitor import VRAMMonitor


@dataclass
class Part3DBenchConfig:
    nome: str
    quantizacao: str
    steps: int
    octree: int
    descricao: str


DEFAULT_CONFIGS: list[Part3DBenchConfig] = [
    Part3DBenchConfig(
        nome="baseline-fp16",
        quantizacao="none",
        steps=50,
        octree=256,
        descricao="Baseline sem quantização (FP16 puro)",
    ),
    Part3DBenchConfig(
        nome="quanto-int8",
        quantizacao="quanto-int8",
        steps=50,
        octree=256,
        descricao="Quanto INT8 (engine atual)",
    ),
    Part3DBenchConfig(
        nome="sdnq-uint8",
        quantizacao="sdnq-uint8",
        steps=50,
        octree=256,
        descricao="SDNQ UINT8 (moderno, unsigned)",
    ),
    Part3DBenchConfig(
        nome="sdnq-int8",
        quantizacao="sdnq-int8",
        steps=50,
        octree=256,
        descricao="SDNQ INT8 (moderno, signed)",
    ),
    Part3DBenchConfig(
        nome="sdnq-uint8-high",
        quantizacao="sdnq-uint8",
        steps=75,
        octree=384,
        descricao="SDNQ UINT8 com steps/octree maiores",
    ),
]


def _part3d_bin() -> str:
    return resolve_binary("PART3D_BIN", "part3d")


def run_part3d_test(
    mesh_path: Path,
    config: Part3DBenchConfig,
    output_dir: Path,
    *,
    timeout_sec: float = 600.0,
) -> dict[str, Any]:
    output_dir.mkdir(exist_ok=True, parents=True)
    abin = _part3d_bin()

    cmd = [
        abin,
        "decompose",
        str(mesh_path),
        "-o",
        str(output_dir / f"{mesh_path.stem}_parts.glb"),
        "--output-segmented",
        str(output_dir / f"{mesh_path.stem}_segmented.glb"),
        "--steps",
        str(config.steps),
        "--octree-resolution",
        str(config.octree),
        "--quantization",
        config.quantizacao,
        "--seed",
        "42",
        "--profile",
    ]

    monitor = VRAMMonitor(interval_sec=0.5)

    try:
        import torch

        torch.cuda.empty_cache()
        torch.cuda.synchronize()

        monitor.start()
        t0 = time.time()

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout_sec,
        )

        elapsed = time.time() - t0
        stats = monitor.stop()

        sucesso = result.returncode == 0

        metrics: dict[str, Any] = {
            "nome": config.nome,
            "config": {
                "quantizacao": config.quantizacao,
                "steps": config.steps,
                "octree": config.octree,
            },
            "sucesso": sucesso,
            "tempo_segundos": elapsed,
            "vram_pico_mb": stats.peak_allocated_mb if stats else None,
            "vram_livre_min_mb": stats.min_free_mb if stats else None,
            "stdout": result.stdout[-2000:] if len(result.stdout) > 2000 else result.stdout,
            "stderr": result.stderr[-2000:] if len(result.stderr) > 2000 else result.stderr,
        }

        return metrics

    except subprocess.TimeoutExpired:
        monitor.stop()
        return {"nome": config.nome, "sucesso": False, "erro": "timeout"}
    except Exception as e:
        monitor.stop()
        return {"nome": config.nome, "sucesso": False, "erro": str(e)}


def analisar_resultados(resultados: list[dict[str, Any]]) -> None:
    print("\n" + "=" * 60)
    print("ANÁLISE DOS RESULTADOS - Part3D")
    print("=" * 60)

    sucessos = [r for r in resultados if r.get("sucesso")]
    falhas = [r for r in resultados if not r.get("sucesso")]

    print(f"\nSucessos: {len(sucessos)}/{len(resultados)}")
    if falhas:
        print(f"Falhas: {len(falhas)}/{len(resultados)}")
        for f in falhas:
            print(f"   - {f['nome']}: {f.get('erro', 'falha desconhecida')}")

    if not sucessos:
        print("\nNenhum teste teve sucesso!")
        return

    print("\nCOMPARAÇÃO:")
    print(f"{'Config':<20} {'Tempo':<10} {'VRAM Pico':<12} {'Status':<10}")
    print("-" * 60)

    for r in resultados:
        nome = r["nome"]
        tempo = f"{r.get('tempo_segundos', 0):.1f}s" if r.get("tempo_segundos") else "N/A"
        vram = f"{r['vram_pico_mb']:.0f}MB" if r.get("vram_pico_mb") else "N/A"
        status = "OK" if r["sucesso"] else "FALHA"
        print(f"{nome:<20} {tempo:<10} {vram:<12} {status:<10}")

    def score(r: dict[str, Any]) -> float:
        if not r["sucesso"]:
            return -1.0
        s = 0.0
        cfg = r.get("config") or {}
        q = str(cfg.get("quantizacao", ""))
        if "sdnq" in q:
            s += 10
        if "uint8" in q:
            s += 5
        if r.get("tempo_segundos", 0) > 300:
            s -= 2
        return s

    melhor = max(sucessos, key=score)
    print("\nRECOMENDAÇÃO:")
    print(f"   Configuração: {melhor['nome']}")
    mc = melhor.get("config") or {}
    print(f"   Quantização: {mc.get('quantizacao')}")
    print(f"   Steps: {mc.get('steps')}, Octree: {mc.get('octree')}")
    print(f"   Tempo: {melhor.get('tempo_segundos', 0):.1f}s")
    if melhor.get("vram_pico_mb"):
        print(f"   VRAM pico: {melhor['vram_pico_mb']:.0f} MB")


def run_bench_cli(
    mesh: Path,
    modo: str,
    output_dir: Path,
) -> int:
    if not mesh.is_file():
        print(f"Mesh não encontrado: {mesh}", file=sys.stderr)
        return 1

    output_dir.mkdir(parents=True, exist_ok=True)

    if modo == "sweep":
        resultados: list[dict[str, Any]] = []
        for config in DEFAULT_CONFIGS:
            result = run_part3d_test(mesh, config, output_dir / config.nome)
            resultados.append(result)
            partial = output_dir / "resultados_partial.json"
            partial.write_text(json.dumps(resultados, indent=2), encoding="utf-8")

        analisar_resultados(resultados)
        (output_dir / "resultados_final.json").write_text(json.dumps(resultados, indent=2), encoding="utf-8")
        return 0

    config = next((c for c in DEFAULT_CONFIGS if c.nome == modo), None)
    if not config:
        print(f"Configuração desconhecida: {modo}", file=sys.stderr)
        return 1

    resultado = run_part3d_test(mesh, config, output_dir)
    print(json.dumps(resultado, indent=2))
    return 0 if resultado.get("sucesso") else 1
