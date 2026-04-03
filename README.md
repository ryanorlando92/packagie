# Packagie (Dutchie Automation)

Packagie started as a brittle autohotkey script made to try and save some time. I've switched to Tauri for v2 so i can use it on my linux laptop at work. You can still find the original v1 in the autohotkey folder.

Packagie is a cross-platform desktop application built with Tauri, Rust, and React(lol) It is designed to automate the tedious process of weekly inventory directly into the Dutchie Backoffice "Receive Inventory" system. 

Packagie mimics human interaction to safely and accurately inject NDC, quantity, package ID, External package ID (Metrc tag), Lot Number, and expiration / package dates into the Dutchie UI.

## How it Works

* **Excel Parsing:** Reads an `.xlsx` inventory prep file and parses the data.
* **Automated Data Entry:** Uses a hybrid approach of JavaScript DOM manipulation and synthetic event dispatching to trick React/MUI.
* **Pause in Background** Packagie will pause if you navigate away from the window.
* **Full Order Input:** Processes one row per 5 seconds, turning an hours long task into a 5 minute coffee break.
* **Finish Prep Sheet** Fill empty fields on prep sheet before importing.

## Features

* **Credential Vault:** Save your username & password once, and packagie will log in for you every time. (not yet supported in Windows)
* **Cross-Platform:** Built on Tauri v2, ensuring lightweight, native execution on Windows, macOS, and Linux.
* **Auto-Updater:** Built-in updater  pulls the latest signed binaries directly from GitHub Releases.

## Installation & Setup

1. Get the latest version from [https://github.com/ryanorlando92/packagie/releases/latest](https://github.com/ryanorlando92/packagie/releases/latest)

* VERANO COMPUTERS MUST USE `packagie-x.x.x_x64-setup.exe` THIS WILL INSTALL INTO YOUR APPDATA FOLDER BY DEFAULT AND WILL NOT REQUIRE AN ADMIN PASSWORD

2. Install the package

Thats it! Yes it's that easy.

## Known Issues

* **Auto Login** Not working on windows

## Roadmap / Planned Features
I am actively working to make Packagie more robust and feature-rich. The following updates are planned:

[ ] In-App Documentation: A "Readme" button directly in the UI to easily access instructions and troubleshooting steps.

[ ] add url target to settings
