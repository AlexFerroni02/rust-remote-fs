# 💻 Client Documentation (FUSE)

Il client è un driver filesystem user-space che traduce le chiamate del kernel in richieste HTTP. È progettato per massimizzare la reattività e minimizzare la latenza di rete.

## 🏗 Stack Tecnologico

* **Core:** `fuser` (Rust bindings per libfuse).
* **Runtime:** `Tokio` (Multi-thread runtime per operazioni HTTP).
* **HTTP:** `Reqwest` (Client HTTP asincrono).
* **Caching:** `lru` e `HashMap` custom.

## ⚡️ Strategie di Ottimizzazione

### 1. Cache-on-Write (Buffer di Scrittura)
Le operazioni di scrittura (`write`) **non** contattano il server immediatamente.
* **Logica:** I dati vengono scritti in un buffer in memoria (`OpenWriteFile.buffer`) indicizzati per offset.
* **Commit:** Solo alla chiusura del file (`release`), il client:
    1. Scarica il file originale (se necessario).
    2. Applica le patch dal buffer.
    3. Esegue l'upload completo (`PUT`).
* **Vantaggio:** Editing fluido e veloce (zero latenza di rete durante la digitazione).

### 2. Chunked Reading (Range Requests)
Le operazioni di lettura (`read`) sfruttano l'header HTTP `Range`.
* Invece di scaricare interi file, il client richiede solo i byte specifici richiesti dal kernel.
* Permette la riproduzione immediata di file multimediali e l'apertura rapida di file di grandi dimensioni.

### 3. Gestione Inode Effimeri
Il server remoto non espone inode persistenti. Il client li genera dinamicamente:
* Mantiene una mappa bidirezionale `path <-> inode`.
* Gli inode sono validi solo per la durata della sessione di mount.
* Supporta attributi "faked" per UID/GID per garantire la compatibilità con il sistema operativo ospite.

### 4. Gestione macOS (Quirks)
Il client intercetta specificamente le chiamate relative agli attributi estesi (`xattr`) usate da macOS Finder (`com.apple.*`).
* Queste chiamate vengono gestite localmente (rispondendo "OK" o "Not Found") senza contattare il server.
* Questo previene errori grafici nel Finder e migliora drasticamente la velocità di navigazione su Mac.

