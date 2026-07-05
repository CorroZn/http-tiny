# http-tiny

A tiny HTTP server intented for use in constrained systems.

It serves static files from a configurable document root. It is intended for use in embedded systems, containers, and other installations with limited ressources.

Free for everyone to use and modify.

---

## Project Goal

Providing a standalone HTTP server that is much lighter on system ressources than traditional HTTP servers so it can be used on small systems.

---

## Current Features

- Lightweight standalone HTTP server
- Static file serving over TCP using HTTP/1.1
- GET request support
- Configurable document root
- Basic request validation
- Path sanitization
- MIME type detection for common file formats
- Simple command-line configuration

---

## Project Status

This project is currently in early development.

It is not yet compliant for production deployments.


Improvements and suggestions are always welcome.

---

## Installation

There are two ways to install **http-tiny**.

### Option 1: Download a precompiled release

[Download the latest Linux builds](https://github.com/CorroZn/http-tiny/releases/tag/linux-stable)

### Option 2: Build from source

See the instructions below.

---

## Building from Source

### Requirements

To build **http-tiny** yourself, you will need:

- Git
- Rust
- Cargo
- musl C library

These tools are only required for compiling the source code. The compiled executable is statically linked and will run with no runtime library dependencies.

### Install build dependencies

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

---

### Compiling (Linux x86 64-bit)

```bash
git clone https://github.com/CorroZn/http-tiny.git

cd http-tiny

rustup default stable
rustup target add x86_64-unknown-linux-musl

cargo build --release --target x86_64-unknown-linux-musl

cd target/x86_64-unknown-linux-musl/release

chmod +x http-tiny
```

The compiled executable can be found at:

```text
target/x86_64-unknown-linux-musl/release/http-tiny
```

---


### Compiling (Linux x86 32-bit)

```bash
git clone https://github.com/CorroZn/http-tiny.git

cd http-tiny

rustup default stable
rustup target add i686-unknown-linux-musl

cargo build --release --target i686-unknown-linux-musl

cd target/i686-unknown-linux-musl/release

chmod +x http-tiny
```

The compiled executable can be found at:

```text
target/i686-unknown-linux-musl/release/http-tiny
```

---


### Compiling (Linux ARM 64-bit)

```bash
git clone https://github.com/CorroZn/http-tiny.git

cd http-tiny

rustup default stable
rustup target add aarch64-unknown-linux-musl

cargo build --release --target aarch64-unknown-linux-musl

cd target/aarch64-unknown-linux-musl/release

chmod +x http-tiny
```


If you are compiling on a non-ARM system you will need to use ```cross``` instead of ```cargo```:

```bash
cargo install cross

cross build --release --target aarch64-unknown-linux-musl

cd target/aarch64-unknown-linux-musl/release

chmod +x http-tiny
```

The compiled executable can be found at:

```text
target/aarch64-unknown-linux-musl/release/http-tiny
```

---


### Compiling (Linux ARM 32-bit)

```bash
git clone https://github.com/CorroZn/http-tiny.git

cd http-tiny

rustup default stable
rustup target add armv7-unknown-linux-musleabihf

cargo build --release --target armv7-unknown-linux-musleabihf

cd target/armv7-unknown-linux-musleabihf/release

chmod +x http-tiny
```


If you are compiling on a non-ARM system you will need to use ```cross``` instead of ```cargo```:

```bash
cargo install cross

cross build --release --target armv7-unknown-linux-musleabihf

cd target/armv7-unknown-linux-musleabihf/release

chmod +x http-tiny
```

The compiled executable can be found at:

```text
target/armv7-unknown-linux-musleabihf/release/http-tiny
```

---

## Usage


Run the server:


```bash
./http-tiny -d /srv/www -i 127.0.0.1 -p 1234
```


-d = Document root (Default: /var/www)


-i = IP address to bind to (Default: 0.0.0.0)


-p = TCP port to listen on (Default: 8080)


-h or --help = Print help


-V or --version = Print version


Running without any options specified will make http-tiny run with default values.

---

## Supported Platforms

- Linux x86 (32-bit and 64-bit)
- Linux ARM (32-bit and 64-bit)

---

## Contributing

Bug reports, feature requests, and pull requests are welcome.

If you find a bug or have an idea for an improvement, please open an issue or submit a pull request.

---

## License

This project is licensed under the **GNU General Public License v2.0 (GPL-2.0)**.

It is free and open-source software. You are welcome to use, modify, and redistribute it under the terms of the GPL-2.0 license.

See the [LICENSE file](https://github.com/CorroZn/http-tiny/blob/main/LICENSE) for the full license text.