check-fmt:
	cargo +nightly fmt --all -- --check

check-clippy:
	cargo clippy --all-targets --all-features --workspace -- -D warnings

install-cargo-sort:
	cargo install cargo-sort@1.0.9

cargo-sort: install-cargo-sort
	cargo sort -c -w

install-cargo-machete:
	cargo install cargo-machete

cargo-machete: install-cargo-machete
	cargo machete

check: check-fmt check-clippy cargo-sort cargo-machete
