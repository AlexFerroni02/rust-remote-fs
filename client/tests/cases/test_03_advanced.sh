#!/bin/bash
set -o pipefail

# Assicurati che MOUNT_POINT sia impostato
if [ -z "$MOUNT_POINT" ]; then
    echo "Errore: la variabile d'ambiente MOUNT_POINT non è impostata."
    exit 1
fi

# --- Configurazione Iniziale ---
cd "$MOUNT_POINT"
echo "Esecuzione test nel punto di mount: $PWD"
FAILED_TESTS=0

# --- Funzioni Helper Avanzate ---
test_command() {
  local description=$1
  local command=$2
  echo -n "  - Test: $description..."
  # Usiamo un file temporaneo per catturare stderr senza interferire con l'output di eval
  local stderr_file=$(mktemp)
  local output; local error_output
  output=$(eval "$command" 2> "$stderr_file")
  local exit_code=$?
  error_output=$(cat "$stderr_file")
  rm "$stderr_file"
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

# --- Esecuzione dei Test ---
echo "--- Avvio dei Test ---"

# 1. Nomi di File e Directory Particolari
echo "Sezione 1: Test su nomi di file e directory particolari"
test_command "Creare un file con spazi nel nome" "touch 'file con spazi.txt' && [ -f 'file con spazi.txt' ]"
test_command "Scrivere in un file con spazi nel nome" "echo 'test' > 'file con spazi.txt'"
test_command "Verificare il contenuto del file" "[ \"\$(cat 'file con spazi.txt')\" = 'test' ]"
test_command "Creare un file nascosto (con il punto)" "touch .file_nascosto && [ -f .file_nascosto ]"
test_command "Verificare che 'ls' non mostri il file nascosto" "! ls | grep -q '.file_nascosto'"
test_command "Verificare che 'ls -a' mostri il file nascosto" "ls -a | grep -q '.file_nascosto'"
test_command "Rimuovere i file speciali" "rm 'file con spazi.txt' .file_nascosto"

# 2. Test Funzionalità Implementate (devono avere successo)
echo "Sezione 2: Test su funzionalità che ora dovrebbero avere successo"
# Questo file viene creato e rimosso esclusivamente per questi test
echo -n "  - Preparazione: Creare un file per i test di rename/chmod..."
touch file_per_test_avanzati.txt || {
  echo -e "\e[31m FALLITA\e[0m. Impossibile creare il file di test. Salto i test di funzionalità avanzate."
  FAILED_TESTS=$((FAILED_TESTS + 1))
}

if [ -f "file_per_test_avanzati.txt" ]; then
  test_command "Rinominare un file ('rename' implementato)" "mv file_per_test_avanzati.txt file_rinominato.txt && [ -f file_rinominato.txt ] && ! [ -f file_per_test_avanzati.txt ]"
  test_command "Modificare i permessi ('setattr' implementato)" "chmod 777 file_rinominato.txt && [ \"\$(stat -c '%a' file_rinominato.txt)\" = '777' ]"
  test_command "Modificare la dimensione ('setattr' implementato)" "truncate -s 100 file_rinominato.txt && [ \"\$(stat -c '%s' file_rinominato.txt)\" = '100' ]"
  test_command "Pulizia: Rimuovere il file di test rinominato" "rm file_rinominato.txt"
fi

# 3. Test di Carico Leggero
echo "Sezione 3: Test di carico leggero"
test_command "Creare 10 file in un ciclo" "for i in {1..10}; do touch file_ciclo_\$i.txt; done && [ \$(ls | grep file_ciclo_ | wc -l) -eq 10 ]"
test_command "Rimuovere i 10 file creati" "rm file_ciclo_*.txt"
test_command "Creare una directory e un file al suo interno" "mkdir 'dir_test' && touch 'dir_test/file_interno.txt' && [ -f 'dir_test/file_interno.txt' ]"
test_command "Rimuovere ricorsivamente la directory" "rm -r 'dir_test'"

# --- Esito Finale ---
echo "--- Riassunto dei Test ---"
if [ "$FAILED_TESTS" -eq 0 ]; then
  echo -e "\e[32mTutti i test completati con successo!\e[0m"
else
  echo -e "\e[31mATTENZIONE: $FAILED_TESTS test(s) falliti.\e[0m"
fi
exit $FAILED_TESTS