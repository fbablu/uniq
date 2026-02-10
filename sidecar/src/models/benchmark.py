"""Benchmark-related data models."""

from __future__ import annotations

from pydantic import BaseModel


class ExecutionMetrics(BaseModel):
    build_success: bool
    build_error: str | None = None
    test_pass_rate: float | None = None
    tests_passed: int | None = None
    tests_total: int | None = None
    runtime_ms: float | None = None
    memory_mb: float | None = None
    custom_metrics: dict[str, float] = {}


class JudgeScores(BaseModel):
    code_quality: float
    novelty: float
    feasibility: float
    goal_alignment: float
    completeness: float
    overall: float
    explanation: str


class BenchmarkRequest(BaseModel):
    variant_branches: list[str]
    project_path: str
    metrics: list[str] = []
    timeout_seconds: int = 300


class BenchmarkResult(BaseModel):
    results: dict[str, ExecutionMetrics]
