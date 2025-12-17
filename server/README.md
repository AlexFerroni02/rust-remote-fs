# 🖥️ Server Documentation

Il backend del progetto è costruito con **Rust** e **Axum**. Fornisce un'interfaccia RESTful per le operazioni sui file e un canale WebSocket per le notifiche di cambiamento in tempo reale.

## 🏗 Stack Tecnologico

* **Framework Web:** `Axum` (basato su Hyper/Tokio) per alte prestazioni e gestione asincrona.
* **I/O:** `Tokio` (FileSystem asincrono).
* **File Watcher:** `Notify` (Monitoraggio eventi OS).
* **Tracing:** `Tower-http` per il logging delle richieste.

## 🔌 API Endpoints

Il server espone le seguenti rotte su porta `8080`:

| Metodo | Endpoint | Descrizione | Note |
| :--- | :--- | :--- | :--- |
| `GET` | `/list/*path` | Lista contenuti directory | Ritorna JSON con metadati |
| `GET` | `/files/*path` | Legge contenuto file | Supporta **Range Requests** (206 Partial Content) |
| `PUT` | `/files/*path` | Scrive/Sovrascrive file | Richiede header `X-Client-ID` |
| `DELETE`| `/files/*path` | Elimina file o directory | Ricorsivo per le directory |
| `POST` | `/mkdir/*path` | Crea directory | Crea anche i padri (mkdir -p) |
| `PATCH` | `/files/*path` | Modifica permessi (chmod) | Payload JSON: `{"perm": "755"}` |
| `GET` | `/ws` | Endpoint WebSocket | Per notifiche real-time |

## 🧠 Logiche Chiave

### 1. Streaming I/O
Per minimizzare l'uso della RAM, sia la lettura (`GET`) che la scrittura (`PUT`) utilizzano stream asincroni (`ReaderStream` e `Body::from_stream`). Questo permette di gestire file di dimensioni arbitrarie (es. GB) con un footprint di memoria costante.

### 2. Echo Suppression (Watcher)
Il server utilizza un sistema intelligente per evitare loop di notifiche:
1.  Quando un client esegue una `PUT`/`DELETE`, il server registra l'evento in una mappa in memoria (`AppState.recent_mods`) associandolo al `X-Client-ID`.
2.  Il thread `Watcher` rileva la modifica sul disco.
3.  Prima di inviare il broadcast WebSocket, controlla se la modifica è stata causata da un client noto.
4.  Il messaggio inviato è taggato: `CHANGE:/path/file|BY:client-123`.
5.  Il client che riceve il messaggio controlla il tag e ignora le proprie modifiche.

### 3. Range Requests
L'endpoint `GET /files` implementa l'RFC 7233. Se riceve un header `Range: bytes=0-1023`, esegue un `seek` sul file locale e restituisce solo i byte richiesti. Fondamentale per le performance del client.

## 📦 Dipendenze e Librerie

Ecco l'analisi delle librerie utilizzate nel `Cargo.toml` e il motivo della loro scelta nel progetto:

* **`axum`** (`0.7.9`): Il framework web principale. È stato scelto per la sua modularità, l'integrazione nativa con Tokio e il supporto eccellente per i WebSocket (`features = ["ws"]`). Gestisce il routing HTTP e l'estrazione dei parametri dalle richieste.
* **`tokio`** (`1.37.0`): Il motore asincrono (Runtime). La feature `full` abilita il multi-threading, l'I/O asincrono e i timer. La feature `sync` fornisce i canali `broadcast` usati per sincronizzare il Watcher con i WebSocket.
* **`tokio-util`** (`0.7`): Fornisce utility per lavorare con I/O asincrono. Nello specifico, `ReaderStream` permette di convertire un `tokio::fs::File` in uno stream HTTP, abilitando il download di file senza caricarli in RAM.
* **`http-body-util`** (`0.1.3`): Utilizzato per manipolare i body delle richieste HTTP in modo efficiente (streaming), essenziale per l'upload (`PUT`) di file grandi.
* **`notify`** (`6.1.1`): Libreria cross-platform per il monitoraggio del filesystem. È il cuore del sistema di sincronizzazione: rileva le modifiche su disco per attivare le notifiche WebSocket.
* **`tracing`** / **`tracing-subscriber`**: L'infrastruttura di logging. Sostituisce i semplici `println!` offrendo log strutturati, livelli di priorità (debug, info, error) e filtraggio tramite variabili d'ambiente (`RUST_LOG`).
* **`tower-http`** (`0.6.6`): Middleware HTTP. Usato specificamente per il layer `TraceLayer`, che logga automaticamente ogni richiesta HTTP in ingresso e il relativo status code.
* **`serde`** (`1.0.219`): Framework di serializzazione. Usato per convertire automaticamente le struct Rust (come `RemoteEntry`) in JSON per le risposte API.
* **`futures-util`** (`0.3`): Fornisce metodi estesi (`split`, `next`) per lavorare con gli stream, fondamentali per gestire il ciclo di vita delle connessioni WebSocket.


---

###  Dettaglio Struttura SERVER (`server/`)
Il server è organizzato come un monolite modulare. La logica è separata tra **infrastruttura** (`main.rs`) e **business logic** (`handlers.rs`).

#### 📂 Albero delle Directory
```text
server/
├── Cargo.toml          # Gestione dipendenze
├── data/               # (Generata a runtime) Contiene i file fisici caricati
└── src/
    ├── main.rs         # Entry Point, Configurazione, Watcher, WebSocket
    └── handlers.rs     # Logica API REST (I/O su disco)

```

#### 📍 Dove sono le funzioni?**1. `src/main.rs` (L'Orchestratore)**
Questo file gestisce il ciclo di vita dell'applicazione e le connessioni persistenti.

* **Funzione `main()**`:
* Inizializza il logger (`tracing`).
* Crea la directory `./data`.
* Spawna il thread del **Watcher** (`notify`) che contiene la logica di *Echo Suppression* (filtro `|BY:client-id`).
* Configura le rotte di **Axum** (`Router::new()`).
* Avvia il server TCP.


* **Funzione `websocket_handler**`: Gestisce l'upgrade da HTTP a WebSocket.
* **Funzione `websocket**`: Loop asincrono che inoltra i messaggi dal canale broadcast (`tx`) al socket del client.

**2. `src/handlers.rs` (Il Lavoratore)**
Qui risiedono le funzioni che toccano fisicamente il disco. Ogni funzione corrisponde a una rotta HTTP.

* **Struct `AppState**`: Contiene lo stato condiviso (Canale TX per WebSocket e Mappa `recent_mods` per Echo Suppression).
* **Funzione `get_file**` (`GET /files/*`):
* Legge l'header `Range`.
* Esegue `file.seek()`.
* Restituisce uno stream (`ReaderStream`).


* **Funzione `put_file**` (`PUT /files/*`):
* Legge l'header `X-Client-ID`.
* Chiama `record_change` (per popolare la mappa anti-eco).
* Scrive il file in streaming (`body.frame()`).


* **Funzione `list_directory_contents**` (`GET /list`):
* Usa `fs::read_dir`.
* Mappa i risultati nella struct `RemoteEntry`.


* **Funzioni Helper**:
* `mkdir`: Crea directory ricorsivamente.
* `delete_file`: Rimuove file o cartelle.
* `patch_file`: Cambia i permessi (`chmod`).