## 🔄 Invalidazione Cache
Il client mantiene una connessione WebSocket persistente.
Quando riceve un messaggio `CHANGE`:
1.  Verifica che la modifica non provenga da se stesso (tramite ID univoco generato all'avvio).
2.  Acquisisce il lock sul filesystem.
3.  Rimuove l'entry corrispondente dalla `AttributeCache`.
4.  La successiva operazione `getattr` o `read` forzerà un fetch aggiornato dal server.

## 📦 Dipendenze e Librerie

Ecco l'analisi delle librerie utilizzate nel `Cargo.toml` e il ruolo che svolgono nel client FUSE:

* **`fuser`** (`0.11.0`): Binding Rust per `libfuse`. È la libreria core che permette di implementare il tratto `Filesystem`, intercettando le chiamate del kernel (open, read, write) e gestendole in user-space.
* **`tokio`** (`1.37.0`): Runtime asincrono. Sebbene FUSE sia sincrono, il client deve fare chiamate HTTP (asincrone). Tokio viene istanziato manualmente dentro `RemoteFS` per eseguire queste chiamate tramite `block_on`.
* **`reqwest`** (`0.12.4`): Client HTTP. Usato per tutte le comunicazioni REST col server (`GET`, `PUT`, `DELETE`). La configurazione `rustls-tls` assicura una gestione sicura e moderna della crittografia SSL/TLS.
* **`tokio-tungstenite`** (`0.21`): Client WebSocket. Gestisce la connessione persistente per ricevere le notifiche `CHANGE` dal server in tempo reale.
* **`lru`** (`0.12`): Implementa la cache **Least Recently Used**. È usata nella `AttributeCache` quando la strategia è impostata su "lru", per mantenere in memoria solo gli attributi dei file usati più di recente e risparmiare RAM.
* **`libc`** (`0.2.155`): Fornisce i tipi C grezzi e le costanti di errore (es. `ENOENT`, `EIO`). Necessario perché FUSE comunica col kernel usando codici di errore POSIX standard.
* **`bytes`** (`1.10.1`): Utility per la gestione efficiente dei buffer di byte contigui. Usata per manipolare i chunk di dati scaricati o da caricare senza copie di memoria superflue.
* **`serde`** / **`serde_json`**: Usati per parsare le risposte JSON del server (es. listing directory) e per leggere il file di configurazione.
* **`toml`** (`0.8`): Usato specificamente per deserializzare il file `config.toml` nella struct `Config` all'avvio.
* **`clap`** (`4.5`): Parser per gli argomenti da riga di comando. Gestisce il parsing del punto di mount (es. `cargo run -- /tmp/mountpoint`).
* **`futures-util`** (`0.3`): Utility per flussi asincroni, necessaria per gestire lo stream di messaggi in arrivo dal WebSocket.

### Dettaglio Struttura CLIENT (`client/`)
Il client è molto più articolato perché deve implementare l'interfaccia FUSE. Il codice è diviso in **moduli funzionali** dentro la cartella `fs/`.

#### 📂 Albero delle Directory
```text
client/
├── Cargo.toml          # Dipendenze
├── config.toml         # (Opzionale) Configurazione runtime
└── src/
    ├── main.rs         # Entry Point e WebSocket Thread
    ├── config.rs       # Parsing della configurazione
    ├── api_client.rs   # Livello di astrazione Rete (HTTP)
    └── fs/             # Implementazione Core del Filesystem
        ├── mod.rs      # Strutture dati principali (RemoteFS) e Dispatcher
        ├── prelude.rs  # Export comuni
        ├── cache.rs    # Logica LRU/TTL
        ├── read.rs     # Operazioni di lettura (open, read, lookup)
        ├── write.rs    # Operazioni di scrittura (write, release)
        ├── create.rs   # Creazione file/dir (create, mkdir)
        ├── delete.rs   # Cancellazione (unlink, rmdir)
        ├── rename.rs   # Spostamento (rename)
        ├── attr.rs     # Metadati (getattr, setattr)
        └── xattr.rs    # Attributi estesi (macOS quirks)

```

#### 📍 Dove sono le funzioni?**1. Livello Infrastruttura (`src/`)**

* **`main.rs`**:
* Parsa gli argomenti CLI (mountpoint).
* Monta il filesystem con `fuser::mount2`.
* **Thread WebSocket**: Spawna un thread separato che ascolta `ws://server/ws`, riceve i messaggi `CHANGE` e invalida la cache in `fs`.


* **`api_client.rs`**:
* Contiene tutte le chiamate `reqwest` (`get`, `put`, `delete`, `patch`).
* Implementa la logica di **Chunked Reading** (`get_file_chunk_from_server`).



**2. Il Cuore (`src/fs/mod.rs`)**

* Definisce la struct **`RemoteFS`**: Contiene le mappe Inode (`inode_to_path`), il client HTTP, la cache attributi e il buffer di scrittura.
* Implementa il trait **`Filesystem`**: Riceve tutte le chiamate FUSE dal kernel e le "smista" ai sottomoduli (es. `fn read` chiama `read::read`).

**3. Moduli Funzionali (`src/fs/*.rs`)**

* **`read.rs`**:
* `lookup`: Chiamata quando il sistema cerca un file per nome. Contatta il server (`/list`) e genera un Inode.
* `read`: Intercetta la lettura dei byte. Chiama `api_client::get_file_chunk_from_server` per scaricare solo il pezzo richiesto.


* **`write.rs`**:
* `open`: Se il file è aperto in scrittura, crea una entry nella mappa `open_files`.
* `write`: **Non chiama la rete**. Salva i dati nel buffer RAM (`OpenWriteFile.buffer`).
* `release`: Unisce i dati del buffer con il file originale e fa l'upload (`PUT`).


* **`attr.rs`**:
* `getattr`: Controlla prima `fs.attribute_cache`. Se manca (Cache Miss), fa una richiesta di rete.


* **`rename.rs`**:
* Implementa la logica "Move" lato client: Copia (Download+Upload) -> Cancella vecchio.


* **`cache.rs`**:
* Gestisce la logica di scadenza (TTL) o rimozione (LRU) delle entry cachate.

