
## End-to-End (E2E) Test Documentation

### 1. Overview

This document describes the end-to-end (e2e) test suite for the remote FUSE filesystem. The tests are designed to verify that the FUSE client (`client`) and the Axum HTTP server (`server`) work together correctly, simulating real-world user operations from a `bash` shell.

The tests run directly against a mounted FUSE filesystem, executing standard POSIX commands (e.g., `touch`, `echo`, `ls`, `rm`, `mv`, `mkdir`, `chmod`).

---

### 2. Test Environment & Runner

The entire test suite is managed by the main `run_e2e_tests.sh` script.

#### 2.1. Environment Setup

Before any test case is run, the script performs the following setup:

1.  **Build Projects:** It compiles both the `server` and `client` projects using `cargo build`.
2.  **Start Server:** It launches the Axum `server` in the background. Server logs are redirected to `/tmp/server.log`.
3.  **Create Mountpoint:** It creates a temporary directory at `/tmp/remote_fs_test_mount`.
4.  **Start Client:** It launches the FUSE `client` and mounts it at the `/tmp/remote_fs_test_mount` mountpoint. Client logs are redirected to `/tmp/client.log`.
5.  **Wait for Mount:** The script enters a wait loop, checking `mount` to ensure the FUSE filesystem is fully mounted and ready before proceeding.

#### 2.2. Test Execution

The runner script (`run_e2e_tests.sh`) discovers and executes all test case files matching `cases/test_*.sh`.

It exports the `$MOUNT_POINT` variable, which all test cases use to `cd` into the mounted filesystem.

#### 2.3. Environment Teardown (Cleanup)

A `trap` command ensures that the `cleanup` function runs on script exit (whether pass or fail). This function guarantees a clean state for the next run by:

1.  Forcibly unmounting the filesystem (`umount -l`).
2.  Killing any lingering `server` or `client` processes.
3.  Deleting the mountpoint directory.
4.  Deleting the server's `data` directory to clear all stored files.
5.  Deleting all log files.

---

### 3. Test Suites & Scenarios

The tests are broken into logical groups by file.

#### 3.1. `test_01_files.sh`: Basic File Lifecycle

This test suite validates the complete lifecycle of a single file, covering all fundamental I/O operations (create, read, write, append, overwrite).

* **Test: Create with `touch`**
    * **Command:** `touch file_principale.txt`
    * **Verifies:** The FUSE `create` handler is called and successfully creates an empty file on the server.

* **Test: Write (Create/Truncate)**
    * **Command:** `echo 'linea 1' > file_principale.txt`
    * **Verifies:** The `create` (or `open` with truncate) and `write` handlers work. The `release` handler correctly uploads the content ("linea 1").

* **Test: Read (Initial Content)**
    * **Command:** `cat file_principale.txt`
    * **Verifies:** The `read` handler correctly fetches the content from the server.

* **Test: Write (Append)**
    * **Command:** `echo 'linea 2' >> file_principale.txt`
    * **Verifies:** The `open` handler (detecting append mode), `write` (at an offset), and `release` (merging old and new content) work correctly.
    * **Note:** This is a critical test for the "Read-Modify-Write" logic in the `release` handler.

* **Test: Read (After Append)**
    * **Command:** `cat file_principale.txt`
    * **Verifies:** The content is now "linea 1\nlinea 2".

* **Test: Write (Overwrite - Shorter)**
    * **Command:** `echo 'sovrascritto' > file_principale.txt`
    * **Verifies:** The `open` (with truncate) and `write` handlers correctly overwrite a longer file with shorter content.

* **Test: Write (Overwrite - Longer)**
    * **Command:** `echo 'contenuto molto più lungo...' > file_principale.txt`
    * **Verifies:** The `open` (with truncate) and `write` handlers correctly overwrite a shorter file with longer content.

* **Test: Copy (`cp`)**
    * **Command:** `cp file_principale.txt copia.txt`
    * **Verifies:** A complex operation that tests `read` (on the source) and `create`/`write`/`release` (on the destination).

* **Test: Delete (`rm`)**
    * **Command:** `rm file_principale.txt`
    * **VerFUSEifies:** The `unlink` handler is called and successfully deletes the file from the server.

#### 3.2. `test_02_directories_and_advanced.sh`: Directory Structure

This suite tests operations related to directory hierarchy and metadata.

* **Test: `readdir` (List Directory)**
    * **Command:** `mkdir test_dir`, `touch test_dir/file1.txt`, `ls test_dir`
    * **Verifies:** The `mkdir`, `create`, and `readdir` handlers work together. `ls` can see the entries.

* **Test: Recursive `mkdir`**
    * **Command:** `mkdir -p nested/dir1/dir2`
    * **Verifies:** The `mkdir` handler is called multiple times and can create nested structures. (The server handles this via `fs::create_dir_all`).

* **Test: `create` (Nested File)**
    * **Command:** `touch nested/dir1/dir2/file.txt`
    * **Verifies:** File creation works in subdirectories.

* **Test: `getattr` (Timestamp)**
    * **Command:** `touch file_test.txt`
    * **Verifies:** The `getattr` handler (or `create`) correctly reports a recent timestamp.

* **Test: Error Handling (404)**
    * **Command:** `! cat non_existent_file.txt`
    * **Verifies:** Attempting to `read` a file that does not exist fails with an error (non-zero exit code).

#### 3.3. `test_03_attributes_and_server.sh`: Metadata & Server Health

This suite tests metadata (`setattr`, `getattr`) and server error states.

* **Test: `getattr` (File Size)**
    * **Command:** `echo 'test content' > file_test.txt`, `stat -c '%s' file_test.txt`
    * **Verifies:** The `getattr` handler correctly reports the file size (13 bytes) after a write.

* **Test: `setattr` (Permissions)**
    * **Command:** `chmod 666 file_test.txt`, `stat -c '%a' file_test.txt`
    * **Verifies:** The `setattr` handler is called and correctly sends a `PATCH` request to the server to change the file's mode. `getattr` reads the new mode.

* **Test: Server Error Handling (500)**
    * **Command:** `! curl -f -X GET http://localhost:8080/trigger_500`
    * **Verifies:** The server's test-only `/trigger_500` endpoint correctly returns a 500-level error, and `curl -f` fails as expected.

* **Test: Unmount**
    * **Command:** `umount '$MOUNT_POINT'`
    * **Verifies:** The filesystem can be unmounted cleanly. This test runs last as it shuts down the environment.

#### 3.4. Large File Stress Test

This test was added to the main `test_01_files.sh` script to validate the fix for large file handling.

* **Test: Create Large File (100MB)**
    * **Command:** `yes '0123456789' | head -c 100M > large_file.txt`
    * **Verifies:** The `create` -> `write` (many times) -> `release` pipeline. This specifically tests that the "Read-Modify-Write" logic in `release` can handle appends without crashing or OOM errors.

* **Test: Verify Large File Size**
    * **Command:** `[ $(stat -c%s large_file.txt) -eq 104857600 ]`
    * **Verifies:** The `getattr` handler correctly reports the exact size (100 * 1024 * 1024 bytes) of the large file created.