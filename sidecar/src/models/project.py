"""Project-related data models."""

from __future__ import annotations

from pydantic import BaseModel


class AnalyzeProjectRequest(BaseModel):
    path: str
    description: str


class DetectedFramework(BaseModel):
    name: str
    version: str | None = None
    category: str = "Other"


class IntegrationPoint(BaseModel):
    file_path: str
    description: str
    suggested_approach: str
    complexity: str = "Medium"


class ProjectProfile(BaseModel):
    path: str
    user_request: str
    summary: str
    languages: list[str]
    frameworks: list[DetectedFramework]
    file_count: int
    key_files: list[str]
    integration_points: list[IntegrationPoint]
    file_tree: str
