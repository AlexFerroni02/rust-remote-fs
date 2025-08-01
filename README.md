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

## Main FUSE Filesystem Functions

- **getattr**: Given an inode, returns the attributes (size, permissions, type, timestamps, etc.) of the file or directory.
- **lookup**: Searches for a directory entry by name within a parent directory. If it exists, returns the inode and attributes; otherwise, returns an error.
- **readdir**: Lists the contents of a directory, returning for each entry its name, inode, and type (file or directory).
- **read**: Returns the content (or a portion) of a remote file given its inode, offset, and requested size.
- **write**: Writes data to a remote file starting at a given offset.

These functions allow the kernel to navigate and manipulate the remote filesystem as if it were local.

## Command Flows (Tested & Working)

### 📁 **`ls` Command Flows**
da usare con bin perche senno cerca anche un file ls nel codice va aggiunto che i comandi non vanno cercati, pero per ora per capire il flusso meglio provare cosi
#### **Case 1: `/bin/ls` (current directory)**
```
1. Shell calls → FUSE readdir(inode_of_current_directory)
2. Client maps inode → path using inode_to_path cache
3. Client sends → GET /list/{current_path} to server
4. Server returns → ["file1.txt", "ciao/"]
5. Client processes response and returns directory entries
6. Shell displays: file1.txt  dir1/  dir2/
```

#### **Case 2: `/bin/ls dir1` (specific directory)**
```
1. Shell calls → FUSE lookup(current_inode, "dir1")
   - Client queries server → GET /list/{current_path}
   - Finds "dir1/" in response, recognizes as directory
   - Creates inode mapping: inode_to_path[new_inode] = "dir1"
   - Saves: inode_to_type[new_inode] = FileType::Directory

2. Shell calls → FUSE getattr(dir1_inode)
   - Client returns FileType::Directory (confirms it's accessible)

3. Shell calls → FUSE readdir(dir1_inode)
   - Client maps dir1_inode → "dir1" path
   - Client sends → GET /list/dir1 to server
   - Server returns → ["subfile.txt", "subdir/"]
   - Client processes and returns entries

4. Shell displays: subfile.txt  subdir/
```
#### **Case 3: `/bin/ls dir1/dir2` (nested directory)**
```
1. Shell calls → FUSE lookup(current_inode, "dir1")
   - Client finds/creates inode for "dir1"
   - Returns dir1_inode

2. Shell calls → FUSE lookup(dir1_inode, "dir2")  
   - Client maps dir1_inode → "dir1" path
   - Client sends → GET /list/dir1 to server
   - Finds "dir2/" in response
   - Creates inode mapping: inode_to_path[new_inode] = "dir1/dir2"
   - Saves: inode_to_type[new_inode] = FileType::Directory

3. Shell calls → FUSE getattr(dir2_inode)
   - Client returns FileType::Directory

4. Shell calls → FUSE readdir(dir2_inode)
   - Client maps dir2_inode → "dir1/dir2" path  
   - Client sends → GET /list/dir1/dir2 to server
   - Server returns directory contents
   - Client processes and returns entries

5. Shell displays contents of dir1/dir2/
```



# test per provare il filesystem a mano (capire anche come aggiungere i test Rust)

## Operazioni sui file (funzionano)
  # Test creazione file (create + write)
echo "Hello World" > test_file.txt
echo "Line 2" >> test_file.txt   # Append (non ancora supportato)

# Test lettura file (read)
/bin/cat test_file.txt
/bin/head test_file.txt
/bin/tail test_file.txt

# Test creazione file vuoto
/bin/touch empty_file.txt
/bin/ls -la empty_file.txt

# Test sovrascrittura
echo "New content" > test_file.txt
/bin/cat test_file.txt           # Dovrebbe mostrare solo "New content"

# Test con caratteri speciali
echo "Special chars: àèìòù €" > special.txt
/bin/cat special.txt


## Test creazione directory (mkdir)
/bin/mkdir new_dir
/bin/mkdir -p nested/deep/path   # Creazione ricorsiva (se supportata)
/bin/ls -la                      # Verifica creazione

## Test navigazione nelle nuove directory
cd new_dir
/bin/pwd
echo "File in subdir" > file_in_subdir.txt
/bin/ls -la
cd ..

