from __future__ import annotations

import json
import logging
import traceback
from pathlib import Path

from .db import JobStore
from .handlers import HANDLERS

logger = logging.getLogger(__name__)


def run_one_job(store: JobStore, artifacts_root: Path, job_id: str) -> None:
    row = store.get_job(job_id)
    if row is None or row.status != "running":
        return

    job_dir = artifacts_root / job_id
    job_dir.mkdir(parents=True, exist_ok=True)

    try:
        params = json.loads(row.params_json)
        handler = HANDLERS.get(row.job_type)  # type: ignore[arg-type]
        if handler is None:
            raise ValueError(f"Tipo de job desconhecido: {row.job_type}")
        handler(job_id, params, job_dir)
        store.set_status(job_id, "succeeded", completed=True)
    except Exception as e:
        tb = traceback.format_exc()
        logger.exception("Job %s falhou", job_id)
        err_msg = f"{e!s}\n\n{tb}"
        store.set_status(job_id, "failed", error=err_msg[:16000], completed=True)
