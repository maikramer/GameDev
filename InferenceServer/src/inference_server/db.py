from __future__ import annotations

import json
import sqlite3
import threading
import uuid
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Any

_schema_lock = threading.Lock()


@dataclass
class JobRow:
    id: str
    job_type: str
    status: str
    params_json: str
    error: str | None
    created_at: float
    updated_at: float
    started_at: float | None
    completed_at: float | None


def _connect(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    return conn


def init_schema(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY,
            job_type TEXT NOT NULL,
            status TEXT NOT NULL,
            params_json TEXT NOT NULL,
            error TEXT,
            created_at REAL NOT NULL,
            updated_at REAL NOT NULL,
            started_at REAL,
            completed_at REAL
        )
        """
    )
    conn.execute("CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_jobs_completed ON jobs(completed_at)")
    conn.commit()


class JobStore:
    def __init__(self, db_path: Path) -> None:
        self._path = db_path
        with _schema_lock:
            self._conn = _connect(db_path)
            init_schema(self._conn)

    @contextmanager
    def _write(self) -> Iterator[sqlite3.Connection]:
        with _schema_lock:
            yield self._conn

    def create_job(self, job_type: str, params: dict[str, Any]) -> str:
        job_id = uuid.uuid4().hex
        now = __import__("time").time()
        params_json = json.dumps(params, ensure_ascii=False)
        with self._write() as c:
            c.execute(
                """
                INSERT INTO jobs (id, job_type, status, params_json, error, created_at, updated_at)
                VALUES (?, ?, 'queued', ?, NULL, ?, ?)
                """,
                (job_id, job_type, params_json, now, now),
            )
            c.commit()
        return job_id

    def get_job(self, job_id: str) -> JobRow | None:
        with _schema_lock:
            cur = self._conn.execute("SELECT * FROM jobs WHERE id = ?", (job_id,))
            row = cur.fetchone()
        if row is None:
            return None
        return JobRow(
            id=row["id"],
            job_type=row["job_type"],
            status=row["status"],
            params_json=row["params_json"],
            error=row["error"],
            created_at=row["created_at"],
            updated_at=row["updated_at"],
            started_at=row["started_at"],
            completed_at=row["completed_at"],
        )

    def set_status(
        self,
        job_id: str,
        status: str,
        *,
        error: str | None = None,
        started: bool = False,
        completed: bool = False,
    ) -> None:
        import time as time_mod

        now = time_mod.time()
        with self._write() as c:
            if started:
                c.execute(
                    "UPDATE jobs SET status = ?, error = ?, updated_at = ?, started_at = ? WHERE id = ?",
                    (status, error, now, now, job_id),
                )
            elif completed:
                c.execute(
                    "UPDATE jobs SET status = ?, error = ?, updated_at = ?, completed_at = ? WHERE id = ?",
                    (status, error, now, now, job_id),
                )
            else:
                c.execute(
                    "UPDATE jobs SET status = ?, error = ?, updated_at = ? WHERE id = ?",
                    (status, error, now, job_id),
                )
            c.commit()

    def fetch_next_queued(self) -> str | None:
        """Atomically marca um job queued como running e devolve o id."""
        import time as time_mod

        now = time_mod.time()
        with self._write() as c:
            cur = c.execute("SELECT id FROM jobs WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1")
            row = cur.fetchone()
            if row is None:
                return None
            job_id = row["id"]
            c.execute(
                "UPDATE jobs SET status = 'running', updated_at = ?, started_at = ? WHERE id = ?",
                (now, now, job_id),
            )
            c.commit()
        return job_id

    def list_stale_completed(self, older_than_ts: float) -> list[tuple[str, str]]:
        """Jobs succeeded/failed com completed_at < threshold. Devolve (id, status)."""
        with _schema_lock:
            cur = self._conn.execute(
                """
                SELECT id, status FROM jobs
                WHERE status IN ('succeeded', 'failed')
                  AND completed_at IS NOT NULL
                  AND completed_at < ?
                """,
                (older_than_ts,),
            )
            return [(r["id"], r["status"]) for r in cur.fetchall()]

    def delete_job(self, job_id: str) -> None:
        with self._write() as c:
            c.execute("DELETE FROM jobs WHERE id = ?", (job_id,))
            c.commit()
