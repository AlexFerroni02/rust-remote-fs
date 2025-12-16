# 🦀 Rust Remote Filesystem (FUSE)

Un filesystem di rete ad alte prestazioni scritto interamente in **Rust**.
Questo progetto implementa un client **FUSE** (Filesystem in Userspace) che permette di montare una cartella remota come se fosse un disco locale, sincronizzando i file in tempo reale con un server centrale via HTTP e WebSocket.

## ✨ Caratteristiche Principali

* **🚀 Performance Elevate:**
    * **Cache-on-Write:** Le scritture avvengono in RAM e vengono caricate sul server asincronamente alla chiusura del file.
    * **Chunked Reading:** Utilizza HTTP Range Requests per scaricare solo le parti di file necessarie (streaming video/file grandi supportati con basso uso di RAM).
    * **Metadata Caching:** Cache attributi (TTL/LRU) per navigazione istantanea.
* **🔄 Sincronizzazione Real-Time:**
    * Utilizza **WebSocket** per invalidare la cache dei client istantaneamente quando un file viene modificato da un altro utente.
    * **Echo Suppression:** Previene il loop di notifiche (il client ignora le modifiche fatte da se stesso).
* **🛠 Compatibilità Cross-Platform:** Testato e funzionante su **Linux** e **macOS** (gestione quirks Finder/xattr).
* **🔒 Architettura Robusta:** Server basato su `Axum` e Client basato su `fuser` e `tokio`.

## 📦 Requisiti

* **Rust** (ultima versione stabile): [Installazione](https://rustup.rs/)
* **Librerie FUSE:**
    * *Linux:* `libfuse-dev` (Ubuntu/Debian: `sudo apt install libfuse-dev fuse3`)
    * *macOS:* [macFUSE](https://osxfuse.github.io/)

## 🚀 Quick Start

### 1. Avviare il Server
Il server gestisce lo storage dei file e le notifiche WebSocket.

```bash
# Terminale 1
cd server
cargo run
# Il server ascolterà su 0.0.0.0:8080
# I file verranno salvati nella cartella ./data
```

### 2. Avviare il Client
Il client monta il filesystem remoto in una cartella locale (/tmp/mountpoint in questo caso).
```bash
# Terminale 2
cd client
# Crea il mountpoint
mkdir -p /tmp/mountpoint
# Avvia il client
cargo run -- /tmp/mountpoint
# Avvia il client specificando la cache
cargo run -- /tmp/mountpoint --cache-strategy lru --cache-lru-capacity 3
cargo run -- /tmp/mountpoint --cache-strategy ttl --cache-ttl-seconds 5
```

### 3. Smontare il Filesystem
Per terminare correttamente:
```bash
# Linux
fusermount -u /tmp/mountpoint
```

## 📂 Struttura del Progetto
/client: Codice sorgente del driver FUSE. Gestisce la cache locale, le chiamate syscall e la comunicazione HTTP con il server.

/server: Codice sorgente del server API REST + WebSocket. Gestisce lo storage su disco e il broadcasting delle modifiche.

## ⚠️ Note Tecniche
Questo progetto è stato sviluppato a scopo didattico/sperimentale. Sebbene supporti operazioni complesse come rename atomici (simulati client-side) e gestione concorrente, non è inteso per ambienti di produzione critici senza ulteriore hardening.