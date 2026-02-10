"""Paper search across Semantic Scholar and arXiv."""

from __future__ import annotations

import asyncio
import contextlib
import logging

import feedparser
import httpx

from src.models.paper import PaperMeta, PaperSource

logger = logging.getLogger(__name__)

# Rate limiting.
SEMANTIC_SCHOLAR_DELAY = 1.1  # seconds between requests (API limit: 1/sec with key)
ARXIV_DELAY = 1.5  # seconds between requests

# Timeout for individual search API calls (seconds).
SEARCH_TIMEOUT = 15

SEMANTIC_SCHOLAR_BASE = "https://api.semanticscholar.org/graph/v1"
ARXIV_BASE = "https://export.arxiv.org/api/query"


def _truncate_query(query: str, max_len: int = 200) -> str:
    """Truncate a query to a reasonable length for academic search APIs."""
    if len(query) <= max_len:
        return query
    truncated = query[:max_len]
    # Break at a word boundary.
    last_space = truncated.rfind(" ")
    if last_space > max_len // 2:
        truncated = truncated[:last_space]
    return truncated


async def search_semantic_scholar(
    query: str,
    max_results: int = 100,
    year_min: int | None = None,
    year_max: int | None = None,
    prefer_open_access: bool = True,
) -> list[PaperMeta]:
    """Search Semantic Scholar for papers matching a query."""
    query = _truncate_query(query)
    papers: list[PaperMeta] = []
    fields = "title,url,year,citationCount,openAccessPdf,abstract,authors,fieldsOfStudy,externalIds"
    offset = 0
    limit = min(max_results, 100)  # API max per request is 100.

    async with httpx.AsyncClient(timeout=SEARCH_TIMEOUT, follow_redirects=True) as client:
        retries = 0
        max_retries = 3
        while offset < max_results:
            params = {
                "query": query,
                "offset": offset,
                "limit": limit,
                "fields": fields,
            }

            if year_min or year_max:
                year_filter = f"{year_min or ''}-{year_max or ''}"
                params["year"] = year_filter

            if prefer_open_access:
                params["openAccessPdf"] = ""

            try:
                resp = await client.get(f"{SEMANTIC_SCHOLAR_BASE}/paper/search", params=params)
                if resp.status_code == 429:
                    retries += 1
                    if retries > max_retries:
                        logger.warning("Semantic Scholar rate limit exceeded, giving up")
                        break
                    wait_time = min(3 * retries, 10)
                    logger.warning(f"Semantic Scholar rate limited, waiting {wait_time}s...")
                    await asyncio.sleep(wait_time)
                    continue
                resp.raise_for_status()
                data = resp.json()
                retries = 0  # Reset on success.
            except httpx.HTTPStatusError as e:
                logger.error(f"Semantic Scholar HTTP error: {e}")
                break
            except Exception as e:
                logger.error(f"Semantic Scholar search error: {e}")
                break

            for item in data.get("data", []):
                pdf_url = None
                if item.get("openAccessPdf"):
                    pdf_url = item["openAccessPdf"].get("url")

                authors = [a.get("name", "") for a in item.get("authors", [])]

                # Extract DOI from externalIds.
                external_ids = item.get("externalIds") or {}
                doi = external_ids.get("DOI")

                paper = PaperMeta(
                    id=f"s2:{item.get('paperId', '')}",
                    title=item.get("title", ""),
                    authors=authors,
                    year=item.get("year"),
                    abstract_text=item.get("abstract", "") or "",
                    citation_count=item.get("citationCount"),
                    url=item.get("url", ""),
                    pdf_url=pdf_url,
                    doi=doi,
                    source=PaperSource.SEMANTIC_SCHOLAR,
                    fields=item.get("fieldsOfStudy", []) or [],
                )
                papers.append(paper)

            total = data.get("total", 0)
            offset += limit
            if offset >= total:
                break

            await asyncio.sleep(SEMANTIC_SCHOLAR_DELAY)

    return papers[:max_results]


