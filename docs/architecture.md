Vor dem Veröffentlichen prüfen:

cargo fmt
cargo clippy
cargo test
cargo publish --dry-run



Veröffentlichen:


cargo login
cargo publish



Wichtig: Der name = "nextframe" muss auf crates.io noch frei sein. Wenn nicht, brauchst du einen anderen Namen, z. B. nextframe-rs.


cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt