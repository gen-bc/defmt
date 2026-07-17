# Running on the host (Linux / macOS)

`defmt` primarily targets embedded devices, but programs that use it — including your
crate's **unit tests** — can also be compiled for and run on a host operating system.
Both Linux (ELF) and macOS (Mach-O) executables are supported.

Host builds do **not** use the `defmt.x` linker script, so no `.cargo/config.toml`
changes are needed.

## Setup

Add [`defmt-stdout`] as the global logger and link to it somewhere in your project:

[`defmt-stdout`]: https://github.com/knurling-rs/defmt/tree/main/stdout

```toml
# Cargo.toml
[dependencies]
defmt = "1"
defmt-stdout = "0.1"
```

``` rust,ignore
// src/main.rs
use defmt_stdout as _;

fn main() {
    defmt::println!("Hello, x = {=u32}", 42);
}
```

`defmt-stdout` writes the binary defmt wire data to stdout, or to a file if the
`DEFMT_STDOUT_FILE` environment variable is set. Pipe the data through [`defmt-print`]
to view the logs:

[`defmt-print`]: https://crates.io/crates/defmt-print

```console
$ cargo run | defmt-print -e target/debug/my-app
Hello, x = 42
```

> 💡 Remember that log levels are selected at *compile time* with the `DEFMT_LOG`
> environment variable (see [Filtering](./filtering.md)). By default only ERROR level
> statements are compiled in: `DEFMT_LOG=info cargo run | defmt-print -e ...`.

## Running unit tests

Unit tests work the same way, with one caveat: `cargo test`'s own output ("running 1
test ...") goes to stdout too and would corrupt the binary defmt stream. Use
`DEFMT_STDOUT_FILE` to send the defmt data to a file instead:

```console
$ DEFMT_LOG=info DEFMT_STDOUT_FILE=defmt.bin cargo test
   Compiling my-app v0.1.0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.5s
     Running unittests src/main.rs (target/debug/deps/my_app-8759a844630659af)

running 1 test
test tests::logging_from_a_unit_test ... ok
```

Then decode the file, passing the *test binary* (the path is printed by `cargo test` on
the `Running` line) to `defmt-print`:

```console
$ defmt-print -e target/debug/deps/my_app-8759a844630659af < defmt.bin
INFO  hello from a unit test: 42
```

## Source location information on macOS (the dSYM trick)

`defmt-print` reads DWARF debug info to display file/line/module information (shown with
`--verbose`). On Linux the DWARF is embedded in the executable itself, so this works out
of the box.

On macOS, however, DWARF stays in the object files and is only collected into a separate
`.dSYM` bundle by the `dsymutil` tool. `defmt-print` looks for that bundle next to the
executable (e.g. `target/debug/my-app.dSYM`). To make Cargo generate it, set
[`split-debuginfo`] to `"packed"` in your profile:

```toml
# Cargo.toml
[profile.dev]
split-debuginfo = "packed"
```

Alternatively, run `dsymutil target/debug/my-app` by hand after building. This also
applies to test binaries: with `split-debuginfo = "packed"` Cargo places a `.dSYM`
bundle next to each test binary in `target/debug/deps/`.

> ⚠️ Only do this for macOS builds. On Linux, `split-debuginfo = "packed"` *removes* the
> DWARF from the executable (moving it into separate files that `defmt-print` doesn't
> read), so location information would be lost. If you build for both, scope it with
> `[profile.dev]` in a macOS-specific config or leave the Linux default (`"off"`) alone.

[`split-debuginfo`]: https://doc.rust-lang.org/cargo/reference/profiles.html#split-debuginfo

## Timestamps and panics

On embedded targets the `defmt.x` linker script provides fallback implementations of the
`_defmt_timestamp` and `_defmt_panic` symbols. Host builds don't use the linker script,
so `defmt-stdout` provides those fallbacks instead, behind two default features:

- `timestamp`: an empty timestamp. Disable this feature if you want to define your own
  with `defmt::timestamp!`.
- `panic-handler`: forwards `defmt::panic!` and friends to `core::panic!` (the panic
  message is logged through defmt first). Disable this feature if you want to define
  your own with `#[defmt::panic_handler]`.
