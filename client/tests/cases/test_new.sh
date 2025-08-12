#!/bin/bash
set -o pipefail

# Ensure MOUNT_POINT is set
if [ -z "$MOUNT_POINT" ]; then
    echo "Error: MOUNT_POINT environment variable is not set."
    exit 1
fi

cd "$MOUNT_POINT"
FAILED_TESTS=0

# Helper function for tests
test_command() {
    local description=$1
    local command=$2
    echo -n "  - Test: $description..."
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

# --- Tests ---

# 1. File Attributes
test_command "Verify file size preservation" "echo 'test content' > file_test.txt"
sleep 0.5
test_command "Verify file size preservation" "[ \"\$(stat -c '%s' file_test.txt)\" -eq 13 ]"
test_command "Verify file permissions preservation" "chmod 666 file_test.txt && [ \"\$(stat -c '%a' file_test.txt)\" = '666' ]"
rm file_test.txt

# 3. Streaming Read/Write for Large Files
#test_command "Create a large file (100MB)" "dd if=/dev/zero of=large_file.txt bs=1M count=100 && [ -f large_file.txt ]"
#test_command "Verify large file size" "[ \"\$(stat -c '%s' large_file.txt)\" -eq \$((100 * 1024 * 1024)) ]"
#rm large_file.txt

# 4. Graceful Startup and Shutdown
test_command "Verify client shutdown and unmount" "umount '$MOUNT_POINT' && ! mount | grep -q '$MOUNT_POINT'"

# 5. Error Handling for RESTful API
test_command "Handle 500 error (server internal error)" "! curl -X GET http://localhost:8080/trigger_500"


# --- Final Result ---
if [ "$FAILED_TESTS" -eq 0 ]; then
    echo -e "\e[32mAll tests passed successfully!\e[0m"
else
    echo -e "\e[31mWARNING: $FAILED_TESTS test(s) failed.\e[0m"
fi
exit $FAILED_TESTS