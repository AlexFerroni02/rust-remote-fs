# rust-remote-fs
Remote File System in Rust 


# Project Setup and Development Guide

This document contains all the necessary steps to configure the development environment and start working on the remote file system project in Rust.

## Table of Contents
1. [Phase 1: Local Environment Setup (WSL)](#phase-1-local-environment-setup-wsl)
2. [Phase 2: Local Project Configuration](#phase-2-local-project-configuration)

---

## Phase 1: Local Environment Setup (WSL)
*This phase must be completed by **both** collaborators on their Windows PCs.*

### 1.1. Installing WSL and Ubuntu
1. Open **PowerShell** as an **Administrator**.
2. Run the command to install the Windows Subsystem for Linux and the Ubuntu distribution.
   ```bash
   wsl --install
   ```
3. Reboot your PC when prompted. After rebooting, set up your username and password for the Ubuntu environment.

### 1.2. Installing Development Tools
1. Open the **Ubuntu** terminal from the Start Menu.
2. Update the system and install the essential tools (Git, C compiler, FUSE libraries).
   ```bash
   sudo apt update && sudo apt upgrade -y
   sudo apt install build-essential pkg-config libfuse-dev git -y
   ```
3. Install the **Rust** toolchain via `rustup`.
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
   ```
4. Follow the instructions (choose option 1, "default installation"). When finished, **close and reopen the Ubuntu terminal**.
5. Install the **GitHub CLI** (`gh`) to simplify authentication.
   ```bash
   (type -p wget >/dev/null || (sudo apt update && sudo apt install wget -y)) \
   && sudo mkdir -p -m 755 /etc/apt/keyrings \
   && wget -qO- [https://cli.github.com/packages/githubcli-archive-keyring.gpg](https://cli.github.com/packages/githubcli-archive-keyring.gpg) | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null \
   && sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
   && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] [https://cli.github.com/packages](https://cli.github.com/packages) stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
   && sudo apt update \
   && sudo apt install gh -y
   ```
---

## Phase 2: Local Project Configuration


### 2.1. Git Authentication and Configuration
1. In your Ubuntu terminal, set your name and email (use the same details as your GitHub account).
   ```bash
   git config --global user.name "Your Name"
   git config --global user.email "your-email@example.com"
   ```
2. Authenticate to GitHub via the CLI.
   ```bash
   gh auth login
   ```
   Follow the instructions (choose GitHub.com, HTTPS, Login with a web browser). If the browser doesn't open automatically
    1. The `gh` command will give you an 8-character code and a URL (`https://github.com/login/device`).
    2. **Copy** the code.
    3. **Manually open** your browser and go to the provided URL.
    4. **Paste** the code into the GitHub page and authorize.
    5. The terminal should unblock on its own. If it remains stuck, press `Ctrl + C` and verify with `gh auth status`.
### 3.2. Cloning the Repository and Opening in RustRover
1. Create a folder for your projects and navigate into it.
   ```bash
   cd ~
   mkdir -p projects
   cd projects
   ```
2. Clone the repository.
   ```bash
   gh repo clone REPO_OWNER_USERNAME/REPO_NAME
   ```
3. Open **RustRover** on Windows.
4. Go to **File > Open** and navigate to the project path in WSL. The path will be similar to `\\wsl$\Ubuntu\home\YourLinuxUsername\projects\REPO_NAME`.
5. Open the folder. RustRover should automatically configure the WSL toolchain.

---


# How to Run the Server and Client


## 1. Start the Server

Open a terminal and navigate to the server directory:

```bash
cd ~/projects/REPO_NAME/server
cargo run
```

The server will start and listen for requests (usually on `localhost:8080`).

---

## 2. Start the Client

Open a **second terminal** and navigate to the client directory:

```bash
cd ~/projects/REPO_NAME/client
cargo run -- /tmp/mountpoint
```

Replace `/tmp/mountpoint` with the path where you want to mount the remote filesystem.  
Make sure the directory exists (create it if necessary):

```bash
mkdir -p /tmp/mountpoint
```

---
## 3. Access the Mounted Filesystem

Once the client is running, you can access the remote files via the mountpoint:

```bash
ls /tmp/mountpoint
```

You can use standard file commands (`cat`, `cp`, etc.) on files in the mountpoint.

---