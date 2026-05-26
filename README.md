# 🛰️ Rust Remote Filesystem (FUSE)

[![Rust Version](https://img.shields.io/badge/rustc-1.75%2B-orange.svg?style=for-the-badge&logo=rust)](https://img.shields.io/badge/rustc-1.75%2B-orange.svg)
[![FUSE Integration](https://img.shields.io/badge/FUSE-fuser--0.11-blue?style=for-the-badge)](https://github.com/cberner/fuser)
[![Server Framework](https://img.shields.io/badge/Server-Axum--0.7-lightgrey?style=for-the-badge&logo=rust)](https://github.com/tokio-rs/axum)
[![Runtime](https://img.shields.io/badge/Runtime-Tokio-red?style=for-the-badge)](https://github.com/tokio-rs/tokio)
[![OS Support](https://img.shields.io/badge/OS_Support-Linux_|_macOS_|_Windows_(Server)-success?style=for-the-badge)](https://img.shields.io/badge/OS_Support-Linux_|_macOS_|_Windows_(Server)-success)
[![License](https://img.shields.io/badge/license-MIT-green.svg?style=for-the-badge)](https://img.shields.io/badge/license-MIT-green.svg)

A high-performance, asynchronous remote filesystem client and server architecture written entirely in **Rust**. This project implements a **FUSE (Filesystem in Userspace)** client that mounts a remote storage folder as a local disk, synchronizing modifications in real-time with a centralized Axum server via REST APIs and WebSockets.

Additionally, the server features an **embedded live web dashboard** so recruiters, developers, and users can instantly visualize, browse, upload, edit, and monitor filesystem operations from their browser without needing a mounted FUSE drive.

---

## ⚡️ Visual Demonstration (Web Control Panel)

Because setting up kernel extensions (FUSE) on guest machines is often tedious, this repository embeds a **Futuristic Glassmorphic Web Dashboard** served directly by the server. 

When you start the server and navigate to `http://localhost:8080/`, you get access to:
* **Interactive File Browser:** Navigate through remote directories, view sizes, permissions, and modification times.
* **Inline Editor:** Click on any text file to open a visual editor modal, make changes, and write them back instantly.
* **Drag-and-Drop Uploader:** Drop single or multiple files directly in the browser to upload them to the remote server.
* **Live System Event Feed:** A real-time console that displays raw WebSocket updates (e.g. FUSE client operations, web edits, server events) as they happen.

---

## 🏗 System Architecture

```text
               +-------------------------------------------------+
               |                   USER SPACE                    |
               |                                                 |
               |   +--------------+      +-------------------+   |
               |   |  Filesystem  |      |   Web Browser     |   |
               |   |  Operations  |      |   (Dashboard)     |   |
               |   +-------+------+      +---------+---------+   |
               |           |                       |             |
               |   +-------v------+                |             |
+----------+   |   | FUSE Client  |                |             |   +---------------+
|  KERNEL  |   |   | (fuser driver|                |             |   |  AXUM SERVER  |
|          |   |   +-------+------+                |             |   |               |
|  +----+  |   |           |                       |             |   |  +---------+  |
|  |FUSE<--+---+-----------+                       |             |   |  | REST API|  |
|  |Dev |  |   |  (reqwest)                        |             |   |  | (Axum)  |  |
|  +----+  |   |           |                       |             |   |  +----+----+  |
+----+-----+   |   +-------v------+                |             |   |       |       |
     |         |   | HTTP Client  |                |             |   |  +----+----+  |
     |         |   +-------+------+                |             |   |  |WebSocket|  |
     |         |           | (REST)                |             |   |  | Handler |  |
     +---------+-----------v------------------------v-------------+---+  +----+----+  |
               |           |                                         |       |       |
               |           |              REST HTTP Requests         |       |       |
               |           +=========================================>       |       |
               |                                                     |       |       |
               |           +=========================================>       |       |
               |                      WebSocket Connection           |       |       |
               |                                                     |  +----+----+  |
               |                                                     |  |Notify   |  |
               |                                                     |  |Watcher  |  |
               |                                                     |  +----+----+  |
               |                                                     |       |       |
               |                                                     |  +----+----+  |
               |                                                     |  | Disk    |  |
               |                                                     |  | Storage |  |
               |                                                     |  +---------+  |
               +-----------------------------------------------------+---------------+
```

---

## 🚀 Key Optimization & Sincronizzazione Logics

### 1. Cache-on-Write (Deferred Uploads)
Write operations (`write` system calls) do not trigger network latency on the fly. 
* **Mechanism:** Data is buffered locally in memory (`OpenWriteFile.buffer`) indexed by offsets.
* **Commit:** Only when a file is closed (`release`), the client downloads the remote original file (if necessary), merges it with the local offset modifications, and executes a single atomic `PUT` upload.
* **Benefit:** Ultra-fast file editing (e.g. typing in an editor) with zero network round-trip overhead during active writes.

### 2. Chunked Reading (Range Requests)
To read large files (such as high-definition videos) without loading them entirely into memory, the client utilizes standard HTTP Range Requests (RFC 7233).
* **Mechanism:** The client translates FUSE `read` requests into `Range: bytes=offset-(offset+size-1)` HTTP headers.
* **Benefit:** Real-time streaming support, near-zero RAM footprint, and instant file openings regardless of their size.

### 3. Echo Suppression (Avoid Sinc Loop)
To prevent infinite update notification loops between the client and server:
1. Every mounted client generates a unique `X-Client-ID` upon startup and includes it in all write/delete REST requests.
2. The server processes the operation and temporarily signs the file path modification inside an in-memory registry.
3. The server disk watcher (`notify` crate) triggers a change event. The server inspects the registry and broadcasts a WebSocket notification formatted as: `CHANGE:/path/to/file|BY:client_uuid`.
4. Clients listening to the WebSocket check the sender tag. If the tag matches their own `X-Client-ID`, they safely discard the notification, otherwise they instantly invalidate their internal metadata cache.

### 4. macOS Quirks & Finder Optimization
Navigating mounted drives on macOS causes the Finder to fire hundreds of hidden attribute requests (like `.DS_Store`, `com.apple.quarantine`, custom icons).
* **Mechanism:** The FUSE client intercepts these extended attributes (`xattr`) calls and automatically replies with a local success code (`reply.ok()`) without forwarding them to the server, preventing Finder slowdowns and server data contamination.

---

## 🔌 REST API Endpoints

The server listens on port `8080` by default and exposes:

| Method | Endpoint | Description | Note |
| :--- | :--- | :--- | :--- |
| `GET` | `/` | Serves the HTML Live Dashboard | Web UI Client |
| `GET` | `/health` | Server Health Status | Returns `"OK"` |
| `GET` | `/ws` | Real-time WebSocket channel | Broadcasting changes |
| `GET` | `/list/*path` | List directory contents | Returns a JSON array of `RemoteEntry` metadata |
| `GET` | `/files/*path` | Streams/reads file contents | Supports **HTTP Range Requests** (206) |
| `PUT` | `/files/*path` | Writes/uploads file contents | Requires `X-Client-ID` header |
| `DELETE`| `/files/*path` | Deletes file or directory | Recursive deletion for folders |
| `POST` | `/mkdir/*path` | Creates a new directory | Creates parent folders recursively (mkdir -p) |
| `PATCH` | `/files/*path` | Updates permissions (chmod) | Payload: `{"perm": "755"}` (Unix only) |

---

## 📦 Setup & Requirements

* **Rust:** Latest stable compiler ([install via rustup](https://rustup.rs/)).
* **FUSE Drivers:**
  * **Linux:** `libfuse-dev` and `fuse3` (e.g. `sudo apt install libfuse-dev fuse3`).
  * **macOS:** [macFUSE](https://osxfuse.github.io/) cask.

> [!NOTE]
> The server component has full cross-platform compatibility and compiles perfectly on **Windows**, **Linux**, and **macOS**. The client component is Unix-only since FUSE relies on the native kernel module.

---

## 🚀 Quick Start

### 1. Start the Server
First, run the Axum backend server to store files and orchestrate WebSocket events.
```bash
cd server
cargo run --release
# Server starts listening on http://localhost:8080
# Data directory is generated automatically under `server/data/`
```

### 2. View the Live Dashboard
Open your browser and navigate to:
```text
http://localhost:8080/
```
You can now play with the file explorer, upload files, write to files, and monitor the live log feed on the right!

### 3. Mount the FUSE Client (Linux / macOS)
In another terminal, mount the remote filesystem into a local directory:
```bash
cd client

# 1. Create a mount directory
mkdir -p /tmp/mountpoint

# 2. Run the client
cargo run -- /tmp/mountpoint
```

You can customize the client cache policies via command-line arguments:
```bash
# Enable LRU Cache with a capacity of 10 items
cargo run -- /tmp/mountpoint --cache-strategy lru --cache-lru-capacity 10

# Enable TTL Cache with 5 seconds expiration
cargo run -- /tmp/mountpoint --cache-strategy ttl --cache-ttl-seconds 5

# Run client in background as a daemon
cargo run -- /tmp/mountpoint --daemon
```

### 4. Unmounting
To safely unmount the drive when you are finished:
```bash
# Linux
fusermount -u /tmp/mountpoint

# macOS
diskutil unmount /tmp/mountpoint
```

---

## 📂 Project Structure

```text
rust-remote-fs/
├── client/                  # FUSE Client source code
│   ├── src/
│   │   ├── main.rs          # CLI argument parsing, WebSocket thread, and mounting
│   │   ├── api_client.rs    # HTTP abstraction tier (reqwest requests, chunked reading)
│   │   ├── config.rs        # Configuration loader (config.toml parser)
│   │   └── fs/              # Core filesystem operations
│   │       ├── mod.rs       # Filesystem dispatcher and core RemoteFS struct
│   │       ├── cache.rs     # TTL and LRU Cache structures
│   │       ├── read.rs      # read, readdir, and lookup operations
│   │       ├── write.rs     # write, flush, and release (Cache-on-Write logic)
│   │       ├── create.rs    # create and mkdir operations
│   │       ├── delete.rs    # delete (unlink) and rmdir operations
│   │       ├── rename.rs    # rename operation
│   │       ├── attr.rs      # getattr and setattr (chmod/chown)
│   │       └── xattr.rs     # macOS Finder extended attribute mocks
│   └── config.toml          # Default client connection configurations
│
├── server/                  # Axum REST + WebSocket Backend
│   ├── src/
│   │   ├── main.rs          # Server initialization, Axum routing, and notify thread
│   │   ├── handlers.rs      # File REST operations and WebSocket handlers
│   │   └── index.html       # Embedded Dashboard control panel
│   └── data/                # Remote storage folder (Created at runtime)
│
└── README.md                # Root Documentation
```

---

## 🛡 License
This project is licensed under the MIT License - see the LICENSE file for details.