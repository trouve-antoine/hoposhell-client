# Hoposhell client

`hopo` is a Rust reverse-shell client for
[hoposhell](https://www.hoposhell.com).
It starts a local shell in a pseudo-terminal (PTY) and makes an **outbound**
connection to the backend, which connects it to the Hoposhell web terminal.
The shell machine does not need an inbound listening port.

Only run it on machines you own or are authorized to administer. Anyone
authorized to control the session can execute commands with the client's OS
user privileges.

## Features

- Interactive terminal through the backend, including terminal resizing.
- Certificate-based shell identity and TLS transport.
- Optional reconnection without restarting the local shell.
- Recent terminal-output replay after reconnecting.
- Structured remote commands: file metadata/globbing, file downloads, HTTP
  requests, TCP requests, and execution of named local scripts.
- A localhost request/response TCP-forwarding helper.
- Configurable buffer limits that terminate the client on overflow.

The current implementation targets **Linux and macOS** and uses Unix APIs.
It is synchronous and thread-based. It is not an SSH client or a standalone
terminal server.

## Install

Install Rust and Cargo with [rustup](https://rustup.rs/). The current locked
dependencies require Rust **1.88 or newer**; a current stable toolchain is
recommended.

Native build tools are needed because OpenSSL is built from vendored sources
and zstd includes native code.

On Debian/Ubuntu:

```sh
sudo apt-get install build-essential pkg-config perl
```

On macOS, install the Xcode Command Line Tools if needed:

```sh
xcode-select --install
```

Then build and install from source:

```sh
git clone https://github.com/trouve-antoine/hoposhell-client.git
cd hoposhell-client
rustup update stable
cargo +stable install --path . --locked
hopo version
```

Cargo normally installs `hopo` into `~/.cargo/bin`; ensure that directory is on
your `PATH`.

For a local build or tests instead:

```sh
cargo +stable build --release --locked
cargo +stable test --locked
```

The release executable is `target/release/hopo`. There is no need to install
the backend on the same machine; it must be deployed and reachable separately.

## Set up and connect

First create/select a shell in the Hoposhell GUI, then enroll its client:

```sh
hopo setup shell_example
```

Enter the login code displayed in the GUI. Setup downloads:

```text
~/.hoposhell/server.crt          Backend certificate
~/.hoposhell/shell_example.pem   Client certificate and private-key bundle
```

Setup enforces `0700` on the Hoposhell directory and writes both credential
files atomically with `0600` permissions. Symlink destinations are rejected.
Existing custom parent directories keep their permissions; newly created
credential directories use `0700`. Existing credential files are secured when
setup is rerun.

Start the shell:

```sh
hopo connect shell_example
```

`hopo shell_example` is shorthand for the same command. With no command,
`hopo` defaults to connecting. If there is no selected shell/key, connection
mode attempts to discover a single `shell_*.pem` credential in the Hoposhell
directory.

To use a different backend, configure **both** its HTTP API and TCP endpoint:

```sh
export HOPOSHELL_API=https://hoposhell.example.com
export HOPOSHELL_URL=hoposhell.example.com:10000
hopo setup shell_example
RECONNECT=true hopo connect shell_example
```

`RECONNECT=true` retries after an established connection is lost, keeping the
local PTY alive. Initial TCP connection failures are retried even when it is
false. It does not restart a client that exits or crashes; use an external
supervisor for that. A backend `restart` command exits the client with status
`0`, so a supervisor must restart after clean exits too.

## Commands

```text
hopo setup <shell-id>
hopo connect [shell-id]
hopo <shell-id>
hopo version
hopo populate
hopo command <target-shell-id> <command> <arguments...>
hopo forward-tcp <shell-id> <local-port> <remote-host> <remote-port>
```

### Structured remote commands

These requests go through the backend to a connected target client. They are
**not arbitrary shell command lines**; use the interactive terminal for those.

| Command | Arguments | Behavior |
| --- | --- | --- |
| `ls` | `<path-or-pattern>` | Return metadata for matching remote paths. Use `'/directory/*'` to match its entries. |
| `glob` | `<pattern>` | Return metadata for matching remote paths. |
| `download` / `cp` | `<remote-file> [local-file-or-directory]` | Download a file; default destination is the current directory. Use `-` for stdout. |
| `http` | `<get\|post> <url> [body]` | Make an HTTP request from the target machine and return its body. |
| `tcp` | `<host> <port> <payload>` | Send a text payload over a new TCP connection and return response bytes. |
| `scripts` | `<name>` | Execute a named executable in the target's `~/.hoposhell/scripts/` directory. No script arguments are supported. |

Examples:

```sh
hopo command shell_example ls '/var/log/*'
hopo command shell_example glob '/tmp/*.log' --json
hopo command shell_example download /tmp/report.txt ./report.txt
hopo command shell_example download /tmp/report.txt -
hopo command shell_example http get https://example.com --raw
hopo command shell_example scripts health-check
```

Quote remote patterns to prevent your local shell from expanding them.
`--json` and `--raw` select output modes where supported by the command;
they do not change the wire protocol. Script responses contain the script exit
code and stdout; stderr is discarded.

Outside an existing Hoposhell session, the default authentication identity for
`hopo command` is the target shell ID. To authenticate using another enrolled
shell owned by the same account:

```sh
HOPOSHELL_SHELL_ID=shell_source \
  hopo command shell_target download /tmp/report.txt ./report.txt
```

Here the client uses `shell_source.pem`; the backend checks ownership before
forwarding the request to `shell_target`. `HOPOSHELL_SHELL_KEY` can override
the credential path.

### TCP forwarding

```sh
hopo forward-tcp shell_example 8080 127.0.0.1 8000
```

Listens on local `localhost:8080`; the remote host is resolved/reached from the
target shell machine. This is a **limited request/response helper**, not a
general-purpose bidirectional tunnel: it handles connections sequentially,
reads one local chunk of up to 16 KiB, and makes one remote TCP request.
The remote TCP response has a one-second read timeout. Do not use it as an SSH
or arbitrary streaming TCP replacement.

### Optional terminal utility

`hopo populate` downloads iTerm2's `imgcat` utility into the Hoposhell `bin/`
directory, which is appended to the spawned shell's `PATH`. The download is
over HTTPS from a pinned upstream commit and must match the SHA-256 stored in
[`third-party/imgcat/CHECKSUM`](third-party/imgcat/CHECKSUM). Verification
happens before installation; a mismatch or download failure exits with an error
and leaves any existing installation untouched. Installation is atomic and
rejects symlink destinations. See [pin details](third-party/imgcat/README.md).

## Environment variables

Configuration is read from the process environment at startup; there is no
configuration file. Except for the positional commands and output-mode flags
above, configuration is not exposed as CLI flags.

In the table, `HOPOSHELL_HOME` means the computed path
`$HOME/$HOPOSHELL_HOME_NAME`; **it is not an environment variable**.

| Variable | Default | Meaning |
| --- | --- | --- |
| `HOPOSHELL_SHELL_ID` | Unset | Default/source shell identity. Explicit `connect`, `setup`, and `forward-tcp` shell IDs override it. |
| `HOPOSHELL_URL` | `api.hoposhell.com:10000` | Backend TCP endpoint, as `hostname:port`, not an HTTP URL. |
| `HOPOSHELL_API` | `https://api.hoposhell.com` | HTTP API base URL for setup. |
| `HOPOSHELL_HOME_NAME` | `.hoposhell` | Hoposhell directory relative to `$HOME`; an absolute value overrides the home prefix. |
| `HOPOSHELL_SERVER_CRT` | `HOPOSHELL_HOME/server.crt` | Trusted backend certificate file. |
| `HOPOSHELL_SHELL_KEY` | `HOPOSHELL_HOME/<shell-id>.pem` when an ID is selected | Client PEM certificate/private-key bundle. |
| `HOPOSHELL_WORKING_FOLDER` | Current directory | Initial directory of the spawned shell; shell/environment expansion is applied. |
| `HOPOSHELL_MAX_BUFFER_MB` | `64` | Positive whole MiB buffer limit; 1 MiB = 1,048,576 bytes. |
| `SHELL` | `bash` if unset | Executable to launch in the PTY; not a command line with arguments. |
| `COLS` | `80` | Initial terminal columns, as an unsigned 16-bit integer. |
| `ROWS` | `24` | Initial terminal rows, as an unsigned 16-bit integer. |
| `RECONNECT` | `false` | Reconnect after an established session disconnects. |
| `USE_SSL` | `true` | Enable TLS on the custom TCP connection. Keep enabled in production. |
| `VERIFY_CRT` | `true` | Verify the backend TLS certificate/hostname. Keep enabled in production. |
| `KEEP_ALIVE` | `5000` | Keepalive interval, in milliseconds. |
| `READ_TIMEOUT` | `0` | Connection-mode socket read timeout, in milliseconds; `0` selects nonblocking operation after TLS negotiation. |
| `READ_TIMEOUT_SLEEP` | `0` | Extra sleep after a would-block read, in milliseconds. |
| `COMMAND_TIMEOUT` | `60000` | Command-response inactivity timeout, in milliseconds; receipt of messages resets it. Blocking I/O can delay enforcement. |
| `VERBOSE` | Unset/off | Enables additional logging whenever present, even if its value is `0` or `false`. |
| `HOME` | Inherited; required | Base directory for the default Hoposhell directory. |
| `HOPOSHELL_CONNECTED` | Unset | Internal marker set in the spawned shell. Its presence prevents another `connect` from that environment; do not normally set it yourself. |

For `RECONNECT`, `USE_SSL`, and `VERIFY_CRT`, values `no`, `false`, and `0`
are false (case-insensitive); any other supplied value is true. Durations use
nonnegative whole milliseconds. Other malformed numeric settings may panic;
the buffer-limit setting explicitly rejects invalid values with exit status
`1`.

The client sets `HOPOSHELL_SHELL_ID` and `HOPOSHELL_CONNECTED=1` inside the
spawned shell, and appends `HOPOSHELL_HOME/bin` to its inherited `PATH`.

### Buffer-limit behavior

```sh
HOPOSHELL_MAX_BUFFER_MB=32 hopo connect shell_example
```

The limit covers individual growing buffers, aggregate pending command chunks,
aggregate response chunks, output history, and a shared budget for both
message queues. Exceeding it logs an error and exits the **whole client** with
status `1`, including when detected in a worker thread.

It is not a lifetime output counter. Consumed queue entries stop counting.
Terminal history forgets old output after roughly 2,048 chunks (up to about
32 MiB at full chunk size), but a smaller byte limit can terminate the client
before that rollover.

This is **not a hard total-memory limit**: buffers and copies can coexist,
libraries allocate additional memory, and spawned processes have their own
memory. Exiting does not guarantee descendant cleanup. Use OS-level
supervision for cleanup and, on Linux, a cgroup/systemd memory cap for stronger
containment. See [buffer-limit details](doc/buffer-limits.md).

## Backend protocol

The client uses two separate interfaces:

1. **HTTPS API** to enroll and download credentials.
2. **A custom framed TCP protocol over TLS** for terminal traffic and structured
   commands. This is not HTTP, WebSocket, or SSH on the client connection.
   The backend separately bridges terminal traffic to the web GUI.

### Enrollment and authentication

Setup calls:

```text
GET /shell-credentials/request/<shell-id>
GET /shell-credentials/confirmation/<shell-id>/<login-code>
```

The confirmation JSON contains `serverCrt` and `shellKey`. The latter bundles
the client certificate and private key. Keep these responses and the files
private; the login code is currently part of the request URL and can appear in
server/proxy access logs.

For TCP sessions, the client trusts the configured server certificate file
and presents its client certificate. The backend requests a certificate and
matches its raw certificate bytes against registered shell credentials; it
does not rely solely on conventional CA-based client-certificate validation.
The backend authenticates the source shell and enforces target ownership when
routing shell-to-shell commands.

TLS protects the client/backend link. The backend terminates TLS and can read
terminal traffic; this is not end-to-end encryption between shell clients or
between the shell and browser.

### Framing

Every nonempty frame is:

```text
BASE64(payload)-<tag>---\n
```

Base64 uses the standard alphabet. `---\n` is the frame delimiter, independent
of TCP packet boundaries. An empty `---\n` frame is a keepalive.

| Tag | Direction | Payload |
| --- | --- | --- |
| `hhh` | Client → backend | `v<package-version>` for a shell connection, or `v<package-version>/command` for a command-only connection. |
| `ooo` | Client → backend | Raw PTY output bytes. stdout/stderr are combined by the PTY. |
| `iii` | Backend → client | Raw terminal-input bytes to write to the PTY. |
| `ccc` | Either direction | Control message or structured request/response chunk. |

A connected shell sends its version header, current terminal size, and recent
output history. Command-only connections send the `/command` header and
requests without spawning a PTY.

### Control messages

These are uncompressed payloads inside `ccc` frames:

```text
restart             Backend → client: exit with status 0
resize/<rows>/<cols> Backend → client: resize the PTY
size/<rows>/<cols>   Client → backend: report current PTY size
```

### Structured commands and chunking

Before outer Base64 encoding, a structured chunk contains five slash-separated
metadata fields followed by a byte payload:

```text
<cmd>/req/<message-id>/shell:<target-id>/<chunk-type>/<payload>
<cmd>/res/<message-id>/<status>/<chunk-type>/<payload>
```

Only the first five `/` characters separate fields, so the final payload may
contain slashes or arbitrary binary bytes. Metadata fields must not contain
slashes. Chunk types are `not-last` and `last`; the Rust client emits payload
chunks of at most **8 KiB**. Receivers assemble chunks by message ID, in arrival order, until
`last`. Generated command IDs combine the source shell identity and an
eight-character random alphanumeric suffix.

The Rust client recognizes status codes `200` (success), `400` (incorrect
parameters), and `500` (internal error). Successful structured response bodies
are **zstd-compressed before chunking**; receivers concatenate the chunks and
then decompress. Error bodies are sent without that compression. Request
bodies and ordinary PTY traffic are not zstd-compressed.

| Command | Request body | Successful body before zstd compression |
| --- | --- | --- |
| `ls`, `glob` | UTF-8 path/pattern | JSON `{"entries":[...]}` with `name`, `fileType` (`file`/`dir`), `creationTimestamp`, `modificationTimestamp`, and `sizeInBytes`. |
| `download` | UTF-8 file path | Raw file bytes. |
| `http` | JSON `{"verb":"get","url":"...","headers":{},"body":null}` | Raw HTTP response body. Only GET/POST are implemented; supplied headers are currently unused. |
| `tcp` | JSON `{"host":"...","port":1234,"payload":[...]}` | Raw TCP response bytes. |
| `scripts` | JSON `{"name":"..."}` | Four-byte signed native-endian exit code followed by stdout. |

`cp` is a CLI alias; its wire command is `download`. The native-endian script
exit-code prefix is a compatibility constraint when implementing other clients.

### Implementation references

- Client configuration: [`src/args.rs`](src/args.rs).
- TLS/session handling and frame encoding: [`src/connect.rs`](src/connect.rs).
- Frame decoding: [`src/message.rs`](src/message.rs).
- Chunk encoding/decoding: [`src/commands/request_or_response.rs`](src/commands/request_or_response.rs).
- Backend enrollment: `src/api/shell-client-api.ts` in hoposhell-backend.
- Backend TLS authentication, terminal bridging, and routing:
  `src/shells/tls-socket-pty.ts` in hoposhell-backend.

## License

[MIT](LICENSE).
