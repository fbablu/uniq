"""Paper and technique-related data models."""

from __future__ import annotations

from enum import Enum

from pydantic import BaseModel


class PaperSource(str, Enum):
    SEMANTIC_SCHOLAR = "SemanticScholar"
    ARXIV = "ArXiv"


class PaperMeta(BaseModel):
    id: str
    title: str
    authors: list[str]
    year: int | None = None
    published_date: str | None = None
    abstract_text: str
    citation_count: int | None = None
    url: str
    pdf_url: str | None = None
    source: PaperSource
    fields: list[str] = []
    relevance_score: float | None = None


class SearchRequest(BaseModel):
    queries: list[str]
    max_results: int = 200
    year_min: int | None = None
    year_max: int | None = None
    prefer_open_access: bool = True


class TechniqueCard(BaseModel):
    name: str
    paper_id: str
    paper_title: str
    methodology: str
    key_components: list[str]
    required_data_format: str
    implementation_complexity: str  # "Low", "Medium", "High"
    hardware_requirements: str
    dependencies: list[str]
    relevance_score: float
    integration_approach: str
    selected: bool = False
