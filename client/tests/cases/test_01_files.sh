#!/bin/bash
set -o pipefail

# --- Configurazione Iniziale ---
cd "$MOUNT_POINT"
FAILED_TESTS=0

# --- Funzioni Helper Avanzate ---
test_command() {
  local description=$1
  local command=$2
  echo -n "  - Test: $description..."
  local output; local error_output
  output=$(eval "$command" 2> >(error_output=$(cat); cat >&2))
  local exit_code=$?
  if [ "$exit_code" -eq 0 ]; then
    echo -e "\e[32m PASS\e[0m"
  else
    echo -e "\e[31m FAIL\e[0m"
    echo "    ----------------------------------------------------"
    echo "    REASON: Command failed with exit code $exit_code."
    echo "    COMMAND: $command"
    [ -n "$output" ] && echo "    STDOUT:" && echo "$output" | sed 's/^/    | /'
    [ -n "$error_output" ] && echo "    STDERR:" && echo "$error_output" | sed 's/^/    | /'
    echo "    ----------------------------------------------------"
    FAILED_TESTS=$((FAILED_TESTS + 1))
  fi
}

test_content() {
  local description=$1
  local file_path=$2
  local expected_content=$3
  echo -n "  - Test: $description..."
  local diff_output; diff_output=$(diff -u <(echo -n "$expected_content") "$file_path")
  local exit_code=$?
  if [ "$exit_code" -eq 0 ]; then
    echo -e "\e[32m PASS\e[0m"
  else
    echo -e "\e[31m FAIL\e[0m"
    echo "    ----------------------------------------------------"
    echo "    REASON: File content does not match expected content."
    echo "    FILE: $file_path"
    echo "    DIFF (+ actual, - expected):"
    echo "$diff_output" | sed 's/^/    | /'
    echo "    ----------------------------------------------------"
    FAILED_TESTS=$((FAILED_TESTS + 1))
  fi
}

# --- Esecuzione dei Test ---
# Sequenza logica: creo, scrivo, leggo, aggiungo, leggo, sovrascrivo, leggo, copio, elimino

# 1. Creazione e Scrittura Iniziale
test_command "Creare un file con 'touch'" "touch file_principale.txt && [ -f file_principale.txt ]"
test_command "Scrivere contenuto iniziale con '>'" "echo 'linea 1' > file_principale.txt"
test_content "Verificare contenuto iniziale" "file_principale.txt" "linea 1"

# 2. Append (Aggiunta in coda)
test_command "Aggiungere contenuto con '>>'" "echo 'linea 2' >> file_principale.txt"
test_content "Verificare contenuto dopo l'append" "file_principale.txt" $'linea 1\nlinea 2'

# 3. Sovrascrittura
test_command "Sovrascrivere con contenuto più corto" "echo 'sovrascritto' > file_principale.txt"
test_content "Verificare contenuto dopo sovrascrittura corta" "file_principale.txt" "sovrascritto"

test_command "Sovrascrivere con contenuto più lungo" "echo 'contenuto molto più lungo del precedente' > file_principale.txt"
test_content "Verificare contenuto dopo sovrascrittura lunga" "file_principale.txt" "contenuto molto più lungo del precedente"

# 4. Copia e Rimozione
test_command "Copiare il file" "cp file_principale.txt copia.txt"
test_command "Verificare che la copia sia identica con 'diff'" "diff file_principale.txt copia.txt"
test_command "Rimuovere il file originale" "rm file_principale.txt"
test_command "Verificare che l'originale non esista più" "[ ! -f file_principale.txt ]"
test_command "Verificare che la copia esista ancora" "[ -f copia.txt ]"
test_command "Rimuovere la copia" "rm copia.txt"

# --- Esito Finale ---
exit $FAILED_TESTS