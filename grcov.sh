# cargo binstall ripgrep
rg --files -g '*.profraw' | xargs rm
# html report is not correctly updated without:
rm -rf target/debug/coverage/
# https://github.com/mozilla/grcov?tab=readme-ov-file#usage
export RUSTFLAGS="-Cinstrument-coverage"
cargo build --features xflags
export LLVM_PROFILE_FILE="rustyline-%p-%m.profraw"
cargo test --features xflags
# cargo binstall grcov
grcov . -s . --binary-path ./target/debug/ -t html --ignore-not-existing -o ./target/debug/coverage/
rg --files -g '*.profraw' | xargs rm
open target/debug/coverage/index.html
