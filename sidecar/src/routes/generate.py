"""Variant generation routes."""

from __future__ import annotations

from fastapi import APIRouter

from src.models.variant import GenerateVariantRequest, VariantResult
from src.services.code_generator import generate_variant_code

router = APIRouter()


@router.post("/generate-variant", response_model=VariantResult)
async def generate_variant(req: GenerateVariantRequest) -> VariantResult:
    """Generate a project variant by applying a technique."""
    try:
        result = await generate_variant_code(
            technique=req.technique,
            project=req.project,
            branch_name=req.branch_name,
        )
        return result
    except Exception as e:
        return VariantResult(
            success=False,
            error=str(e),
        )
