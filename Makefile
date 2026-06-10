.PHONY: publish publish-codegen publish-macro publish-gourd clean coverage cov-summary coverage-json coverage-terse

# ── Coverage targets ───────────────────────────────────────────────
cov-summary:			# Print coverage summary from JSON report
	@mkdir -p target/llvm-cov
	@cargo llvm-cov --workspace --json > target/llvm-cov/coverage.json
	@python3 scripts/coverage-summary.py target/llvm-cov/coverage.json

coverage:			# Run tests with coverage, open HTML report in browser
	@$(MAKE) cov-summary
	cargo llvm-cov --workspace --html --open

coverage-json:			# Generate JSON coverage report (machine-readable)
	@mkdir -p target/llvm-cov
	cargo llvm-cov --workspace --json > target/llvm-cov/coverage.json

coverage-terse:			# Generate LCOV report (for CI/integrations)
	@mkdir -p target/llvm-cov
	cargo llvm-cov --workspace --lcov > target/llvm-cov/lcov.info

# ── Publish targets ───────────────────────────────────────────────
publish: publish-codegen publish-macro publish-gourd

publish-codegen:
	@echo "Publishing gourd-codegen..."
	cargo publish -p gourd-codegen --allow-dirty

publish-macro:
	@echo "Publishing gourd-macro..."
	sed -i '' 's|gourd-codegen = { path = "../gourd-codegen" }|gourd-codegen = "0.1"|g' gourd-macro/Cargo.toml
	cargo publish -p gourd-macro --allow-dirty
	sed -i '' 's|gourd-codegen = "0.1"|gourd-codegen = { path = "../gourd-codegen" }|g' gourd-macro/Cargo.toml

publish-gourd:
	@echo "Publishing gourd..."
	sed -i '' 's|gourd-macro = { path = "../gourd-macro" }|gourd-macro = "0.1"|g; s|gourd-codegen = { path = "../gourd-codegen" }|gourd-codegen = "0.1"|g' gourd/Cargo.toml
	cargo publish -p gourd --allow-dirty
	sed -i '' 's|gourd-macro = "0.1"|gourd-macro = { path = "../gourd-macro" }|g; s|gourd-codegen = { path = "../gourd-codegen" }|g' gourd/Cargo.toml

clean:
	rm -rf target/
