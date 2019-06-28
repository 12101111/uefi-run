# uefi-run [![Latest Version]][crates.io] [![Build Status]][travis]

[Build Status]: https://travis-ci.org/Richard-W/uefi-run.svg?branch=master
[travis]: https://travis-ci.org/Richard-W/uefi-run
[Latest Version]: https://img.shields.io/crates/v/uefi-run.svg
[crates.io]: https://crates.io/crates/uefi-run

Run UEFI applications in qemu

## Installation

```shell
cargo install uefi-run
```

## Usage

### Run directly

```shell
# run UEFI application directly
uefi-run [UEFI_FILE]
# run UEFI application in headless qemu
uefi-run [UEFI_FILE] -- -nographic
```

### Using in rust project

First you should install `cargo-xbuild`.

You can set `uefi-run` as a custom runner in `.cargo/config`:

```toml
[build]
target = "x86_64-unknown-uefi"

[target.x86_64-unknown-uefi]
runner = "uefi-run"
```

Then you can run your rust UEFI application through `cargo xrun`.
