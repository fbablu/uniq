"""Variant-related data models."""

from __future__ import annotations

from pydantic import BaseModel

from src.models.paper import TechniqueCard
from src.models.project import ProjectProfile


class GenerateVariantRequest(BaseModel):
    technique: TechniqueCard
    project: ProjectProfile
    branch_name: str


class VariantResult(BaseModel):
    success: bool
    modified_files: list[str] = []
    new_dependencies: list[str] = []
    error: str | None = None
