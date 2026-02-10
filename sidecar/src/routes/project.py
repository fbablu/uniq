"""Project analysis routes."""

from __future__ import annotations

from fastapi import APIRouter, HTTPException

from src.models.project import AnalyzeProjectRequest, ProjectProfile
from src.services.claude_client import get_claude_client

router = APIRouter()


@router.post("/analyze-project", response_model=ProjectProfile)
async def analyze_project(req: AnalyzeProjectRequest) -> ProjectProfile:
    """Analyze a project directory and return a structured profile."""
    import os
    from pathlib import Path

    project_path = Path(req.path)
    if not project_path.exists():
        raise HTTPException(status_code=404, detail=f"Project path does not exist: {req.path}")
    if not project_path.is_dir():
        raise HTTPException(status_code=400, detail=f"Path is not a directory: {req.path}")

    # Scan the project directory.
    languages: list[str] = []
    file_count = 0
    key_files: list[str] = []
    file_tree_lines: list[str] = []

    # Language detection by extension.
    extension_map = {
        ".py": "Python",
        ".rs": "Rust",
        ".ts": "TypeScript",
        ".tsx": "TypeScript",
        ".js": "JavaScript",
        ".jsx": "JavaScript",
        ".go": "Go",
        ".java": "Java",
        ".cs": "CSharp",
        ".cpp": "Cpp",
        ".c": "C",
        ".rb": "Ruby",
        ".swift": "Swift",
        ".kt": "Kotlin",
    }

    # Key file patterns.
    key_patterns = {
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "pom.xml",
        "Makefile",
        "Dockerfile",
        "docker-compose.yml",
        "README.md",
        "main.py",
        "main.rs",
        "index.ts",
        "index.js",
        "app.py",
        "manage.py",
    }

    detected_langs: set[str] = set()
    max_depth = 4
    max_files_tree = 200

    for root, dirs, files in os.walk(project_path):
        # Skip hidden dirs, node_modules, target, etc.
        dirs[:] = [
            d
            for d in dirs
            if not d.startswith(".")
            and d not in ("node_modules", "target", "__pycache__", "venv", ".venv", "dist", "build")
        ]

        rel_root = os.path.relpath(root, project_path)
        depth = rel_root.count(os.sep) if rel_root != "." else 0

        if depth < max_depth and file_count < max_files_tree:
            indent = "  " * depth
            dir_name = os.path.basename(root) if rel_root != "." else "."
            file_tree_lines.append(f"{indent}{dir_name}/")

        for f in files:
            file_count += 1
            ext = os.path.splitext(f)[1]
            if ext in extension_map:
                detected_langs.add(extension_map[ext])

            if f in key_patterns:
                key_files.append(os.path.join(rel_root, f))

            if depth < max_depth and file_count < max_files_tree:
                indent = "  " * (depth + 1)
                file_tree_lines.append(f"{indent}{f}")

    languages = sorted(detected_langs)
    file_tree = "\n".join(file_tree_lines[:max_files_tree])

    # Use Claude to generate a summary and identify integration points.
    claude = get_claude_client()
    summary = ""
    integration_points = []

    if claude:
        prompt = f"""Analyze this project and provide:
1. A concise summary (2-3 sentences) of what this project does
2. Identify 2-5 specific integration points where the following AI capability could be added: "{req.description}"

Project path: {req.path}
Languages: {", ".join(languages)}
Key files: {", ".join(key_files)}
File tree:
{file_tree[:3000]}

Respond in JSON format:
{{
  "summary": "...",
  "integration_points": [
    {{
      "file_path": "relative/path",
      "description": "what this integration point is",
      "suggested_approach": "how to integrate AI here",
      "complexity": "Low|Medium|High"
    }}
  ]
}}"""

        try:
            result = await claude.analyze(prompt)
            import json

            parsed = json.loads(result)
            summary = parsed.get("summary", "")
            integration_points = [
                {
                    "file_path": ip.get("file_path", ""),
                    "description": ip.get("description", ""),
                    "suggested_approach": ip.get("suggested_approach", ""),
                    "complexity": ip.get("complexity", "Medium"),
                }
                for ip in parsed.get("integration_points", [])
            ]
        except Exception:
            summary = f"Project with {file_count} files in {', '.join(languages)}."
    else:
        summary = (
            f"Project with {file_count} files in {', '.join(languages) or 'unknown language'}."
        )

    return ProjectProfile(
        path=req.path,
        user_request=req.description,
        summary=summary,
        languages=languages,
        frameworks=[],
        file_count=file_count,
        key_files=key_files,
        integration_points=integration_points,
        file_tree=file_tree,
    )
