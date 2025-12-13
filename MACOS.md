# Remote Filesystem - macOS Quick Start

This project runs on macOS but requires **macFUSE**, as FUSE is not built into the kernel.

## 1. Install macFUSE (Required)
The client will not compile or link without this library.

```bash
brew install --cask macfuse

```

> **Important:** You must allow the system extension in **System Settings > Privacy & Security** and restart your Mac if prompted.

---

##2. Quick Start**Terminal 1 (Server):**

```bash
cd server && cargo run --release

```

**Terminal 2 (Client):**

```bash
cd client
mkdir mountpoint
cargo run --release -- mountpoint

```

---

##3. macOS Specifics* **Finder & Metadata:** macOS attempts to write many hidden attributes (e.g., `.DS_Store`, `com.apple.quarantine`, custom icons). The client prevents Finder errors by pretending these writes succeed (`reply.ok()`), but it does not store this metadata on the server.

