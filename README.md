# Palworld Server Manager

A modern, open-source desktop control panel for your Palworld dedicated server, built with Tauri v2 and React and powered by the official Palworld REST API — a lightweight, native Windows app for monitoring, administering, and managing your server in real time.

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows-0078D6.svg)](#download--install)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri%20v2-24C8DB.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/React-18-61DAFB.svg)](https://react.dev)

Palworld Server Manager gives server admins a fast, friendly, and secure way to keep an eye on their world, manage players, and run administrative commands — all from a sleek OLED dark interface that installs in seconds and keeps your credentials on your own machine.

## Screenshots

> Screenshots render here once you add the image files under `docs/screenshots/`.

![Real-time dashboard showing server FPS, uptime, players online, and frame time](docs/screenshots/dashboard.png)

![Player management panel listing connected players with kick, ban, and unban controls](docs/screenshots/players.png)

![Live world map plotting the positions of every player and Pal](docs/screenshots/world-map.png)

![Server settings inspector with search and export tools](docs/screenshots/settings.png)

![Command console for announcements, saves, and scheduled shutdowns](docs/screenshots/console.png)

## Features

- **Real-time dashboard** — monitor server FPS, uptime, players online, and frame time at a glance, with metrics refreshing live.
- **Live world map** — plot the position of every player and every Pal on an interactive map that updates as your world changes.
- **Player management** — view connected players and instantly **kick**, **ban**, or **unban** anyone with a single click.
- **Command console** — send server **announcements**, trigger a **save**, schedule a **shutdown**, or issue a **force stop** without touching the terminal.
- **Full server settings inspector** — browse every server option with instant **search** and one-click **export** for backups and documentation.
- **Sleek OLED dark UI** — a clean, high-contrast interface designed to be easy on the eyes during long admin sessions.
- **Tiny native installer** — a lightweight Tauri build with a small footprint and near-instant startup, not a bulky bundled browser.
- **Credentials stay on your machine** — your admin password is stored locally and sent only to the server you connect to.

## Screens

- **Dashboard** — the at-a-glance command center for server health: FPS, uptime, players online, and frame time.
- **Players** — a live roster of everyone connected, with quick kick, ban, and unban actions.
- **World Map** — a real-time map plotting every player and Pal across your world.
- **Console** — a command hub for announcements, saves, scheduled shutdowns, and force stops.
- **Settings** — a searchable, exportable inspector for every server setting exposed by the REST API.
- **Ban Manager** — review your banned player list and lift bans whenever you're ready.

## Download & Install

1. Head to the **Releases page** and download the latest **`.msi`** or **`.exe`** installer.
2. Run the installer and follow the prompts. WebView2 is preinstalled on Windows 11, so there's nothing extra to set up.
3. Because release builds may be unsigned, Windows **SmartScreen** might warn you the first time. Click **"More info"**, then **"Run anyway"** to continue.

## Enable the REST API on your server

Palworld Server Manager talks to your server through the official Palworld REST API, which is off by default. Enable it in your server's `PalWorldSettings.ini` file, under the `[/Script/Pal.PalGameWorldSettings]` section, inside the `OptionSettings` line:

```ini
RESTAPIEnabled=True,RESTAPIPort=8212,AdminPassword="YourPassword"
```

Then **restart the server** so the changes take effect.

## Connect

Launch Palworld Server Manager and enter:

- **Host** — `localhost` if the app runs on the same machine as the server, or your server's **LAN IP** address.
- **Port** — `8212` (or whatever you set for `RESTAPIPort`).
- **AdminPassword** — the `AdminPassword` you configured above.

Click connect and you're in.

## Build from source

Prefer to build it yourself? It's a standard Tauri + React project.

**Prerequisites**

- **Node.js 20+**
- **Rust** (stable, installed via [rustup](https://rustup.rs))
- **Microsoft C++ Build Tools**
- **WebView2** (preinstalled on Windows 11)

**Steps**

```bash
# Install JavaScript dependencies
npm install

# Run the app in development mode
npm run tauri dev

# Produce Windows installers (.msi / .exe)
npm run tauri build
```

Built installers are written to `src-tauri/target/release/bundle`.

## Security

The Palworld REST API is designed for **LAN use** — please do **not** expose it directly to the public internet. If you need remote access, put it behind a VPN or a properly secured tunnel. Palworld Server Manager stores your credentials locally and sends them only to the server you explicitly configure; nothing is transmitted anywhere else.

## Roadmap

- Direct `PalWorldSettings.ini` editing from within the app
- RCON support
- Historical metrics graphs (FPS, players, uptime over time)
- macOS and Linux builds

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

## License

Released under the [MIT License](LICENSE).

<!--
Suggested GitHub topics:
palworld, palworld-server, palworld-dedicated-server, game-server-manager, server-manager, dedicated-server, rest-api, tauri, react, typescript, windows, control-panel
-->
