"""Benchmark routes."""

from __future__ import annotations

from fastapi import APIRouter
from pydantic import BaseModel

from src.models.benchmark import BenchmarkRequest, BenchmarkResult, JudgeScores
from src.services.benchmarker import run_benchmarks, run_llm_judge

router = APIRouter()


class LlmJudgeRequest(BaseModel):
    """Request body for LLM judge evaluation."""

    variant_branches: list[str]
    project_path: str
    user_request: str


@router.post("/run-benchmark", response_model=BenchmarkResult)
async def benchmark(req: BenchmarkRequest) -> BenchmarkResult:
    """Run automated benchmarks on variant branches."""
    results = await run_benchmarks(
        variant_branches=req.variant_branches,
        project_path=req.project_path,
        metrics=req.metrics,
        timeout_seconds=req.timeout_seconds,
    )
    return BenchmarkResult(results=results)


@router.post("/llm-judge", response_model=dict[str, JudgeScores])
async def llm_judge(req: LlmJudgeRequest) -> dict[str, JudgeScores]:
    """Run LLM-as-judge evaluation on variants."""
    scores = await run_llm_judge(
        variant_branches=req.variant_branches,
        project_path=req.project_path,
        user_request=req.user_request,
    )
    return scores
