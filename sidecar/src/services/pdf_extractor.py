"""PDF text extraction using PyMuPDF."""

from __future__ import annotations

import logging

import httpx
import pymupdf

logger = logging.getLogger(__name__)


async def extract_pdf_text(pdf_url: str, max_pages: int = 30) -> str:
    """Download a PDF from a URL and extract text content.

    Uses PyMuPDF for fast, layout-aware text extraction.
    Returns Markdown-formatted text.
    """
    # Download the PDF.
    async with httpx.AsyncClient(timeout=60, follow_redirects=True) as client:
        resp = await client.get(pdf_url)
        resp.raise_for_status()
        pdf_bytes = resp.content

    # Extract text using PyMuPDF.
    try:
        doc = pymupdf.open(stream=pdf_bytes, filetype="pdf")
        pages_text: list[str] = []

        for page_num in range(min(len(doc), max_pages)):
            page = doc[page_num]
            text = page.get_text("text")
            if text.strip():
                pages_text.append(f"## Page {page_num + 1}\n\n{text}")

        doc.close()
        full_text = "\n\n".join(pages_text)

        if not full_text.strip():
            logger.warning(f"No text extracted from PDF: {pdf_url}")
            return "(No text could be extracted from this PDF)"

        return full_text

    except Exception as e:
        logger.error(f"PDF extraction error for {pdf_url}: {e}")
        raise RuntimeError(f"Failed to extract text from PDF: {e}") from e
