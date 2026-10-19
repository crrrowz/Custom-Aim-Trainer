"""
Valorant MCP Server — entry point.

Wraps the Henrik Dev Valorant API (https://docs.henrikdev.xyz/valorant/general)
as a set of MCP tools callable by any MCP-compatible AI client.

Environment variables:
  HENRIK_API_KEY  (required) Your Henrik Dev API key.

Usage:
  valorant-mcp-server              # run via installed script
  uv run valorant-mcp-server       # run via uv
  uv run mcp dev src/valorant_mcp_server/server.py  # MCP Inspector
"""

from typing import Any
from mcp.types import ToolAnnotations

from mcp.server.fastmcp import FastMCP

from valorant_mcp_server.literals import (
    GameMode,
    MapName,
    Platform,
    Region,
    SeasonShort,
    EsportsRegion,
    League,
)
from valorant_mcp_server.tools import accounts, leaderboard, matches, mmr, esports

# ---------------------------------------------------------------------------
# Server instance
# ---------------------------------------------------------------------------

mcp = FastMCP("Valorant MCP Server")

# ---------------------------------------------------------------------------
# Account tools
# ---------------------------------------------------------------------------


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_account(
    name: str, tag: str, force_update: bool = False
) -> dict[str, Any]:
    """Retrieve Valorant account details by Riot ID (name + tag).

    Returns puuid, region, account level, card image URLs, and last update time.

    Args:
        name: In-game name (e.g. 'TenZ').
        tag: Tag line without '#' (e.g. 'SEN').
        force_update: Force a data refresh from Riot servers.
    """
    return await accounts.get_account(name, tag, force_update)


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_account_by_puuid(
    puuid: str, force_update: bool = False
) -> dict[str, Any]:
    """Retrieve Valorant account details by PUUID.

    Returns the same data as get_account, looked up by the player's unique PUUID.

    Args:
        puuid: Player unique identifier.
        force_update: Force a data refresh from Riot servers.
    """
    return await accounts.get_account_by_puuid(puuid, force_update)


# ---------------------------------------------------------------------------
# MMR / rank tools
# ---------------------------------------------------------------------------


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_mmr(
    region: Region,
    name: str,
    tag: str,
    platform: Platform = "pc",
) -> dict[str, Any]:
    """Retrieve current MMR / rank details for a player by Player#Tag.

    Returns account, peak, currentposition
    and seasonal mmr info.

    Args:
        region: Server region — eu, na, latam, br, ap, or kr.
        name: In-game name.
        tag: Tag line without '#'.
        platform: 'pc' (default) or 'console'.
    """
    return await mmr.get_mmr(region, name, tag, platform)


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_mmr_by_puuid(
    region: Region,
    puuid: str,
    platform: Platform = "pc",
) -> dict[str, Any]:
    """Retrieve current MMR / rank details for a player by PUUID.

    Args:
        region: Server region — eu, na, latam, br, ap, or kr.
        puuid: Player unique identifier.
        platform: 'pc' (default) or 'console'.
    """
    return await mmr.get_mmr_by_puuid(region, puuid, platform)


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_mmr_history(
    region: Region,
    puuid: str,
    platform: Platform = "pc",
) -> dict[str, Any]:
    """Retrieve ranked rating (RR) change history for a player by PUUID.

    Each entry shows the RR gained/lost in a competitive match along
    with the tier, map, and date.

    Args:
        region: Server region — eu, na, latam, br, ap, or kr.
        puuid: Player unique identifier.
        platform: 'pc' (default) or 'console'.
    """
    return await mmr.get_mmr_history(region, puuid, platform)


# ---------------------------------------------------------------------------
# Match tools
# ---------------------------------------------------------------------------


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
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

    Returns a list of match summaries including scores, kills, agents played,
    and match outcome for all players.

    Args:
        region: Server region — eu, na, latam, br, ap, or kr.
        name: In-game name.
        tag: Tag line without '#'.
        platform: 'pc' (default) or 'console'.
        mode: Optional game mode filter (e.g. 'competitive', 'unrated').
        map: Optional map name filter (e.g. 'Ascent').
        size: Number of matches to return. Default and max vary by API tier.
    """
    return await matches.get_match_history(
        region, name, tag, platform, mode, map_name, size
    )


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_match(
    region: Region,
    match_id: str,
) -> dict[str, Any]:
    """Retrieve full details for a single Valorant match by match ID.

    Returns complete data: metadata, all players (agents, stats, loadouts),
    round-by-round results, kill feed, and economy.

    Args:
        region: Server region — eu, na, latam, br, ap, or kr.
        match_id: Match UUID (e.g. '696848f3-f16f-45bf-af13-e2192f81a600').
    """
    return await matches.get_match(region, match_id)


# ---------------------------------------------------------------------------
# Leaderboard tools
# ---------------------------------------------------------------------------


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_leaderboard(
    region: Region,
    platform: Platform = "pc",
    name: str | None = None,
    tag: str | None = None,
    puuid: str | None = None,
    season_short: SeasonShort | None = None,
    size: int | None = None,
    page: int | None = None,
) -> dict[str, Any]:
    """Retrieve the competitive leaderboard for a region and platform.

    Filter by a specific player using name+tag OR puuid (not both).
    Optionally filter by season (e.g. 'e9a1') for historical data.

    Args:
        region: Server region — eu, na, latam, br, ap, or kr.
        platform: 'pc' (default) or 'console'.
        name: Filter to a specific player name (requires tag).
        tag: Filter to a specific player tag (requires name).
        puuid: Filter by PUUID — mutually exclusive with name/tag.
        season_short: Season short code (e.g. 'e9a1') for historical leaderboards.
        size: Number of entries to return.
        page: Pagination offset (0-indexed).
    """
    return await leaderboard.get_leaderboard(
        region, platform, name, tag, puuid, season_short, size, page
    )


# ---------------------------------------------------------------------------
# Esports Tools
# ---------------------------------------------------------------------------


@mcp.tool(
    annotations=ToolAnnotations(
        readOnlyHint=True,
        destructiveHint=False,
        idempotentHint=True,
        openWorldHint=True,
    )
)
async def get_esports_games_data(
    region: EsportsRegion | None = None,
    league: list[League] | None = None,
) -> list[dict[str, Any]]:
    """Retrieve the current and upcoming schedule for Valorant esports matches.

    Can be filtered by a specific broader region or by an explicit list
    of leagues/tournaments.

    Args:
        region: Optional region to filter by (e.g., 'international', 'north america', 'emea').
        league: Optional list of specific leagues to filter by (e.g., ['vct_americas', 'vct_emea']).
    """
    return await esports.get_esports_games_data(region, league)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    """Start the Valorant MCP Server using stdio transport."""
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
