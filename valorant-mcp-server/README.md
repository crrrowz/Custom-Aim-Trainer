<div align="center">
  <h1>🎯 Valorant MCP Server</h1>
  <p><strong>Model Context Protocol Server for Valorant Data</strong></p>
  
  [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
  [![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker&logoColor=white)](https://www.docker.com/)
  [![Python](https://img.shields.io/badge/Python-3.13+-ffd43b?logo=python&logoColor=blue)](https://www.python.org/)
  [![MCP](https://img.shields.io/badge/MCP-Ready-00E5FF)](https://modelcontextprotocol.io/)
  [![API](https://img.shields.io/badge/API-HenrikDev-red)](https://docs.henrikdev.xyz/)
</div>

<p align="center">
  A highly capable, community-driven Model Context Protocol (MCP) server that empowers your AI assistant to fetch real-time data from Valorant. Whether you're tracking performance, analyzing match history, or checking the competitive leaderboards, this MCP server seamlessly integrates Valorant statistics directly into your preferred AI clients.
</p>

---

## 📑 Table of Contents

- [Features](#-features)
- [Demo / Screenshots](#-demo--screenshots)
- [Tech Stack](#-tech-stack)
- [Quick Start / Setup](#-quick-start--setup)
- [Local Development & Advanced Setup](#-local-development--advanced-setup)
- [Credits](#-credits)
- [License](#-license)

---

## ✨ Features

Equip your AI with comprehensive Valorant knowledge:

- 👤 **Account Lookups**: Retrieve account details, levels, and player cards using Riot ID or PUUID.
- 🏆 **Rank & MMR**: Check current competitive tier, Ranked Rating (RR), and full match rating history natively.
- 🔫 **Match History**: Access detailed summaries of recent games, filterable by map, mode, or platform.
- 📊 **Match Details**: Dive deep into post-game details, including economy, rounds, and per-player statistics.
- 🌐 **Leaderboards**: Pull ranked leaderboards for any region, filter for specific players, or check historical seasons.

---

## 📸 Demo / Screenshots

![demo](./assets/demo.gif)

> *(Visuals coming soon!)*

---

## 🛠 Tech Stack

Built with modern Python tooling for maximum performance and reliability:

- ![Python](https://img.shields.io/badge/Python-3.13-blue?logo=python&logoColor=white) **Python 3.13+**
- ![uv](https://img.shields.io/badge/uv-Package%20Manager-purple) **uv** for blazing-fast dependency management
- ![httpx](https://img.shields.io/badge/httpx-Async%20HTTP-green) **httpx** for non-blocking API calls
- ![MCP](https://img.shields.io/badge/MCP-Protocol-00E5FF) **mcp[cli]** for seamless Model Context Pr## 🚀 Quick Start / Setup

To use this server, you need a **Henrik API Key** (get one at [HenrikDev Docs](https://docs.henrikdev.xyz/)) and **Docker** installed.

Configure your preferred MCP client by adding the following snippet to your configuration file. *You do not need to clone the repository to use the server; the official Docker image will be used automatically.*

### 🤖 Claude Desktop

**Where to find the config file:**
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Linux:** `~/.config/Claude/claude_desktop_config.json`

**Configuration:**
```json
{
  "mcpServers": {
    "valorant-mcp-server": {
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "-e",
        "HENRIK_API_KEY=your_api_key_here",
        "ghcr.io/codenilson/valorant-mcp-server:latest"
      ]
    }
  }
}
```
*Restart Claude Desktop entirely after saving the file.*

### 🌌 Antigravity

**Where to find the config file:**
- Built into the Antigravity UI directly, usually in Settings -> MCP Servers.

**Configuration:**
Simply add this to your MCP servers configuration menu or JSON:
```json
{
  "mcpServers": {
    "valorant-mcp-server": {
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "-e",
        "HENRIK_API_KEY=your_api_key_here",
        "ghcr.io/codenilson/valorant-mcp-server:latest"
      ]
    }
  }
}
```

### 💻 VS Code (Roo Code / Cline)

**Where to find the config file:**
- Typically located in `~/.vscode/mcp.json` or within the extension's specific MCP settings UI. (e.g., `%APPDATA%\Code\User\globalStorage\rooveterinaryinc.roo-cline\settings\cline_mcp_settings.json` on Windows).

**Configuration:**
```json
{
  "mcpServers": {
    "valorant-mcp-server": {
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "-e",
        "HENRIK_API_KEY=your_api_key_here",
        "ghcr.io/codenilson/valorant-mcp-server:latest"
      ]
    }
  }
}
```
*Reload the VS Code window (`Ctrl+Shift+P` -> `Developer: Reload Window`) after saving.*

---

## 🛠 Local Development & Advanced Setup

If you want to run the server from source, build the Docker image locally, or contribute to the project, follow these steps.

### Prerequisites
- **Python 3.13+** and **uv** package manager
- **Git**

### Option 1: Local Development (via uv)

1. **Clone the repository:**
   ```bash
   git clone https://github.com/codeNilson/valorant-mcp-server
   cd valorant-mcp-server
   ```

2. **Sync dependencies:**
   ```bash
   uv sync
   ```

3. **Client Configuration:**
   Configure your MCP client to run the local version directly using `uv`:
   ```json
   {
     "mcpServers": {
       "valorant-mcp-server": {
         "command": "uv",
         "args": [
           "--directory",
           "C:\\path\\to\\valorant-mcp-server",
           "run",
           "valorant-mcp-server"
         ],
         "env": {
           "HENRIK_API_KEY": "your_api_key_here"
         }
       }
     }
   }
   ```

### Option 2: Build Docker Image Locally

If you prefer to build the container yourself instead of using the pre-built image:

1. **Clone and build:**
   ```bash
   git clone https://github.com/codeNilson/valorant-mcp-server
   cd valorant-mcp-server
   docker build -t valorant-mcp-server .
   ```

2. **Client Configuration:**
   Use the same configuration as the Quick Start, but replace the `ghcr.io/...` image with your local tag:
   ```json
   {
     "mcpServers": {
       "valorant-mcp-server": {
         "command": "docker",
         "args": [
           "run",
           "-i",
           "--rm",
           "-e",
           "HENRIK_API_KEY=your_api_key_here",
           "valorant-mcp-server"
         ]
       }
     }
   }
   ```

---

## 🙏 Credits

This project is made possible thanks to the **Henrik API** providing the unofficial Valorant API data endpoints.

- ⚡ **API Documentation**: [https://docs.henrikdev.xyz](https://docs.henrikdev.xyz)
- ⚠️ **Disclaimer**: This project is an unofficial community creation and is **not** affiliated with, endorsed, or sponsored by Riot Games or Valorant. Valorant and Riot Games are trademarks or registered trademarks of Riot Games, Inc.

---

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
