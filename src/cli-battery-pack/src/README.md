# cli-battery-pack

A [battery pack](https://crates.io/crates/battery-pack) containing crates commonly
needed when building command-line applications in Rust.

## Quick start

```bash
cargo bp add cli
```

Then in your code:

```rust,ignore
use cli::{clap::Parser, anyhow::Result};

#[derive(Parser)]
struct Args {
    name: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("Hello, {}!", args.name);
    Ok(())
}
```
