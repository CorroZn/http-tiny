# http-tiny

A tiny HTTP server intended for use in constrained systems.

**http-tiny** serves static files from a configurable document root. It is designed for embedded systems, containers, and other environments with limited memory, processing power, or storage capacity.

The project aims to provide essential HTTP functionality with minimal resource requirements and a small operational footprint.

Free and open-source software licensed under GPL-2.0.

> \[\!WARNING\] **Development Status: Early Development**http-tiny is actively being developed and is not yet considered production-ready. Its functionality, security, and stability may change between releases.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Project Goals](#project-goals)
- [Features](#features)
- [Installation](#installation)
- [Building from Source](#building-from-source)
  - [Requirements](#requirements)
  - [Build Dependencies](#build-dependencies)
  - [Linux x86-64](#linux-x86-64)
  - [Linux x86-32](#linux-x86-32)
  - [Linux ARM64](#linux-arm64)
  - [Linux ARM32](#linux-arm32)
  - [Cross-compilation Requirements](#cross-compilation-requirements)
- [Usage](#usage)
  - [Command-Line Options](#command-line-options)
  - [Environment Variables](#environment-variables)
  - [Document Root Structure](#document-root-structure)
  - [Network Access](#network-access)
- [Server Limits and Timeouts](#server-limits-and-timeouts)
- [Request Handling](#request-handling)
- [File Access and Security](#file-access-and-security)
- [Supported Platforms](#supported-platforms)
- [Contributing](#contributing)
- [License](#license)

---

## Quick Start

The following steps demonstrate how to download, configure, and start a pre-compiled version of http-tiny.

### 1\. Download

Download the appropriate Linux binary for your system architecture:

**Download the latest Linux release**

Available architectures:

| Architecture | Description |
| --- | --- |
| `x86_64` | Linux x86-64 |
| `i686` | Linux x86 (32-bit) |
| `aarch64` | Linux ARM64 |
| `armv7` | Linux ARM (32-bit) |

### 2\. Prepare the Executable

Make the downloaded binary executable:

```bash
chmod +x http-tiny
```

### 3\. Start the Server

Start the server using its default configuration:

```bash
./http-tiny
```

Default configuration:

| Setting | Default |
| --- | --- |
| Document root | `www` |
| Listening address | `127.0.0.1` |
| Listening port | `8080` |

The document root is created automatically if it does not exist.

### 4\. Verify the Server

Once started, the server accepts local HTTP connections at:

```text
http://127.0.0.1:8080
```

### 5\. Custom Configuration

Configuration can be supplied through command-line arguments:

```bash
./http-tiny -d /srv/www -i 127.0.0.1 -p 8080
```

To make the server accessible through all network interfaces:

```bash
./http-tiny -d /srv/www -i 0.0.0.0 -p 8080
```

See the Usage section for all available configuration options.

> \[\!CAUTION\] Binding to `0.0.0.0` exposes the server on all network interfaces. Any device that can reach the server may retrieve accessible files beneath the document root. Only use this configuration when network access is intended.

---

## Project Goals

The goal of http-tiny is to provide a standalone HTTP server with a significantly smaller resource footprint than traditional HTTP servers.

The project prioritizes:

- Minimal resource consumption.
- A small and self-contained executable.
- Straightforward configuration.
- Predictable resource usage.
- Secure file access within the configured document root.
- Compatibility with resource-constrained Linux systems.

## Features

- Lightweight, standalone HTTP server.
- Static file serving over TCP using HTTP/1.1.
- HTTP `GET` request support.
- Configurable document root.
- Command-line and environment variable configuration.
- Basic HTTP request validation.
- Path sanitization and file-access restrictions.
- MIME type detection for common file formats.
- Bounded concurrent client handling.
- Request and response timeouts.
- Unix directory-relative file operations for file confinement.

## Project Status

**http-tiny is currently in early development.**

The server is functional but has not yet been declared production-ready.

Features, interfaces, and implementation details may change as development progresses.

Bug reports, feature requests, and contributions are welcome.

---

## Installation

There are two ways to install http-tiny.

### Option 1: Download a pre-compiled Release

Download the latest available Linux binaries:

**Download Linux Releases**

Select the binary matching your target architecture.

### Option 2: Build from Source

Follow the instructions in Building from Source.

---

## Building from Source

### Requirements

The following tools are required to compile http-tiny:

- Git
- Rust
- Cargo
- musl C library

These dependencies are only required during compilation.

The resulting executable is statically linked against musl and does not require additional runtime library dependencies.

### Build Dependencies

Install the required packages for your Linux distribution.

#### Arch Linux

```bash
sudo pacman -S git rustup base-devel musl
```

#### Debian

```bash
sudo apt install git rustup build-essential musl musl-dev musl-tools
```

#### Fedora

```bash
sudo dnf install git rustup gcc make musl-gcc musl-devel
```

### Clone the Repository

Clone the repository and enter the project directory:

```bash
git clone https://github.com/CorroZn/http-tiny.git
cd http-tiny
```

### Linux x86-64

Target: `x86_64-unknown-linux-musl`

```bash
rustup default stable
rustup target add x86_64-unknown-linux-musl

cargo build --release --target x86_64-unknown-linux-musl
```

Executable:

```text
target/x86_64-unknown-linux-musl/release/http-tiny
```

### Linux x86-32

Target: `i686-unknown-linux-musl`

```bash
rustup default stable
rustup target add i686-unknown-linux-musl

cargo build --release --target i686-unknown-linux-musl
```

Executable:

```text
target/i686-unknown-linux-musl/release/http-tiny
```

### Linux ARM64

Target: `aarch64-unknown-linux-musl`


```bash
rustup default stable
rustup target add aarch64-unknown-linux-musl

cargo build --release --target aarch64-unknown-linux-musl
```

Executable:

```text
target/aarch64-unknown-linux-musl/release/http-tiny
```

### Linux ARM32

Target: `armv7-unknown-linux-musleabihf`

```bash
rustup default stable
rustup target add armv7-unknown-linux-musleabihf

cargo build --release --target armv7-unknown-linux-musleabihf
```

Executable:

```text
target/armv7-unknown-linux-musleabihf/release/http-tiny
```

### Cross-compilation Requirements

When compiling for a different architecture, [cross](<https://github.com/cross-rs/cross>) can be used instead of Cargo.

`cross` relies on Docker or another supported container runtime to provide the target compilation environment.

Requirements:

- Docker or a compatible supported container runtime.
- An operational container runtime service.
- Permission to access the container runtime.

Install `cross`:

```bash
cargo install cross
```

Build for the desired target:


```bash
cross build --release --target aarch64-unknown-linux-musl
```

Replace the target with the appropriate architecture.

For additional information, refer to the [cross documentation](<https://github.com/cross-rs/cross>).

---

## Usage

Start the server using:

```bash
./http-tiny -d /srv/www -i 127.0.0.1 -p 1234
```

This starts the server with `/srv/www` as its document root, listening on `127.0.0.1` on port `1234`.

### Command-Line Options

| Option | Description | Default |
| --- | --- | --- |
| `-d` | Website directory (document root). Created automatically if missing. | `www` |
| `-i` | IP address the server listens on. | `127.0.0.1` |
| `-p` | Port the server listens on. | `8080` |
| `-h`, `--help` | Display available command-line options. | — |
| `-V`, `--version` | Display the server version. | — |

### Environment Variables

Configuration can also be provided through environment variables.

| Variable | Description |
| --- | --- |
| `DOC_ROOT` | Website directory (document root). |
| `LISTEN_ADDR` | IP address the server listens on. |
| `LISTEN_PORT` | Port the server listens on. |

#### Configuration Priority

Configuration is resolved independently for each setting:

1. Command-line options have the highest priority.
2. Environment variables are used when no corresponding command-line option is provided.
3. Built-in defaults are used when neither is provided.

This allows command-line arguments and environment variables to be combined.

#### Example: Environment Variable Configuration

```bash
DOC_ROOT=/srv/www LISTEN_ADDR=127.0.0.1 LISTEN_PORT=1234 ./http-tiny
```

#### Example: Mixed Configuration

```bash
DOC_ROOT=/srv/env-www LISTEN_ADDR=0.0.0.0 LISTEN_PORT=8081 \
./http-tiny -d /srv/cli-www
```

In this example, the server uses:

| Setting | Source | Value |
| --- | --- | --- |
| Document root | Command line | `/srv/cli-www` |
| Listening address | Environment | `0.0.0.0` |
| Listening port | Environment | `8081` |

Invalid environment variable values are reported during startup unless overridden by the corresponding command-line option.

For example, `LISTEN_PORT` must be an integer between `1` and `65535`.

### Document Root Structure

The document root defines the directory from which static files are served.

Example directory structure:


```text
/srv/www/
├── index.html
├── favicon.ico
├── css/
│   └── style.css
├── js/
│   └── script.js
└── images/
    └── logo.png
```

Files are accessible through their corresponding relative HTTP paths:

| File | Request path |
| --- | --- |
| `index.html` | `/` |
| `about.html` | `/about.html` |
| `css/style.css` | `/css/style.css` |
| `js/script.js` | `/js/script.js` |
| `images/logo.png` | `/images/logo.png` |

Specify a different document root using the `-d` option or the `DOC_ROOT` environment variable.

```bash
./http-tiny -d /srv/www
```

The configured document-root directory is securely reopened for each response. Replacing the directory at that path therefore takes effect on subsequent requests.

### Network Access

By default, http-tiny only accepts connections from the local machine:

```text
127.0.0.1
```

To make the server accessible through all network interfaces, configure the listening address as `0.0.0.0`.

Using a command-line option:

```bash
./http-tiny -i 0.0.0.0
```

Or using an environment variable:

```bash
LISTEN_ADDR=0.0.0.0 ./http-tiny
```

When listening on `0.0.0.0`, any device that can reach the server may access its published files.

---

## Server Limits and Timeouts

http-tiny applies fixed resource limits and timeouts to bound connection handling and prevent clients from holding resources indefinitely.

| Limit | Value |
| --- | --- |
| Maximum active clients | 64 |
| Maximum request header size | 8 KiB |
| Request header idle timeout | 5 seconds |
| Total request header deadline | 30 seconds |
| File metadata read timeout | 30 seconds |
| Individual response write timeout | 30 seconds |
| Complete response deadline | 5 minutes |

These limits help maintain predictable resource usage and reduce exposure to slow-client behavior.

---

## Request Handling

http-tiny implements a deliberately limited subset of HTTP/1.1.

### Supported Requests

The server currently supports:

- HTTP/1.1 `GET` requests.
- Requests without a request body.
- Exactly one non-empty `Host` header per request.

Other methods and unsupported request formats are rejected.

### HTTP Error Responses

The server returns the following status codes when requests cannot be processed:

| Status Code | Description |
| --- | --- |
| `400 Bad Request` | Malformed request. |
| `404 Not Found` | Requested file is unavailable or access is disallowed. |
| `405 Method Not Allowed` | Unsupported HTTP method. Includes `Allow: GET`. |
| `431 Request Header Fields Too Large` | Request headers exceed the configured size limit. |
| `505 HTTP Version Not Supported` | Unsupported HTTP version. |

### URL Query and Fragment Handling

Literal `?` and `#` characters mark the beginning of query and fragment suffixes, respectively.

These suffixes are ignored when resolving the requested filename.

Percent-encoded characters such as `%3F` and `%23` remain part of the filename.

---

## File Access and Security

http-tiny restricts file access to the configured document root using Unix directory-relative file operations.

### File Access Restrictions

- Only non-hidden regular files beneath the configured document root are served.
- Symlinks resolving to non-hidden files inside the document root are supported.
- Symlinks escaping the document root are rejected.
- Symlinks resolving into hidden paths are rejected.
- The document root is securely reopened for each response.

The document root is reopened for every response, so replacing the directory at its configured path takes effect on subsequent requests.

### Operational Security

When running http-tiny as an ordinary user, ensure that the selected document root is readable and searchable by that user.

Use an unprivileged port, such as the default `8080`. Binding to ports below `1024` may require elevated privileges, depending on the operating system.

When configured with `0.0.0.0`, the server listens on all network interfaces. Any device that can reach the server may retrieve accessible files beneath the document root.

**Treat the document root as trusted, publicly accessible content.**

MIME type detection identifies file formats but does not sanitize file contents.

Do not store untrusted uploads in the document root. If user-uploaded content needs to be served, use a separate origin or serve the files as downloads.

---

## Supported Platforms

The following Linux architectures are supported:

| Architecture | Rust Target |
| --- | --- |
| x86 (32-bit) | `i686-unknown-linux-musl` |
| x86-64 (64-bit) | `x86_64-unknown-linux-musl` |
| ARM (32-bit) | `armv7-unknown-linux-musleabihf` |
| ARM64 (64-bit) | `aarch64-unknown-linux-musl` |

### Platform Limitations

The file-confinement implementation relies on Unix directory-relative file operations.

On non-Unix targets, the server exits during startup rather than falling back to less secure path-based file operations.

---

## Contributing

Contributions are welcome.

Bug reports, feature requests, and pull requests help improve the project.

If you encounter an issue or have a suggestion:

- Open an issue to report bugs or discuss proposed changes.
- Submit a pull request with improvements or fixes.

Constructive feedback and contributions are appreciated.

---

## License

http-tiny is free and open-source software licensed under the **GNU General Public License v2.0 (GPL-2.0)**.

You are free to use, modify, and redistribute this software under the terms of the license.

See the [LICENSE file](https://github.com/CorroZn/http-tiny/blob/main/LICENSE) for the full license text.
