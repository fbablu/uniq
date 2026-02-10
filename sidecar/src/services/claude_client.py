"""Claude API client wrapper."""

from __future__ import annotations

import logging
import os

import anthropic

logger = logging.getLogger(__name__)

_client: ClaudeClient | None = None


class ClaudeClient:
    """Wrapper around the Anthropic SDK for uniq-specific operations."""

    def __init__(self, api_key: str, model: str = "claude-sonnet-4-20250514"):
        self.client = anthropic.AsyncAnthropic(api_key=api_key)
        self.model = model

    async def analyze(self, prompt: str, max_tokens: int = 4096) -> str:
        """Send a prompt to Claude and return the text response.

        The prompt should request JSON output. This method returns the raw
        text response — the caller is responsible for parsing.
        """
        try:
            message = await self.client.messages.create(
                model=self.model,
                max_tokens=max_tokens,
                messages=[{"role": "user", "content": prompt}],
            )
            # Extract text from the response.
            text = ""
            for block in message.content:
                if block.type == "text":
                    text += block.text

            # Try to extract JSON if wrapped in markdown code blocks.
            text = text.strip()
            if text.startswith("```json"):
                text = text[7:]
            elif text.startswith("```"):
                text = text[3:]
            if text.endswith("```"):
                text = text[:-3]

            return text.strip()
        except Exception as e:
            logger.error(f"Claude API error: {e}")
            raise

    async def generate_code(
        self,
        system_prompt: str,
        user_prompt: str,
        max_tokens: int = 8192,
    ) -> str:
        """Generate code using Claude with a system prompt for context."""
        try:
            message = await self.client.messages.create(
                model=self.model,
                max_tokens=max_tokens,
                system=system_prompt,
                messages=[{"role": "user", "content": user_prompt}],
            )
            text = ""
            for block in message.content:
                if block.type == "text":
                    text += block.text
            return text.strip()
        except Exception as e:
            logger.error(f"Claude API error during code generation: {e}")
            raise


def get_claude_client() -> ClaudeClient | None:
    """Get or create the global Claude client.

    Returns None if no API key is configured.
    """
    global _client
    if _client is not None:
        return _client

    api_key = os.environ.get("ANTHROPIC_API_KEY", "")
    if not api_key:
        # Try loading from uniq config.
        config_path = os.path.expanduser("~/.config/uniq/config.toml")
        if os.path.exists(config_path):
            try:
                import tomllib

                with open(config_path, "rb") as f:
                    config = tomllib.load(f)
                api_key = config.get("api_keys", {}).get("anthropic", "")
            except Exception:
                pass

    if not api_key:
        logger.warning("No Anthropic API key found. Claude features will be unavailable.")
        return None

    model = os.environ.get("UNIQ_CLAUDE_MODEL", "claude-sonnet-4-20250514")
    _client = ClaudeClient(api_key=api_key, model=model)
    return _client
