#!/bin/bash
set -euo pipefail

# --- Configurazione ---
BASE_DIR=$(dirname "$0")
# `readlink -f` ottiene il percorso assoluto e risolve i link simbolici
CLIENT_PROJECT_DIR=$(readlink -f "$BASE_DIR/..")
SERVER_PROJECT_DIR=$(readlink -f "$BASE_DIR/../../server")

MOUNT_POINT="/tmp/remote_fs_test_mount"
SERVER_LOG="/tmp/server.log"
CLIENT_LOG="/tmp/client.log"

if [ -t 1 ]; then
  # Se è un terminale, definisci i codici colore
  COLOR_INFO='\e[34m'
  COLOR_SUCCESS='\e[32m'
  COLOR_FAIL='\e[31m'
  COLOR_RESET='\e[0m'
else
  # Se non è un terminale (es. un file), lascia le variabili vuote
  COLOR_INFO=''
  COLOR_SUCCESS=''
  COLOR_FAIL=''
  COLOR_RESET=''
fi
# --- Funzioni di Utility ---
info() { echo -e "${COLOR_INFO}INFO: $1${COLOR_RESET}"; }
success() { echo -e "${COLOR_SUCCESS}✔ SUCCESS: $1${COLOR_RESET}"; }
fail() { echo -e "${COLOR_FAIL}✖ FAILURE: $1${COLOR_RESET}"; }

cleanup() {
  info "Pulizia in corso..."
  umount -l "$MOUNT_POINT" 2>/dev/null || true
  pkill -f "target/debug/server" 2>/dev/null || true
  pkill -f "target/debug/client" 2>/dev/null || true

  # Rimuovi la directory di mount e i log
  rm -rf "$MOUNT_POINT"
  rm -f "$SERVER_LOG" "$CLIENT_LOG"
  rm -rf "$SERVER_PROJECT_DIR/data/"*
  info "Pulizia completata."
}
trap cleanup EXIT

# --- Preparazione Ambiente ---
info "Preparazione dell'ambiente di test..."
info "Compilazione di server e client..."
cargo build --manifest-path="$SERVER_PROJECT_DIR/Cargo.toml" --quiet
cargo build --manifest-path="$CLIENT_PROJECT_DIR/Cargo.toml" --quiet

mkdir -p "$MOUNT_POINT"

info "Avvio del server..."
"$SERVER_PROJECT_DIR/target/debug/server" &> "$SERVER_LOG" &

info "Avvio del client FUSE..."
"$CLIENT_PROJECT_DIR/target/debug/client" "$MOUNT_POINT" &> "$CLIENT_LOG" &

info "Attesa che il mount point sia pronto..."
timeout=10
while ! mount | grep -q "$MOUNT_POINT"; do
  sleep 0.5
  timeout=$((timeout - 1))
  if [ "$timeout" -eq 0 ]; then
    fail "Mount point non pronto dopo 10 secondi."
    cat "$CLIENT_LOG"
    exit 1
  fi
done
success "Mount point pronto!"
echo "-------------------------------------------"

# --- Esecuzione dei Test ---
FAILED_COUNT=0
# Esporta la variabile MOUNT_POINT per renderla disponibile agli script di test
export MOUNT_POINT

for test_file in "$BASE_DIR"/cases/test_*.sh; do
  info "Esecuzione di: $(basename "$test_file")"
  if bash "$test_file"; then
    success "Il gruppo di test '$(basename "$test_file")' è stato superato."
  else
    fail "Il gruppo di test '$(basename "$test_file")' è fallito."
    FAILED_COUNT=$((FAILED_COUNT + 1))
  fi
  echo "-------------------------------------------"
done

# --- Riepilogo Finale ---
if [ "$FAILED_COUNT" -eq 0 ]; then
  success "TUTTI I GRUPPI DI TEST SONO STATI SUPERATI!"
  exit 0
else
  fail "$FAILED_COUNT gruppo/i di test fallito/i."
  exit 1
fi