async def search_arxiv(
    query: str,
    max_results: int = 100,
    year_min: int | None = None,
    year_max: int | None = None,
) -> list[PaperMeta]:
    """Search arXiv for papers matching a query."""
    query = _truncate_query(query)
    papers: list[PaperMeta] = []

    # Build arXiv search query.
    search_query = f"all:{query}"

    async with httpx.AsyncClient(timeout=SEARCH_TIMEOUT) as client:
        offset = 0
        batch_size = min(max_results, 200)

        while offset < max_results:
            params = {
                "search_query": search_query,
                "start": offset,
                "max_results": batch_size,
                "sortBy": "relevance",
                "sortOrder": "descending",
            }

            try:
                resp = await client.get(ARXIV_BASE, params=params)
                resp.raise_for_status()
                feed = feedparser.parse(resp.text)
            except Exception as e:
                logger.error(f"arXiv search error: {e}")
                break

            if not feed.entries:
                break

            for entry in feed.entries:
                # Extract arXiv ID from the entry URL.
                arxiv_id = entry.get("id", "").split("/abs/")[-1]

                # Find PDF link.
                pdf_url = None
                for link in entry.get("links", []):
                    if link.get("type") == "application/pdf":
                        pdf_url = link.get("href")
                        break
                if not pdf_url and arxiv_id:
                    pdf_url = f"https://arxiv.org/pdf/{arxiv_id}"

                # Extract year.
                published = entry.get("published", "")
                year = None
                if published and len(published) >= 4:
                    with contextlib.suppress(ValueError):
                        year = int(published[:4])

                # Year filter.
                if year_min and year and year < year_min:
                    continue
                if year_max and year and year > year_max:
                    continue

                authors = [a.get("name", "") for a in entry.get("authors", [])]
                categories = [t.get("term", "") for t in entry.get("tags", [])]

                # ArXiv entries may have a DOI link.
                arxiv_doi = None
                for link in entry.get("links", []):
                    href = link.get("href", "")
                    if "doi.org" in href:
                        arxiv_doi = href.split("doi.org/")[-1] if "doi.org/" in href else None
                        break
                # feedparser also parses the <arxiv:doi> tag if present.
                if not arxiv_doi:
                    arxiv_doi = entry.get("arxiv_doi")

                paper = PaperMeta(
                    id=f"arxiv:{arxiv_id}",
                    title=entry.get("title", "").replace("\n", " ").strip(),
                    authors=authors,
                    year=year,
                    published_date=published[:10] if published else None,
                    abstract_text=entry.get("summary", "").replace("\n", " ").strip(),
                    url=entry.get("id", ""),
                    pdf_url=pdf_url,
                    doi=arxiv_doi,
                    source=PaperSource.ARXIV,
                    fields=categories,
                )
                papers.append(paper)

            offset += batch_size
            if len(feed.entries) < batch_size:
                break

            await asyncio.sleep(ARXIV_DELAY)

    return papers[:max_results]


async def search_all_sources(
    queries: list[str],
    max_results: int = 200,
    year_min: int | None = None,
    year_max: int | None = None,
    prefer_open_access: bool = True,
) -> list[PaperMeta]:
    """Search all sources and merge/deduplicate results."""
    all_papers: list[PaperMeta] = []
    seen_titles: set[str] = set()

    per_query_limit = max(max_results // max(len(queries), 1), 20)

    # Build all search tasks upfront and run concurrently.
    tasks: list[asyncio.Task] = []
    for query in queries:
        s2_task = asyncio.create_task(
            search_semantic_scholar(
                query=query,
                max_results=per_query_limit,
                year_min=year_min,
                year_max=year_max,
                prefer_open_access=prefer_open_access,
            )
        )
        arxiv_task = asyncio.create_task(
            search_arxiv(
                query=query,
                max_results=per_query_limit,
                year_min=year_min,
                year_max=year_max,
            )
        )
        tasks.extend([s2_task, arxiv_task])

    # Wait for all with a global timeout.
    done, pending = await asyncio.wait(tasks, timeout=45)

    # Cancel anything still running.
    for task in pending:
        task.cancel()
        logger.warning(f"Search task timed out and was cancelled: {task.get_name()}")

    for task in done:
        try:
            papers = task.result()
            for paper in papers:
                title_lower = paper.title.lower().strip()
                if title_lower not in seen_titles:
                    seen_titles.add(title_lower)
                    all_papers.append(paper)
        except Exception as e:
            logger.warning(f"Search task failed: {e}")

    # Sort by citation count (descending), then by year (descending).
    all_papers.sort(key=lambda p: (p.citation_count or 0, p.year or 0), reverse=True)

    return all_papers[:max_results]
