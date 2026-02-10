"""Merge-related data models."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel

from src.models.project import ProjectProfile


class MergeRequest(BaseModel):
    variant_a_branch: str
    variant_a_technique: Any  # TechniqueCard or merge lineage
    variant_b_branch: str
    variant_b_technique: Any
    blend_a: int  # 0, 25, 50, 75, 100
    blend_b: int
    project: ProjectProfile
    target_branch: str
