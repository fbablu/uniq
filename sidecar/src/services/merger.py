"""Variant merge service — combines two variants with configurable blend ratios."""

from __future__ import annotations

import json
import logging
import subprocess
from pathlib import Path
from typing import Any

from src.models.project import ProjectProfile
from src.models.variant import VariantResult
from src.services.claude_client import get_claude_client

logger = logging.getLogger(__name__)

BLEND_DESCRIPTIONS = {
    0: "not directly used, but its insights inform the design",
    25: "contributes a minor sub-component or enhancement",
    50: "equal architectural weight in a true hybrid approach",
    75: "serves as the primary architecture",
    100: "fully implemented as the core approach",
}


async def merge_variant_code(
    variant_a_branch: str,
    variant_a_technique: Any,
    variant_b_branch: str,
    variant_b_technique: Any,
    blend_a: int,
    blend_b: int,
    project: ProjectProfile,
    target_branch: str,
) -> VariantResult:
    """Merge two variants by having Claude create a hybrid implementation.

    This is NOT a git merge — it's a semantic merge where Claude reads both
    variant codebases and creates a new hybrid implementation based on the
    specified blend ratios.
    """
    claude = get_claude_client()
    if not claude:
        raise RuntimeError("Claude API client not available. Set ANTHROPIC_API_KEY.")

    project_path = Path(project.path)

    # Read the code from both variant branches.
    code_a = _read_branch_code(project_path, variant_a_branch)
    code_b = _read_branch_code(project_path, variant_b_branch)

    # Create the target branch from the original (non-variant) branch.
    # Determine the base branch (usually main or master).
    base_branch = _get_default_branch(project_path)

    try:
        subprocess.run(
            ["git", "checkout", base_branch],
            cwd=project_path,
            check=True,
            capture_output=True,
        )
        subprocess.run(
            ["git", "checkout", "-b", target_branch],
            cwd=project_path,
            check=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as e:
        raise RuntimeError(f"Failed to create merge branch: {e.stderr.decode()}") from e

    try:
        blend_a_desc = BLEND_DESCRIPTIONS.get(blend_a, f"{blend_a}% integration")
        blend_b_desc = BLEND_DESCRIPTIONS.get(blend_b, f"{blend_b}% integration")

        technique_a_str = (
            json.dumps(variant_a_technique, indent=2)
            if isinstance(variant_a_technique, dict)
            else str(variant_a_technique)
        )
        technique_b_str = (
            json.dumps(variant_b_technique, indent=2)
            if isinstance(variant_b_technique, dict)
            else str(variant_b_technique)
        )

        system_prompt = f"""You are an expert software engineer creating a hybrid implementation that merges two different AI techniques into a single codebase.

Project details:
- Languages: {", ".join(project.languages)}
- Summary: {project.summary}
- User's goal: {project.user_request}

You must create a NEW implementation that combines both techniques according to the specified blend ratios. This is a semantic merge — not a mechanical merge of code.

Generate file modifications as a JSON array:
{{
  "files": [
    {{"path": "...", "content": "...", "action": "create|modify"}}
  ],
  "dependencies": ["lib1", "lib2"],
  "merge_summary": "How the two techniques were combined"
}}"""

        user_prompt = f"""Create a hybrid implementation combining these two techniques:

=== TECHNIQUE A (blend: {blend_a}% — {blend_a_desc}) ===
{technique_a_str}

Code from variant A branch ({variant_a_branch}):
{code_a[:8000]}

=== TECHNIQUE B (blend: {blend_b}% — {blend_b_desc}) ===
{technique_b_str}

Code from variant B branch ({variant_b_branch}):
{code_b[:8000]}

=== MERGE INSTRUCTIONS ===
- Technique A should be {blend_a_desc} ({blend_a}%)
- Technique B should be {blend_b_desc} ({blend_b}%)
- Create a cohesive implementation that intelligently combines both approaches
- Resolve any conflicts between the two techniques
- Ensure the merged code is functional and well-structured

Generate the merged implementation now."""

        result_text = await claude.generate_code(system_prompt, user_prompt)

        # Parse JSON response.
        if "```json" in result_text:
            result_text = result_text.split("```json")[1].split("```")[0]
        elif "```" in result_text:
            result_text = result_text.split("```")[1].split("```")[0]

        result_data = json.loads(result_text)

        modified_files = []
        new_dependencies = result_data.get("dependencies", [])

        # Apply file changes.
        for file_change in result_data.get("files", []):
            file_path = project_path / file_change["path"]
            file_path.parent.mkdir(parents=True, exist_ok=True)
            file_path.write_text(file_change["content"])
            modified_files.append(file_change["path"])

        # Write TECHNIQUE.md for the merge.
        merge_summary = result_data.get("merge_summary", "Hybrid merge of two techniques")
        technique_md = f"""# Merged Variant

**Source A:** {variant_a_branch} ({blend_a}%)
**Source B:** {variant_b_branch} ({blend_b}%)

## Merge Summary

{merge_summary}

## Blend Ratios

- **A ({blend_a}%):** {blend_a_desc}
- **B ({blend_b}%):** {blend_b_desc}

## New Dependencies

{chr(10).join(f"- {d}" for d in new_dependencies)}

## Modified Files

{chr(10).join(f"- {f}" for f in modified_files)}
"""
        (project_path / "TECHNIQUE.md").write_text(technique_md)
        modified_files.append("TECHNIQUE.md")

        # Git add and commit.
        subprocess.run(
            ["git", "add", "-A"],
            cwd=project_path,
            check=True,
            capture_output=True,
        )
        subprocess.run(
            [
                "git",
                "commit",
                "-m",
                f"uniq: Merge {variant_a_branch} ({blend_a}%) + {variant_b_branch} ({blend_b}%)",
            ],
            cwd=project_path,
            check=True,
            capture_output=True,
        )

        # Switch back.
        subprocess.run(
            ["git", "checkout", base_branch],
            cwd=project_path,
            check=True,
            capture_output=True,
        )

        return VariantResult(
            success=True,
            modified_files=modified_files,
            new_dependencies=new_dependencies,
        )

    except Exception as e:
        # Attempt recovery.
        try:
            base = _get_default_branch(project_path)
            subprocess.run(
                ["git", "checkout", base],
                cwd=project_path,
                capture_output=True,
            )
        except Exception:
            pass
        logger.error(f"Merge failed: {e}")
        return VariantResult(success=False, error=str(e))


def _read_branch_code(project_path: Path, branch_name: str) -> str:
    """Read the diff of a branch compared to the default branch."""
    try:
        base = _get_default_branch(project_path)
        result = subprocess.run(
            ["git", "diff", f"{base}...{branch_name}"],
            cwd=project_path,
            capture_output=True,
            text=True,
        )
        return result.stdout
    except Exception as e:
        logger.error(f"Failed to read branch code for {branch_name}: {e}")
        return ""


def _get_default_branch(project_path: Path) -> str:
    """Detect the default branch name (main or master)."""
    try:
        result = subprocess.run(
            ["git", "symbolic-ref", "refs/remotes/origin/HEAD"],
            cwd=project_path,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return result.stdout.strip().split("/")[-1]
    except Exception:
        pass

    # Fallback: check if main exists, then master.
    for branch in ("main", "master"):
        result = subprocess.run(
            ["git", "rev-parse", "--verify", branch],
            cwd=project_path,
            capture_output=True,
        )
        if result.returncode == 0:
            return branch

    return "main"
