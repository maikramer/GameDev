from __future__ import annotations

from functools import lru_cache
from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="INFERENCE_SERVER_",
        env_file=".env",
        extra="ignore",
    )

    host: str = Field(default="0.0.0.0", description="Bind HTTP")
    port: int = Field(default=8765, ge=1, le=65535)
    api_key: str | None = Field(
        default=None,
        description="Se definido, exige Authorization: Bearer <valor>",
    )
    data_dir: Path = Field(
        default=Path.home() / ".cache" / "inference_server",
        description="SQLite + pasta artifacts/",
    )
    job_ttl_seconds: int = Field(default=7 * 24 * 3600, ge=60, description="Remover jobs terminados após N s")
    cleanup_interval_seconds: int = Field(default=3600, ge=60)
    worker_poll_seconds: float = Field(default=0.25, gt=0)


@lru_cache(maxsize=1)
def get_settings() -> Settings:
    return Settings()
