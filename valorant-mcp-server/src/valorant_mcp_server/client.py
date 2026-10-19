"""
HTTP client for the Henrik Dev Valorant API.

Base URL: https://api.henrikdev.xyz
Authentication: pass HENRIK_API_KEY environment variable for higher rate limits.
"""

import os
import sys
from typing import Any

import httpx

from dotenv import load_dotenv

_ = load_dotenv()

BASE_URL = "https://api.henrikdev.xyz"
_API_KEY = os.getenv("HENRIK_API_KEY")

if not _API_KEY:
    ERROR_MESSAGE = (
        "\n=================================================================\n"
        "ERRO: CHAVE DE API DO VALORANT NÃO ENCONTRADA\n"
        "=================================================================\n"
        "A variável de ambiente 'HENRIK_API_KEY' não está configurada.\n"
        "Se você está usando Docker no Claude Desktop, verifique se passou o argumento:\n"
        '"-e", "HENRIK_API_KEY=sua_chave_aqui"\n'
        "dentro da seção 'args' do seu arquivo claude_desktop_config.json.\n"
        "=================================================================\n"
    )
    print(ERROR_MESSAGE, file=sys.stderr)
    sys.exit(1)


def _build_headers() -> dict[str, str]:
    headers: dict[str, str] = {"Accept": "application/json"}
    if _API_KEY:
        headers["Authorization"] = _API_KEY
    return headers


async def get(path: str, params: dict[str, Any] | None = None) -> Any:
    """Make an authenticated GET request to the Henrik Dev API.

    Args:
        path: URL path relative to the base URL (e.g. '/valorant/v2/account/TenZ/SEN').
        params: Optional query parameters.

    Returns:
        Parsed JSON response.

    Raises:
        httpx.HTTPStatusError: If the server returns an error status code.
        RuntimeError: If the API returns an error payload.
    """
    url = f"{BASE_URL}{path}"
    async with httpx.AsyncClient(timeout=15.0) as client:
        response = await client.get(url, headers=_build_headers(), params=params or {})
        response.raise_for_status()
        data = response.json()

    # Henrik API wraps errors in a "status" field.
    if isinstance(data, dict) and data.get("status") not in (None, 200, 1):
        errors = data.get("errors", [{"message": "Unknown API error"}])
        message = (
            errors[0].get("message", "Unknown API error")
            if errors
            else "Unknown API error"
        )
        raise RuntimeError(f"Henrik API error (status {data.get('status')}): {message}")

    return data
