"""PDF text extraction with multi-source fallback downloading."""

from __future__ import annotations

import logging
import re

import httpx
import pymupdf
from bs4 import BeautifulSoup

logger = logging.getLogger(__name__)

# User-Agent to avoid blocks from academic publishers.
_HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/120.0.0.0 Safari/537.36"
    ),
    "Accept": "application/pdf,*/*",
}

# Timeout for each download attempt (seconds).
_DOWNLOAD_TIMEOUT = 45


async def _download_pdf(url: str, client: httpx.AsyncClient) -> bytes | None:
    """Try to download a PDF from a URL. Returns bytes or None on failure."""
    try:
        resp = await client.get(url, headers=_HEADERS)
        if resp.status_code != 200:
            logger.warning(f"PDF download returned {resp.status_code}: {url}")
            return None

        content = resp.content
        # Sanity check: PDFs start with %PDF.
        if not content[:5].startswith(b"%PDF"):
            logger.warning(f"Response is not a PDF (first bytes: {content[:20]!r}): {url}")
            return None

        return content
    except Exception as e:
        logger.warning(f"PDF download error for {url}: {e}")
        return None


async def _try_scihub(doi: str, client: httpx.AsyncClient) -> bytes | None:
    """Try to get a PDF via Sci-Hub using the DOI."""
    scihub_urls = [
        f"https://sci-hub.se/{doi}",
        f"https://sci-hub.st/{doi}",
    ]

    for scihub_url in scihub_urls:
        try:
            resp = await client.get(scihub_url, headers=_HEADERS)
            if resp.status_code != 200:
                continue

            # Parse the HTML to find the embedded PDF iframe/link.
            soup = BeautifulSoup(resp.text, "html.parser")

            # Sci-Hub embeds the PDF in an iframe or a direct link.
            pdf_url = None

            # Try iframe src.
            iframe = soup.find("iframe", id="pdf")
            if iframe and iframe.get("src"):
                pdf_url = iframe["src"]

            # Try embed tag.
            if not pdf_url:
                embed = soup.find("embed", {"type": "application/pdf"})
                if embed and embed.get("src"):
                    pdf_url = embed["src"]

            # Try direct download button.
            if not pdf_url:
                button = soup.find("button", onclick=True)
                if button:
                    onclick = button.get("onclick", "")
                    match = re.search(r"location\.href\s*=\s*['\"]([^'\"]+)['\"]", onclick)
                    if match:
                        pdf_url = match.group(1)

            if not pdf_url:
                logger.debug(f"No PDF link found in Sci-Hub page: {scihub_url}")
                continue

            # Normalize URL.
            if pdf_url.startswith("//"):
                pdf_url = "https:" + pdf_url
            elif pdf_url.startswith("/"):
                # Relative URL — resolve against Sci-Hub host.
                from urllib.parse import urlparse

                parsed = urlparse(scihub_url)
                pdf_url = f"{parsed.scheme}://{parsed.netloc}{pdf_url}"

            logger.info(f"Sci-Hub PDF URL: {pdf_url}")
            return await _download_pdf(pdf_url, client)

        except Exception as e:
            logger.warning(f"Sci-Hub attempt failed ({scihub_url}): {e}")
            continue

    return None


async def _try_unpaywall(doi: str, client: httpx.AsyncClient) -> bytes | None:
    """Try to get a PDF via the Unpaywall API."""
    try:
        url = f"https://api.unpaywall.org/v2/{doi}?email=uniq-tool@example.com"
        resp = await client.get(url, headers={"Accept": "application/json"})
        if resp.status_code != 200:
            return None

        data = resp.json()
        best_oa = data.get("best_oa_location") or {}
        pdf_url = best_oa.get("url_for_pdf")
        if not pdf_url:
            # Try the landing page URL as a last resort.
            pdf_url = best_oa.get("url_for_landing_page")
            if not pdf_url:
                return None

        logger.info(f"Unpaywall PDF URL: {pdf_url}")
        return await _download_pdf(pdf_url, client)

    except Exception as e:
        logger.warning(f"Unpaywall lookup failed for DOI {doi}: {e}")
        return None


async def download_pdf_bytes(
    pdf_url: str | None,
    doi: str | None = None,
) -> bytes:
    """Download PDF bytes, trying multiple sources with fallback.

    Order of attempts:
    1. Direct pdf_url (from search results)
    2. Sci-Hub (if DOI available)
    3. Unpaywall API (if DOI available)

    Raises RuntimeError if all sources fail.
    """
    async with httpx.AsyncClient(
        timeout=_DOWNLOAD_TIMEOUT,
        follow_redirects=True,
    ) as client:
        # Attempt 1: Direct URL.
        if pdf_url:
            logger.info(f"Trying direct PDF URL: {pdf_url}")
            pdf_bytes = await _download_pdf(pdf_url, client)
            if pdf_bytes:
                return pdf_bytes

        # Attempt 2: Sci-Hub.
        if doi:
            logger.info(f"Trying Sci-Hub for DOI: {doi}")
            pdf_bytes = await _try_scihub(doi, client)
            if pdf_bytes:
                return pdf_bytes

        # Attempt 3: Unpaywall.
        if doi:
            logger.info(f"Trying Unpaywall for DOI: {doi}")
            pdf_bytes = await _try_unpaywall(doi, client)
            if pdf_bytes:
                return pdf_bytes

    sources_tried = []
    if pdf_url:
        sources_tried.append(f"direct URL ({pdf_url})")
    if doi:
        sources_tried.append(f"Sci-Hub (DOI: {doi})")
        sources_tried.append(f"Unpaywall (DOI: {doi})")
    raise RuntimeError(
        f"Failed to download PDF from all sources: {', '.join(sources_tried) or 'no URL/DOI'}"
    )


def _extract_text_from_bytes(pdf_bytes: bytes, max_pages: int = 30) -> str:
    """Extract text from PDF bytes using PyMuPDF."""
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
            logger.warning("No text extracted from PDF")
            return "(No text could be extracted from this PDF)"

        return full_text

    except Exception as e:
        logger.error(f"PDF text extraction error: {e}")
        raise RuntimeError(f"Failed to extract text from PDF: {e}") from e


async def extract_pdf_text(
    pdf_url: str | None,
    doi: str | None = None,
    max_pages: int = 30,
) -> str:
    """Download a PDF and extract text content.

    Tries multiple download sources (direct URL, Sci-Hub, Unpaywall)
    before falling back to an error.
    Returns Markdown-formatted text.
    """
    pdf_bytes = await download_pdf_bytes(pdf_url, doi)
    return _extract_text_from_bytes(pdf_bytes, max_pages)
