"""FastAPI server for the uniq sidecar.

Launched by the Rust TUI via: uv run --project ./sidecar python -m src.server --port {port}
"""

from __future__ import annotations

import argparse
import asyncio
import os
import signal

import uvicorn
from fastapi import FastAPI

from src.routes.benchmark import router as benchmark_router
from src.routes.generate import router as generate_router
from src.routes.merge import router as merge_router
from src.routes.project import router as project_router
from src.routes.research import router as research_router

app = FastAPI(
    title="uniq-sidecar",
    version="0.1.0",
    description="Python sidecar for uniq research engine",
)

# Register route modules.
app.include_router(project_router, prefix="/api")
app.include_router(research_router, prefix="/api")
app.include_router(generate_router, prefix="/api")
app.include_router(merge_router, prefix="/api")
app.include_router(benchmark_router, prefix="/api")


@app.get("/api/health")
async def health():
    """Health check endpoint."""
    return {"status": "ok", "version": "0.1.0"}


@app.post("/api/shutdown")
async def shutdown():
    """Graceful shutdown endpoint."""
    # Schedule shutdown after responding.
    asyncio.get_event_loop().call_later(0.5, lambda: os.kill(os.getpid(), signal.SIGTERM))
    return {"status": "shutting_down"}


def main():
    parser = argparse.ArgumentParser(description="uniq sidecar server")
    parser.add_argument("--port", type=int, required=True, help="Port to listen on")
    parser.add_argument("--host", type=str, default="127.0.0.1", help="Host to bind to")
    args = parser.parse_args()

    # Print ready signal for the Rust side.
    print(f"SIDECAR_READY port={args.port}", flush=True)

    uvicorn.run(
        app,
        host=args.host,
        port=args.port,
        log_level="info",
        access_log=False,
    )


if __name__ == "__main__":
    main()
