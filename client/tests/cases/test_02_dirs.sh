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

test_command_fails() {
  local description=$1
  local command=$2
  echo -n "  - Test (atteso fallimento): $description..."
  local output; local error_output
  output=$(eval "$command" 2> >(error_output=$(cat); cat >&2))
  local exit_code=$?
  if [ "$exit_code" -ne 0 ]; then
    echo -e "\e[32m PASS (fallito come previsto)\e[0m"
  else
    echo -e "\e[31m FAIL (ha avuto successo)\e[0m"
    echo "    ----------------------------------------------------"
    echo "    REASON: Il comando doveva fallire ma ha avuto successo (exit code 0)."
    echo "    COMMAND: $command"
    [ -n "$output" ] && echo "    STDOUT:" && echo "$output" | sed 's/^/    | /'
    echo "    ----------------------------------------------------"
    FAILED_TESTS=$((FAILED_TESTS + 1))
  fi
}

# --- Esecuzione dei Test ---

# 1. Creazione e Struttura
test_command "Creare una directory 'dir1'" "mkdir dir1 && [ -d dir1 ]"
test_command "Creare una struttura di directory annidata con '-p'" "mkdir -p dir1/subdir/subsubdir && [ -d dir1/subdir/subsubdir ]"
test_command "Listare il contenuto per verificare la creazione" "ls dir1 | grep -q 'subdir'"

# 2. Spostamento e Gestione Contenuto
test_command "Creare un file da spostare" "echo 'contenuto da spostare' > file_da_spostare.txt"
test_command "Spostare il file dentro 'dir1/subdir'" "mv file_da_spostare.txt dir1/subdir/"
test_command "Verificare che il file sia nella nuova posizione" "[ -f dir1/subdir/file_da_spostare.txt ]"
test_command "Verificare che il file non sia più nella vecchia posizione" "[ ! -f file_da_spostare.txt ]"

# 3. Rimozione e Casi Limite
test_command "Rimuovere una directory vuota con 'rmdir'" "rmdir dir1/subdir/subsubdir"
test_command_fails "Fallire nel rimuovere una directory non vuota con 'rmdir'" "rmdir dir1/subdir"
test_command_fails "Fallire nel rimuovere un file con 'rmdir'" "rmdir dir1/subdir/file_da_spostare.txt"
test_command "Rimuovere una directory e il suo contenuto con 'rm -r'" "rm -r dir1"
test_command "Verificare che la directory non esista più" "[ ! -d dir1 ]"

# --- Esito Finale ---
exit $FAILED_TESTS