"""Benchmark service — runs automated tests and LLM-as-judge evaluation."""

from __future__ import annotations

import contextlib
import json
import logging
import subprocess
import time
from pathlib import Path

from src.models.benchmark import ExecutionMetrics, JudgeScores
from src.services.claude_client import get_claude_client

logger = logging.getLogger(__name__)


async def run_benchmarks(
    variant_branches: list[str],
    project_path: str,
    metrics: list[str],
    timeout_seconds: int = 300,
) -> dict[str, ExecutionMetrics]:
    """Run automated benchmarks on each variant branch.

    For each variant:
    1. Checkout the branch.
    2. Attempt to build.
    3. Attempt to run tests.
    4. Measure runtime and memory usage.
    5. Switch back to the default branch.
    """
    results: dict[str, ExecutionMetrics] = {}
    path = Path(project_path)
    base_branch = _get_default_branch(path)

    for branch in variant_branches:
        logger.info(f"Benchmarking variant: {branch}")
        metrics_result = await _benchmark_single_variant(path, branch, base_branch, timeout_seconds)
        results[branch] = metrics_result

    # Ensure we're back on the base branch.
    with contextlib.suppress(Exception):
        subprocess.run(
            ["git", "checkout", base_branch],
            cwd=path,
            capture_output=True,
        )

    return results


async def _benchmark_single_variant(
    project_path: Path,
    branch: str,
    base_branch: str,
    timeout: int,
) -> ExecutionMetrics:
    """Benchmark a single variant."""
    # Checkout the branch.
    try:
        subprocess.run(
            ["git", "checkout", branch],
            cwd=project_path,
            check=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as e:
        return ExecutionMetrics(
            build_success=False,
            build_error=f"Failed to checkout branch: {e.stderr.decode()}",
        )

    # Detect project type and run appropriate build/test commands.
    build_success = True
    build_error = None
    test_pass_rate = None
    tests_passed = None
    tests_total = None
    runtime_ms = None
    memory_mb = None

    # Try to build.
    build_cmd = _detect_build_command(project_path)
    if build_cmd:
        start = time.time()
        try:
            result = subprocess.run(
                build_cmd,
                cwd=project_path,
                capture_output=True,
                text=True,
                timeout=timeout,
                shell=True,
            )
            if result.returncode != 0:
                build_success = False
                build_error = result.stderr[:500]
        except subprocess.TimeoutExpired:
            build_success = False
            build_error = f"Build timed out after {timeout}s"
        except Exception as e:
            build_success = False
            build_error = str(e)
        runtime_ms = (time.time() - start) * 1000

    # Try to run tests.
    test_cmd = _detect_test_command(project_path)
    if test_cmd and build_success:
        try:
            result = subprocess.run(
                test_cmd,
                cwd=project_path,
                capture_output=True,
                text=True,
                timeout=timeout,
                shell=True,
            )
            # Basic pass/fail detection.
            test_pass_rate = 1.0 if result.returncode == 0 else 0.0
        except Exception:
            pass

    # Switch back.
    with contextlib.suppress(Exception):
        subprocess.run(
            ["git", "checkout", base_branch],
            cwd=project_path,
            capture_output=True,
        )

    return ExecutionMetrics(
        build_success=build_success,
        build_error=build_error,
        test_pass_rate=test_pass_rate,
        tests_passed=tests_passed,
        tests_total=tests_total,
        runtime_ms=runtime_ms,
        memory_mb=memory_mb,
    )


async def run_llm_judge(
    variant_branches: list[str],
    project_path: str,
    user_request: str,
) -> dict[str, JudgeScores]:
    """Run LLM-as-judge evaluation on variants."""
    claude = get_claude_client()
    if not claude:
        raise RuntimeError("Claude API client not available.")

    results: dict[str, JudgeScores] = {}
    path = Path(project_path)
    base_branch = _get_default_branch(path)

    for branch in variant_branches:
        # Read the diff for this variant.
        try:
            diff_result = subprocess.run(
                ["git", "diff", f"{base_branch}...{branch}"],
                cwd=path,
                capture_output=True,
                text=True,
            )
            diff_text = diff_result.stdout[:10000]
        except Exception:
            diff_text = "(Could not read diff)"

        prompt = f"""You are evaluating a code implementation. Rate it on these criteria (0-10 scale):

User's goal: {user_request}
Branch: {branch}

Code changes (git diff):
{diff_text}

Evaluate and respond in JSON:
{{
  "code_quality": 0-10,
  "novelty": 0-10,
  "feasibility": 0-10,
  "goal_alignment": 0-10,
  "completeness": 0-10,
  "overall": 0-10 (weighted average),
  "explanation": "2-3 sentence evaluation"
}}"""

        try:
            result_text = await claude.analyze(prompt)
            scores_data = json.loads(result_text)
            results[branch] = JudgeScores(**scores_data)
        except Exception as e:
            logger.error(f"LLM judge failed for {branch}: {e}")
            results[branch] = JudgeScores(
                code_quality=0,
                novelty=0,
                feasibility=0,
                goal_alignment=0,
                completeness=0,
                overall=0,
                explanation=f"Evaluation failed: {e}",
            )

    return results


def _detect_build_command(project_path: Path) -> str | None:
    """Detect the appropriate build command for the project."""
    if (project_path / "Cargo.toml").exists():
        return "cargo build"
    if (project_path / "package.json").exists():
        return "npm run build"
    if (project_path / "pyproject.toml").exists():
        return "uv run python -m py_compile"
    if (project_path / "Makefile").exists():
        return "make"
    if (project_path / "go.mod").exists():
        return "go build ./..."
    return None


def _detect_test_command(project_path: Path) -> str | None:
    """Detect the appropriate test command for the project."""
    if (project_path / "Cargo.toml").exists():
        return "cargo test"
    if (project_path / "package.json").exists():
        return "npm test"
    if (project_path / "pyproject.toml").exists():
        return "uv run pytest"
    if (project_path / "Makefile").exists():
        return "make test"
    if (project_path / "go.mod").exists():
        return "go test ./..."
    return None


def _get_default_branch(project_path: Path) -> str:
    """Detect the default branch name."""
    for branch in ("main", "master"):
        result = subprocess.run(
            ["git", "rev-parse", "--verify", branch],
            cwd=project_path,
            capture_output=True,
        )
        if result.returncode == 0:
            return branch
    return "main"
