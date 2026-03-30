from __future__ import annotations

import logging
import os
import sys

logging.basicConfig(
    level=os.environ.get("INFERENCE_SERVER_LOG_LEVEL", "INFO"),
    format="%(asctime)s %(levelname)s %(name)s %(message)s",
)


def main() -> None:
    import uvicorn

    from inference_server.config import get_settings

    s = get_settings()
    uvicorn.run(
        "inference_server.main:app",
        host=s.host,
        port=s.port,
        factory=False,
        log_level=os.environ.get("UVICORN_LOG_LEVEL", "info"),
    )


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