per eliminare provare rmdir nome_cartella
 o rm nome_file



 
# DA AGGIUNGERE

- Attualmente, la seconda operazione `echo "Line 2" >> test_file.txt` sovrascrive il contenuto invece di aggiungere il testo in coda. Questo comportamento va corretto modificando la funzione `write` per gestire correttamente l'operazione di append, utilizzando il parametro offset.

- È necessario implementare attributi realistici per i file al momento della loro creazione, in modo che mostrino informazioni corrette come dimensione effettiva e timestamp appropriati, invece dei valori predefiniti attualmente utilizzati.

- Manca la funzionalità di rinominazione dei file e delle directory: è necessario implementare la funzione `rename` nel filesystem per supportare il comando `mv`.

- Le mappe attualmente create potrebbero essere ottimizzate, riducendone il numero e memorizzando direttamente tutti gli attributi di un file o di una directory come valore, invece di mantenere solo il tipo in mappe separate.

- Bisogna valutare se esistano altre strategie di cache da implementare, dato che alcune funzioni potrebbero inviare troppe richieste al server in modo ridondante. È da verificare se l'approccio attuale con le mappe sia il più efficiente o se esistano alternative migliori per ridurre il carico di rete.

- Il mapping tra inode e path viene ricreato da zero ogni volta che si riavvia il client. Sarebbe opportuno implementare una soluzione più robusta e persistente che mantenga le informazioni dell'ultimo stato del filesystem, valutando se la struttura attuale delle mappe sia adeguata o necessiti modifiche.

- La gestione degli errori attuale presenta molti `unwrap()` che andrebbero sostituiti con un approccio più elegante se possibile. Inoltre, è necessario implementare test automatici per le API per garantire la robustezza del sistema.

- È importante verificare se vengano rispettate le specifiche richieste dal professore, in particolare la strategia configurabile di invalidazione della cache (`configurable cache invalidation strategy`) e il supporto per file di grandi dimensioni con lettura e scrittura in streaming (`large file streaming read/write`).

- Infine, va controllato se manchino altre funzionalità o se ci siano aspetti che potrebbero essere implementati diversamente per migliorare l'efficienza e la robustezza del sistema

## Test Suite

### Overview
The test suite ensures full coverage of the server's RESTful API and the client-side FUSE filesystem. It includes tests for:
1. **Health Check**: Verifies the `/health` endpoint.
2. **Directory Listing**: Tests `/list/` and `/list/<path>` for root, nested, and empty directories.
3. **File Operations**: Tests `/files/<path>` for reading, writing, overwriting, and deleting files.
4. **Directory Operations**: Tests `/mkdir/<path>` for creating directories and `/files/<path>` for deleting directories.
5. **Error Handling**: Tests invalid paths and ensures proper error codes are returned.

### Running the Tests
1. Navigate to the `server` directory:
   ```bash
   cd ~/projects/REPO_NAME/server
   ```

2. Run the tests:
   ```bash
   cargo test
   ```

3. Example output:
   ```
   running 10 tests
   test endpoints_tests::test_health_endpoint ... ok
   test endpoints_tests::test_list_root_directory ... ok
   ...
   ```

---

## Running Tests with Coverage

### Install `cargo-tarpaulin`
1. Install the `cargo-tarpaulin` tool:
   ```bash
   cargo install cargo-tarpaulin
   ```

### Run Tests with Coverage
1. Run the tests with coverage:
   ```bash
   cargo tarpaulin
   ```

2. Generate an HTML report:
   ```bash
   cargo tarpaulin --test endpoints --out Html --output-dir ./coverage
   ```

3. Open the report:
   ```bash
   xdg-open tarpaulin-report.html
   ```

### Notes
- Ensure the `reqwest` crate is configured with the `json` feature in `Cargo.toml`:
   ```toml
   reqwest = { version = "0.12.22", features = ["json"] }
   ```
- If using `tokio` or async tests, you may need to enable the `--force` flag:
   ```bash
   cargo tarpaulin --force
   ```

---

## Additional Notes
- The test suite is located in `server/tests/endpoints.rs`.
- The client-side FUSE filesystem can be tested manually using standard file commands (`ls`, `cat`, `echo`, etc.) on the mounted directory.
```