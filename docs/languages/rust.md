# Rust

```bash
cargo add zindi
cargo add tokio --features full
```

```rust
use zindi::{Client, CompetitionQuery};

#[tokio::main]
async fn main() -> zindi::Result<()> {
    let client = Client::new()?;
    for comp in client.competitions(CompetitionQuery::default()).await? {
        println!("{}  {}", comp.id, comp.title);
    }
    Ok(())
}
```

API documentation: [docs.rs/zindi](https://docs.rs/zindi). More in the
[crate README](https://github.com/eaedk/zindi-package/tree/main/rust#readme).
