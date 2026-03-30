from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

import httpx


def _headers(api_key: str | None) -> dict[str, str]:
    h = {"Accept": "application/json"}
    if api_key:
        h["Authorization"] = f"Bearer {api_key}"
    return h


def _poll(
    client: httpx.Client,
    base: str,
    headers: dict[str, str],
    job_id: str,
    poll_interval: float,
    timeout: float,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    last: dict[str, Any] = {}
    while time.monotonic() < deadline:
        r = client.get(f"{base.rstrip('/')}/jobs/{job_id}", headers=headers)
        r.raise_for_status()
        last = r.json()
        st = last.get("status")
        if st in ("succeeded", "failed"):
            return last
        time.sleep(poll_interval)
    raise TimeoutError(f"Job {job_id} não terminou em {timeout}s (último estado: {last.get('status')})")


def _submit(client: httpx.Client, base: str, headers: dict[str, str], job_type: str, params: dict[str, Any]) -> str:
    r = client.post(
        f"{base.rstrip('/')}/jobs",
        headers={**headers, "Content-Type": "application/json"},
        json={"type": job_type, "params": params},
        timeout=120.0,
    )
    if r.status_code >= 400:
        raise RuntimeError(f"POST /jobs falhou: {r.status_code} {r.text}")
    return str(r.json()["job_id"])


def _download_all(
    client: httpx.Client,
    base: str,
    headers: dict[str, str],
    job_id: str,
    out_dir: Path,
) -> list[Path]:
    r = client.get(f"{base.rstrip('/')}/jobs/{job_id}/artifacts", headers=headers)
    r.raise_for_status()
    names = r.json().get("files") or []
    out_dir.mkdir(parents=True, exist_ok=True)
    saved: list[Path] = []
    for name in names:
        dr = client.get(
            f"{base.rstrip('/')}/jobs/{job_id}/download/{name}",
            headers=headers,
            timeout=600.0,
        )
        dr.raise_for_status()
        p = out_dir / name
        p.write_bytes(dr.content)
        saved.append(p)
    return saved


def cmd_run_text2d(args: argparse.Namespace) -> int:
    params: dict[str, Any] = {
        "prompt": args.prompt,
        "width": args.width,
        "height": args.height,
        "steps": args.steps,
        "guidance_scale": args.guidance_scale,
        "cpu": args.cpu,
        "low_vram": args.low_vram,
        "output_basename": args.output_basename,
    }
    if args.seed is not None:
        params["seed"] = args.seed
    if args.model_id:
        params["model_id"] = args.model_id
    return _run_and_fetch(args, "text2d", params)


def cmd_run_text3d(args: argparse.Namespace) -> int:
    if not (args.from_image or (args.prompt and args.prompt.strip())):
        print("Usa --prompt ou --from-image", file=sys.stderr)
        return 2
    params: dict[str, Any] = {
        "output_format": args.format,
        "cpu": args.cpu,
        "low_vram": args.low_vram,
        "preset": args.preset,
        "image_width": args.image_width,
        "image_height": args.image_height,
        "t2d_steps": args.t2d_steps,
        "t2d_full_gpu": args.t2d_full_gpu,
        "save_reference_image": args.save_reference_image,
        "max_retries": args.max_retries,
        "output_basename": args.output_basename,
    }
    if args.prompt:
        params["prompt"] = args.prompt
    if args.from_image:
        data = Path(args.from_image).read_bytes()
        params["from_image_base64"] = base64.standard_b64encode(data).decode("ascii")
    if args.seed is not None:
        params["seed"] = args.seed
    return _run_and_fetch(args, "text3d", params)


def cmd_run_skymap2d(args: argparse.Namespace) -> int:
    params: dict[str, Any] = {
        "prompt": args.prompt,
        "width": args.width,
        "height": args.height,
        "steps": args.steps,
        "guidance_scale": args.guidance_scale,
        "image_format": args.image_format,
        "output_basename": args.output_basename,
    }
    if args.seed is not None:
        params["seed"] = args.seed
    if args.model_id:
        params["model_id"] = args.model_id
    return _run_and_fetch(args, "skymap2d", params)


def cmd_run_texture2d(args: argparse.Namespace) -> int:
    params: dict[str, Any] = {
        "prompt": args.prompt,
        "width": args.width,
        "height": args.height,
        "steps": args.steps,
        "guidance_scale": args.guidance_scale,
        "output_basename": args.output_basename,
    }
    if args.seed is not None:
        params["seed"] = args.seed
    if args.model_id:
        params["model_id"] = args.model_id
    return _run_and_fetch(args, "texture2d", params)


def _run_and_fetch(args: argparse.Namespace, job_type: str, params: dict[str, Any]) -> int:
    base = args.base_url
    headers = _headers(args.api_key)
    timeout = float(args.timeout)
    poll = float(args.poll_interval)
    out_dir = Path(args.output_dir)

    with httpx.Client() as client:
        job_id = _submit(client, base, headers, job_type, params)
        print(job_id, flush=True)
        try:
            final = _poll(client, base, headers, job_id, poll, timeout)
        except TimeoutError as e:
            print(str(e), file=sys.stderr)
            return 1
        if final.get("status") == "failed":
            err = final.get("error") or "?"
            print(err, file=sys.stderr)
            return 1
        paths = _download_all(client, base, headers, job_id, out_dir)
        for p in paths:
            print(str(p.resolve()), flush=True)
    return 0


def cmd_submit(args: argparse.Namespace) -> int:
    try:
        body = json.loads(Path(args.json_file).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        print(f"JSON inválido: {e}", file=sys.stderr)
        return 1
    job_type = body.get("type")
    params = body.get("params")
    if not job_type or not isinstance(params, dict):
        print('Ficheiro deve ser JSON: {"type": "...", "params": {...}}', file=sys.stderr)
        return 1
    headers = _headers(args.api_key)
    with httpx.Client() as client:
        job_id = _submit(client, args.base_url, headers, str(job_type), params)
        print(job_id)
    return 0


def cmd_status(args: argparse.Namespace) -> int:
    headers = _headers(args.api_key)
    with httpx.Client() as client:
        r = client.get(f"{args.base_url.rstrip('/')}/jobs/{args.job_id}", headers=headers)
        r.raise_for_status()
        print(json.dumps(r.json(), indent=2, ensure_ascii=False))
    return 0


def cmd_fetch(args: argparse.Namespace) -> int:
    headers = _headers(args.api_key)
    out_dir = Path(args.output_dir)
    with httpx.Client() as client:
        paths = _download_all(client, args.base_url, headers, args.job_id, out_dir)
        for p in paths:
            print(str(p.resolve()), flush=True)
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="inference-client", description="Cliente HTTP para inference-server")
    p.add_argument(
        "--base-url",
        default=os.environ.get("INFERENCE_CLIENT_BASE_URL", "http://127.0.0.1:8765"),
        help="URL base do servidor (ou INFERENCE_CLIENT_BASE_URL)",
    )
    p.add_argument(
        "--api-key",
        default=os.environ.get("INFERENCE_CLIENT_API_KEY"),
        help="Bearer token (ou INFERENCE_CLIENT_API_KEY); omitir se o servidor não usa chave",
    )
    sub = p.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="Submeter job, esperar e descarregar artefactos")
    run.add_argument("--timeout", type=float, default=7200.0, help="Segundos máximos a esperar")
    run.add_argument("--poll-interval", type=float, default=2.0, help="Intervalo de poll (s)")
    run.add_argument("--output-dir", "-o", default=".", help="Pasta para gravar ficheiros")
    run_sub = run.add_subparsers(dest="job_type", required=True)

    t2 = run_sub.add_parser("text2d")
    t2.add_argument("prompt")
    t2.add_argument("--width", type=int, default=1024)
    t2.add_argument("--height", type=int, default=1024)
    t2.add_argument("--steps", type=int, default=4)
    t2.add_argument("--guidance-scale", type=float, default=1.0)
    t2.add_argument("--seed", type=int, default=None)
    t2.add_argument("--cpu", action="store_true")
    t2.add_argument("--low-vram", action="store_true")
    t2.add_argument("--model-id", default=None)
    t2.add_argument("--output-basename", default="output")
    t2.set_defaults(func=cmd_run_text2d)

    t3 = run_sub.add_parser("text3d")
    t3.add_argument("--prompt", default="", help="Text-to-3D (omitir se --from-image)")
    t3.add_argument("--from-image", type=str, default=None, help="Imagem de entrada (image-to-3D)")
    t3.add_argument("--format", choices=("glb", "ply", "obj"), default="glb")
    t3.add_argument("--preset", choices=("fast", "balanced", "hq"), default=None)
    t3.add_argument("--image-width", type=int, default=768)
    t3.add_argument("--image-height", type=int, default=768)
    t3.add_argument("--t2d-steps", type=int, default=8)
    t3.add_argument("--t2d-full-gpu", action="store_true")
    t3.add_argument("--save-reference-image", action="store_true")
    t3.add_argument("--max-retries", type=int, default=1)
    t3.add_argument("--seed", type=int, default=None)
    t3.add_argument("--cpu", action="store_true")
    t3.add_argument("--low-vram", action="store_true")
    t3.add_argument("--output-basename", default="mesh")
    t3.set_defaults(func=cmd_run_text3d)

    sm = run_sub.add_parser("skymap2d")
    sm.add_argument("prompt")
    sm.add_argument("--width", type=int, default=2048)
    sm.add_argument("--height", type=int, default=1024)
    sm.add_argument("--steps", type=int, default=40)
    sm.add_argument("--guidance-scale", type=float, default=6.0)
    sm.add_argument("--seed", type=int, default=None)
    sm.add_argument("--image-format", choices=("png", "exr"), default="png")
    sm.add_argument("--model-id", default=None)
    sm.add_argument("--output-basename", default="skymap")
    sm.set_defaults(func=cmd_run_skymap2d)

    tx = run_sub.add_parser("texture2d")
    tx.add_argument("prompt")
    tx.add_argument("--width", type=int, default=1024)
    tx.add_argument("--height", type=int, default=1024)
    tx.add_argument("--steps", type=int, default=50)
    tx.add_argument("--guidance-scale", type=float, default=7.5)
    tx.add_argument("--seed", type=int, default=None)
    tx.add_argument("--model-id", default=None)
    tx.add_argument("--output-basename", default="texture")
    tx.set_defaults(func=cmd_run_texture2d)

    sp = sub.add_parser("submit", help="Submeter job a partir de JSON (só devolve job_id)")
    sp.add_argument("json_file", type=str)
    sp.set_defaults(func=cmd_submit)

    st = sub.add_parser("status", help="Estado de um job")
    st.add_argument("job_id")
    st.set_defaults(func=cmd_status)

    ft = sub.add_parser("fetch", help="Descarregar artefactos de um job já concluído")
    ft.add_argument("job_id")
    ft.add_argument("--output-dir", "-o", default=".")
    ft.set_defaults(func=cmd_fetch)

    return p


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    fn = getattr(args, "func", None)
    if fn is None:
        parser.print_help()
        sys.exit(2)
    sys.exit(fn(args))


if __name__ == "__main__":
    main()
