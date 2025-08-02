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

# 1. Nomi di File e Directory Particolari
test_command "Creare un file con spazi nel nome" "touch 'file con spazi.txt' && [ -f 'file con spazi.txt' ]"
test_command "Scrivere in un file con spazi nel nome" "echo 'test' > 'file con spazi.txt'"
test_command "Creare un file nascosto (con il punto)" "touch .file_nascosto && [ -f .file_nascosto ]"
test_command "Verificare che 'ls' non mostri il file nascosto" "! ls | grep -q '.file_nascosto'"
test_command "Verificare che 'ls -a' mostri il file nascosto" "ls -a | grep -q '.file_nascosto'"
test_command "Rimuovere i file speciali" "rm 'file con spazi.txt' .file_nascosto"

# 2. Testare Funzionalità Mancanti (devono fallire)
test_command "Creare un file per i test di rename/chmod" "echo 'test' > file_per_test_avanzati.txt"
test_command_fails "Rinominare un file ('rename' non implementato)" "mv file_per_test_avanzati.txt file_rinominato.txt"
test_command_fails "Modificare i permessi ('setattr' non implementato)" "chmod 777 file_per_test_avanzati.txt"

# 3. Test di Carico Leggero
test_command "Creare 10 file in un ciclo" "for i in {1..10}; do touch file_ciclo_\$i.txt; done && [ \$(ls | grep file_ciclo_ | wc -l) -eq 10 ]"
test_command "Rimuovere i 10 file creati" "rm file_ciclo_*.txt"

# --- Esito Finale ---
exit $FAILED_TESTS