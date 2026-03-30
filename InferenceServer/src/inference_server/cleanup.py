from __future__ import annotations

import logging
import shutil
import time
from pathlib import Path

from .db import JobStore

logger = logging.getLogger(__name__)


def cleanup_expired_jobs(store: JobStore, artifacts_root: Path, ttl_seconds: int) -> int:
    """Remove jobs terminados mais antigos que TTL. Devolve número de jobs apagados."""
    threshold = time.time() - ttl_seconds
    pairs = store.list_stale_completed(threshold)
    removed = 0
    for job_id, _status in pairs:
        job_dir = artifacts_root / job_id
        try:
            if job_dir.is_dir():
                shutil.rmtree(job_dir, ignore_errors=True)
        except OSError as e:
            logger.warning("Falha a apagar pasta do job %s: %s", job_id, e)
        store.delete_job(job_id)
        removed += 1
    if removed:
        logger.info("Limpeza TTL: removidos %d jobs", removed)
    return removed
