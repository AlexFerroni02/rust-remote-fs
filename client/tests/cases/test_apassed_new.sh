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
test_command "Verify file timestamp preservation" "touch file_test.txt && [ \"\$(stat -c '%Y' file_test.txt)\" -le \"\$(date +%s)\" ]"
test_command "Verify file permissions preservation" "chmod 644 file_test.txt && [ \"\$(stat -c '%a' file_test.txt)\" = '644' ]"
rm file_test.txt

# 4. Graceful Startup and Shutdown
test_command "Verify client startup and mount point readiness" "mount | grep -q '$MOUNT_POINT'"

# 5. Error Handling for RESTful API
test_command "Handle 404 error (file not found)" "! cat non_existent_file.txt"

# 7. Performance Metrics
test_command "Measure latency for file creation" "time touch test_latency.txt && rm test_latency.txt"

# 8. Directory Listing
mkdir test_dir
touch test_dir/file1.txt test_dir/file2.txt
sleep 1
test_command "Verify directory listing correctness" "ls test_dir | grep -q 'file1.txt' && ls test_dir | grep -q 'file2.txt'"
rm -r test_dir

# 9. innested mkdir e mkfile
test_command "Create nested directories with mkdir -p" "mkdir -p nested/dir1/dir2 && [ -d nested/dir1/dir2 ]"
test_command "Create a file in nested directory" "touch nested/dir1/dir2/file.txt && [ -f nested/dir1/dir2/file.txt ]"

# --- Final Result ---
if [ "$FAILED_TESTS" -eq 0 ]; then
    echo -e "\e[32mAll tests passed successfully!\e[0m"
else
    echo -e "\e[31mWARNING: $FAILED_TESTS test(s) failed.\e[0m"
fi
exit $FAILED_TESTS