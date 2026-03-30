from __future__ import annotations

from fastapi.testclient import TestClient
from inference_server.main import app


def test_health() -> None:
    with TestClient(app) as client:
        r = client.get("/health")
        assert r.status_code == 200
        assert r.json() == {"status": "ok"}


def test_version() -> None:
    with TestClient(app) as client:
        r = client.get("/version")
        assert r.status_code == 200
        data = r.json()
        assert "inference_server_version" in data
        assert "job_types" in data
        assert isinstance(data["job_types"], list)
