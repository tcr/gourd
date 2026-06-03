#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# Step 1: publish gourd-codegen (no deps)
cargo publish -p gourd-codegen --allow-dirty

# Step 2: publish gourd-macro (swap to version, publish, swap back)
sed -i '' 's|gourd-codegen = { path = "../gourd-codegen" }|gourd-codegen = "0.1"|g' gourd-macro/Cargo.toml
cargo publish -p gourd-macro --allow-dirty
sed -i '' 's|gourd-codegen = "0.1"|gourd-codegen = { path = "../gourd-codegen" }|g' gourd-macro/Cargo.toml

# Step 3: publish gourd (swap to version, publish, swap back)
sed -i '' 's|gourd-macro = { path = "../gourd-macro" }|gourd-macro = "0.1"|g; s|gourd-codegen = { path = "../gourd-codegen" }|gourd-codegen = "0.1"|g' gourd/Cargo.toml
cargo publish -p gourd --allow-dirty
sed -i '' 's|gourd-macro = "0.1"|gourd-macro = { path = "../gourd-macro" }|g; s|gourd-codegen = "0.1"|gourd-codegen = { path = "../gourd-codegen" }|g' gourd/Cargo.toml

echo "All crates published."
