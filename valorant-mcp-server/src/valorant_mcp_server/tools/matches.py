"""
MCP tools for Valorant match history and match details via the Henrik Dev API.

Endpoints used:
  GET /valorant/v4/matches/{region}/{platform}/{name}/{tag}
  GET /valorant/v4/match/{region}/{matchid}
"""

from typing import Any

from valorant_mcp_server import client
from valorant_mcp_server.literals import GameMode, MapName, Platform, Region


async def get_match_history(
    region: Region,
    name: str,
    tag: str,
    platform: Platform = "pc",
    mode: GameMode | None = None,
    map_name: MapName | None = None,
    size: int | None = None,
) -> list[dict[str, Any]]:
    """Retrieve recent match history for a player by Riot ID.

    Args:
        region: Server region. One of: eu, na, latam, br, ap, kr.
        name: The player's in-game name.
        tag: The player's tag line without '#'.
        platform: Platform to query — 'pc' (default) or 'console'.
        mode: Optional game mode filter (e.g. 'competitive', 'unrated').
        map_name: Optional map_name name filter (e.g. 'Ascent', 'Bind').
        size: Number of matches to return (max varies by API tier).

    Returns:
        A list of match summary objects. Each entry includes match metadata,
        teams, and per-player stats for that match.
    """
    params: dict[str, Any] = {}
    if mode:
        params["mode"] = mode
    if map_name:
        params["map_name"] = map_name
    if size is not None:
        params["size"] = size

    data = await client.get(
        f"/valorant/v4/matches/{region}/{platform}/{name}/{tag}", params=params
    )
    return data.get("data", data)


async def get_match(region: Region, match_id: str) -> dict[str, Any]:
    """Retrieve full details for a single Valorant match by match ID.

    Args:
        region: Server region. One of: eu, na, latam, br, ap, kr.
        match_id: The unique match UUID (e.g. '696848f3-f16f-45bf-af13-e2192f81a600').

    Returns:
        A dictionary with complete match data including metadata, all players,
        round results, kills, economy, and team outcomes.
    """
    data = await client.get(f"/valorant/v4/match/{region}/{match_id}")
    return data.get("data", data)
