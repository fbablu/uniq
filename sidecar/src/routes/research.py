"""Research discovery routes — paper search and technique extraction."""

from __future__ import annotations

import json
import logging
import re

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ValidationError

from src.models.paper import PaperMeta, SearchRequest, TechniqueCard
from src.services.claude_client import get_claude_client
from src.services.paper_search import search_all_sources
from src.services.pdf_extractor import extract_pdf_text

logger = logging.getLogger(__name__)

router = APIRouter()


class ExtractTechniqueRequest(BaseModel):
    """Request body for technique extraction."""

    pdf_url: str | None = None
    paper_id: str
    paper_title: str
    project_summary: str
    user_request: str
    doi: str | None = None


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
    # Validate that we have at least one way to get the PDF.
    if not req.pdf_url and not req.doi:
        raise HTTPException(
            status_code=400,
            detail="At least one of pdf_url or doi must be provided.",
        )

    # Download and extract text from the PDF (with fallback sources).
    try:
        pdf_text = await extract_pdf_text(req.pdf_url, doi=req.doi)
    except RuntimeError as e:
        raise HTTPException(
            status_code=502,
            detail=f"Failed to download/extract PDF: {e}",
        )

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

Extract the most relevant technique from this paper and respond with ONLY a JSON object (no markdown, no explanation):
{{
  "name": "Human-readable technique name",
  "paper_id": "{req.paper_id}",
  "paper_title": "{req.paper_title}",
  "methodology": "Detailed description of the methodology (3-5 sentences)",
  "key_components": ["Component 1", "Component 2"],
  "required_data_format": "What input data format this technique needs",
  "implementation_complexity": "Low or Medium or High",
  "hardware_requirements": "CPU/GPU/memory requirements",
  "dependencies": ["library1", "library2"],
  "relevance_score": 0.85,
  "integration_approach": "Specific plan for integrating into the user's project"
}}"""

    try:
        result = await claude.analyze(prompt)
    except Exception as e:
        logger.error(f"Claude API call failed for paper {req.paper_id}: {e}")
        raise HTTPException(
            status_code=502,
            detail=f"Claude API call failed: {e}",
        )

    # Robust JSON extraction: try direct parse, then find JSON in text.
    parsed = None
    try:
        parsed = json.loads(result)
    except json.JSONDecodeError:
        # Try to extract JSON from the response (Claude sometimes wraps it in text).
        json_match = re.search(r"\{[\s\S]*\}", result)
        if json_match:
            try:
                parsed = json.loads(json_match.group())
            except json.JSONDecodeError:
                pass

    if parsed is None:
        logger.error(f"Failed to parse Claude response as JSON for paper {req.paper_id}")
        logger.debug(f"Raw Claude response: {result[:500]}")
        raise HTTPException(
            status_code=502,
            detail="Failed to parse technique extraction result as JSON.",
        )

    # Ensure required fields have fallbacks.
    parsed.setdefault("paper_id", req.paper_id)
    parsed.setdefault("paper_title", req.paper_title)
    parsed.setdefault("selected", False)

    # Validate relevance_score is a float.
    try:
        parsed["relevance_score"] = float(parsed.get("relevance_score", 0.5))
    except (TypeError, ValueError):
        parsed["relevance_score"] = 0.5

    try:
        return TechniqueCard(**parsed)
    except ValidationError as e:
        logger.error(f"TechniqueCard validation failed: {e}")
        raise HTTPException(
            status_code=502,
            detail=f"Technique card validation failed: {e}",
        )
