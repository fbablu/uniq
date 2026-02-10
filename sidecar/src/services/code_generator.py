"""Code generation service — applies a research technique to a project."""

from __future__ import annotations

import contextlib
import json
import logging
import subprocess
from pathlib import Path

from src.models.paper import TechniqueCard
from src.models.project import ProjectProfile
from src.models.variant import VariantResult
from src.services.claude_client import get_claude_client

logger = logging.getLogger(__name__)


async def generate_variant_code(
    technique: TechniqueCard,
    project: ProjectProfile,
    branch_name: str,
) -> VariantResult:
    """Generate a project variant by applying a technique using Claude.

    1. Create a git branch.
    2. Ask Claude to generate the code modifications.
    3. Apply the modifications to the branch.
    4. Commit the changes.
    """
    claude = get_claude_client()
    if not claude:
        raise RuntimeError("Claude API client not available. Set ANTHROPIC_API_KEY.")

    project_path = Path(project.path)

    # Create a new git branch.
    try:
        subprocess.run(
            ["git", "checkout", "-b", branch_name],
            cwd=project_path,
            check=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as e:
        raise RuntimeError(f"Failed to create branch {branch_name}: {e.stderr.decode()}") from e

    try:
        # Build the code generation prompt.
        system_prompt = f"""You are an expert software engineer implementing a research technique into an existing codebase.

Project details:
- Languages: {", ".join(project.languages)}
- Summary: {project.summary}
- File tree:
{project.file_tree[:3000]}

Your task is to implement the research technique described below into this project.
Generate file modifications as a JSON array of objects, each with:
- "path": relative file path (create new files or modify existing ones)
- "content": the complete file content
- "action": "create" or "modify"

Also list any new dependencies needed.

Respond ONLY in JSON format:
{{
  "files": [
    {{"path": "...", "content": "...", "action": "create|modify"}}
  ],
  "dependencies": ["lib1", "lib2"],
  "technique_summary": "Brief description of what was implemented"
}}"""

        user_prompt = f"""Implement the following research technique:

Technique: {technique.name}
Paper: {technique.paper_title} ({technique.paper_id})

Methodology:
{technique.methodology}

Key Components: {", ".join(technique.key_components)}

Required Data Format: {technique.required_data_format}

Integration Approach:
{technique.integration_approach}

User's Goal: {project.user_request}

Generate the implementation now."""

        result_text = await claude.generate_code(system_prompt, user_prompt)

        # Parse the JSON response.
        # Try to extract JSON if wrapped in markdown.
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

        # Write a TECHNIQUE.md file.
        technique_md = f"""# {technique.name}

**Source Paper:** {technique.paper_title}
**Paper ID:** {technique.paper_id}
**Relevance Score:** {technique.relevance_score:.0%}

## Methodology

{technique.methodology}

## Key Components

{chr(10).join(f"- {c}" for c in technique.key_components)}

## Integration Approach

{technique.integration_approach}

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
                f"uniq: Apply technique '{technique.name}' from {technique.paper_id}",
            ],
            cwd=project_path,
            check=True,
            capture_output=True,
        )

        # Switch back to the original branch.
        subprocess.run(
            ["git", "checkout", "-"],
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
        # Attempt to switch back to the original branch on failure.
        with contextlib.suppress(Exception):
            subprocess.run(
                ["git", "checkout", "-"],
                cwd=project_path,
                capture_output=True,
            )
        logger.error(f"Variant generation failed: {e}")
        return VariantResult(success=False, error=str(e))
