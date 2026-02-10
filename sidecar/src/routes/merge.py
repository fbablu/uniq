"""Variant merge routes."""

from __future__ import annotations

from fastapi import APIRouter

from src.models.merge import MergeRequest
from src.models.variant import VariantResult
from src.services.merger import merge_variant_code

router = APIRouter()


@router.post("/merge-variants", response_model=VariantResult)
async def merge_variants(req: MergeRequest) -> VariantResult:
    """Merge two variants with specified blend ratios."""
    try:
        result = await merge_variant_code(
            variant_a_branch=req.variant_a_branch,
            variant_a_technique=req.variant_a_technique,
            variant_b_branch=req.variant_b_branch,
            variant_b_technique=req.variant_b_technique,
            blend_a=req.blend_a,
            blend_b=req.blend_b,
            project=req.project,
            target_branch=req.target_branch,
        )
        return result
    except Exception as e:
        return VariantResult(
            success=False,
            error=str(e),
        )
