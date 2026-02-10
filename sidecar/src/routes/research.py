"""Research discovery routes — paper search and technique extraction."""

from __future__ import annotations

import json

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from src.models.paper import PaperMeta, SearchRequest, TechniqueCard
from src.services.claude_client import get_claude_client
from src.services.paper_search import search_all_sources
from src.services.pdf_extractor import extract_pdf_text

router = APIRouter()


class ExtractTechniqueRequest(BaseModel):
    """Request body for technique extraction."""

    pdf_url: str
    paper_id: str
    paper_title: str
    project_summary: str
    user_request: str


@router.post("/search-papers", response_model=list[PaperMeta])
async def search_papers(req: SearchRequest) -> list[PaperMeta]:
    """Search for academic papers across multiple sources."""
    papers = await search_all_sources(
        queries=req.queries,
        max_results=req.max_results,
        year_min=req.year_min,
        year_max=req.year_max,
        prefer_open_access=req.prefer_open_access,
    )
    return papers


@router.post("/extract-technique", response_model=TechniqueCard)
async def extract_technique(req: ExtractTechniqueRequest) -> TechniqueCard:
    """Extract a technique card from a paper PDF."""
    # Download and extract text from the PDF.
    pdf_text = await extract_pdf_text(req.pdf_url)

    # Use Claude to extract a structured technique card.
    claude = get_claude_client()
    if not claude:
        raise HTTPException(
            status_code=503,
            detail="Claude API client not available. Set ANTHROPIC_API_KEY env var.",
        )

    prompt = f"""You are analyzing an academic paper to extract a specific technique that can be applied to a software project.

Paper title: {req.paper_title}
Paper ID: {req.paper_id}

Project context: {req.project_summary}
User's goal: {req.user_request}

Paper content (extracted from PDF):
{pdf_text[:15000]}

Extract the most relevant technique from this paper and respond in JSON:
{{
  "name": "Human-readable technique name",
  "paper_id": "{req.paper_id}",
  "paper_title": "{req.paper_title}",
  "methodology": "Detailed description of the methodology (3-5 sentences)",
  "key_components": ["Component 1", "Component 2", ...],
  "required_data_format": "What input data format this technique needs",
  "implementation_complexity": "Low|Medium|High",
  "hardware_requirements": "CPU/GPU/memory requirements",
  "dependencies": ["library1", "library2"],
  "relevance_score": 0.0-1.0,
  "integration_approach": "Specific plan for integrating into the user's project"
}}"""

    result = await claude.analyze(prompt)

    parsed = json.loads(result)
    return TechniqueCard(**parsed)
