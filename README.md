# pqlens

Trying to make a simple TUI tool similar to [csvlens](https://github.com/YS-L/csvlens), but for parquet files.

## This project is not:

- Vibe coded or using AI.
- Blazingly fast with an ⚡ icon/badge showing off (no benchmarking, no performance claims on how many ms it takes to read a 1GB parquet file).
- Trying to sell it as a good project just because it is written in Rust (language is not that important).
- Aiming to be a full-featured parquet file inspector (just a simple TUI tool).

## Installation

### From GitHub

Install the CLI directly from GitHub with Cargo:

```bash
cargo install --git https://github.com/PMassicotte/pqlens
```

This builds the `pqlens` binary and installs it in `~/.cargo/bin`. Make sure that directory is in your `PATH` (rustup usually sets this up for you):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## Contributing

Human contributions are welcome. Please feel free to submit a pull request or an issue if you have any suggestions or improvements.
