from __future__ import annotations

import asyncio
import contextlib
import json
import logging
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Annotated

from fastapi import Depends, FastAPI, Header, HTTPException, Request
from fastapi.responses import FileResponse

from . import __version__
from .cleanup import cleanup_expired_jobs
from .config import Settings, get_settings
from .db import JobStore
from .deps import optional_imports
from .schemas import (
    ArtifactsResponse,
    CreateJobRequest,
    CreateJobResponse,
    JobStatusResponse,
    JobType,
    Skymap2DParams,
    Text2DParams,
    Text3DParams,
    Texture2DParams,
    VersionResponse,
)
from .worker import run_one_job

logger = logging.getLogger(__name__)

_PARAM_MODELS: dict[JobType, type] = {
    "text2d": Text2DParams,
    "text3d": Text3DParams,
    "skymap2d": Skymap2DParams,
    "texture2d": Texture2DParams,
}


def _artifacts_path(settings: Settings) -> Path:
    p = settings.data_dir / "artifacts"
    p.mkdir(parents=True, exist_ok=True)
    return p


def _db_path(settings: Settings) -> Path:
    settings.data_dir.mkdir(parents=True, exist_ok=True)
    return settings.data_dir / "jobs.sqlite"


def verify_api_key(
    settings: Annotated[Settings, Depends(get_settings)],
    authorization: Annotated[str | None, Header()] = None,
) -> None:
    if not settings.api_key:
        return
    expected = f"Bearer {settings.api_key}"
    if authorization != expected:
        raise HTTPException(status_code=401, detail="Authorization inválido ou em falta")


@asynccontextmanager
async def lifespan(app: FastAPI):
    settings = get_settings()
    store = JobStore(_db_path(settings))
    artifacts_root = _artifacts_path(settings)
    app.state.settings = settings
    app.state.store = store
    app.state.artifacts_root = artifacts_root

    stop = asyncio.Event()

    async def worker() -> None:
        while not stop.is_set():
            job_id = await asyncio.to_thread(store.fetch_next_queued)
            if job_id is None:
                await asyncio.sleep(settings.worker_poll_seconds)
                continue
            await asyncio.to_thread(run_one_job, store, artifacts_root, job_id)

    async def janitor() -> None:
        while not stop.is_set():
            await asyncio.sleep(settings.cleanup_interval_seconds)
            await asyncio.to_thread(
                cleanup_expired_jobs,
                store,
                artifacts_root,
                settings.job_ttl_seconds,
            )

    await asyncio.to_thread(
        cleanup_expired_jobs,
        store,
        artifacts_root,
        settings.job_ttl_seconds,
    )

    worker_task = asyncio.create_task(worker(), name="inference-worker")
    janitor_task = asyncio.create_task(janitor(), name="inference-janitor")
    logger.info(
        "Inference server: data_dir=%s api_key=%s",
        settings.data_dir,
        "sim" if settings.api_key else "não (inseguro em LAN pública)",
    )
    yield
    stop.set()
    worker_task.cancel()
    janitor_task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await worker_task
    with contextlib.suppress(asyncio.CancelledError):
        await janitor_task


app = FastAPI(title="Inference Server", version=__version__, lifespan=lifespan)


def get_store(request: Request) -> JobStore:
    return request.app.state.store


def get_artifacts_root(request: Request) -> Path:
    return request.app.state.artifacts_root


def _job_dir(artifacts_root: Path, job_id: str) -> Path:
    return artifacts_root / job_id


def _safe_file_path(job_dir: Path, filename: str) -> Path:
    if not filename or filename in (".", "..") or "/" in filename or "\\" in filename:
        raise HTTPException(status_code=400, detail="Nome de ficheiro inválido")
    path = (job_dir / filename).resolve()
    job_r = job_dir.resolve()
    try:
        path.relative_to(job_r)
    except ValueError:
        raise HTTPException(status_code=400, detail="Caminho fora do job") from None
    return path


@app.get("/health")
def health():
    return {"status": "ok"}


@app.get("/version", response_model=VersionResponse)
def version():
    opt = optional_imports()
    return VersionResponse(
        inference_server_version=__version__,
        job_types=list(_PARAM_MODELS.keys()),
        optional_pipelines_installed=all(opt.values()),
    )


@app.post("/jobs", response_model=CreateJobResponse, dependencies=[Depends(verify_api_key)])
def create_job(
    req: CreateJobRequest,
    store: Annotated[JobStore, Depends(get_store)],
):
    model = _PARAM_MODELS[req.type]
    try:
        validated = model.model_validate(req.params)
    except Exception as e:
        raise HTTPException(status_code=422, detail=str(e)) from e

    payload = validated.model_dump(mode="json")
    job_id = store.create_job(req.type, payload)
    return CreateJobResponse(job_id=job_id)


@app.get("/jobs/{job_id}", response_model=JobStatusResponse, dependencies=[Depends(verify_api_key)])
def get_job(job_id: str, store: Annotated[JobStore, Depends(get_store)]):
    row = store.get_job(job_id)
    if row is None:
        raise HTTPException(status_code=404, detail="Job não encontrado")
    try:
        params = json.loads(row.params_json)
    except json.JSONDecodeError:
        params = {}
    return JobStatusResponse(
        id=row.id,
        type=row.job_type,  # type: ignore[arg-type]
        status=row.status,  # type: ignore[arg-type]
        error=row.error,
        created_at=row.created_at,
        updated_at=row.updated_at,
        started_at=row.started_at,
        completed_at=row.completed_at,
        params=params,
    )


@app.get(
    "/jobs/{job_id}/artifacts",
    response_model=ArtifactsResponse,
    dependencies=[Depends(verify_api_key)],
)
def list_artifacts(
    job_id: str,
    store: Annotated[JobStore, Depends(get_store)],
    artifacts_root: Annotated[Path, Depends(get_artifacts_root)],
):
    if store.get_job(job_id) is None:
        raise HTTPException(status_code=404, detail="Job não encontrado")
    d = _job_dir(artifacts_root, job_id)
    if not d.is_dir():
        return ArtifactsResponse(files=[])
    files = sorted(f.name for f in d.iterdir() if f.is_file())
    return ArtifactsResponse(files=files)


@app.get("/jobs/{job_id}/download/{filename}", dependencies=[Depends(verify_api_key)])
def download_file(
    job_id: str,
    filename: str,
    store: Annotated[JobStore, Depends(get_store)],
    artifacts_root: Annotated[Path, Depends(get_artifacts_root)],
):
    row = store.get_job(job_id)
    if row is None:
        raise HTTPException(status_code=404, detail="Job não encontrado")
    if row.status != "succeeded":
        raise HTTPException(status_code=409, detail="Job ainda não concluído com sucesso")
    job_dir = _job_dir(artifacts_root, job_id)
    path = _safe_file_path(job_dir, filename)
    if not path.is_file():
        raise HTTPException(status_code=404, detail="Ficheiro não encontrado")
    return FileResponse(path, filename=path.name)
