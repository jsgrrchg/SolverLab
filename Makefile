.PHONY: build-rust generate-bindings xcframework clean check test

# Configuration
MAKEFILE_DIR := $(CURDIR)/
CRATE_DIR = $(MAKEFILE_DIR)solver-core-rs
LIB_NAME = solver_core
OUT_DIR = $(MAKEFILE_DIR)Sources/SolverCoreRS
FRAMEWORK_NAME = SolverCoreRS
CARGO_TARGET_DIR ?= $(CRATE_DIR)/target

# Default target
all: xcframework

# Type-check without building (fast feedback)
check:
	cd "$(CRATE_DIR)" && cargo check

# Run Rust tests
test:
	cd "$(CRATE_DIR)" && cargo test

# Build for both architectures
build-rust:
	cd "$(CRATE_DIR)" && cargo build --release --target aarch64-apple-darwin
	cd "$(CRATE_DIR)" && cargo build --release --target x86_64-apple-darwin

# Generate Swift bindings from UDL
generate-bindings:
	cd "$(CRATE_DIR)" && cargo run --bin uniffi-bindgen generate \
		src/solver_core.udl --language swift --out-dir "$(OUT_DIR)"

# Create universal binary + XCFramework
xcframework: build-rust generate-bindings
	@mkdir -p "$(OUT_DIR)"
	lipo -create \
			"$(CARGO_TARGET_DIR)/aarch64-apple-darwin/release/lib$(LIB_NAME).a" \
			"$(CARGO_TARGET_DIR)/x86_64-apple-darwin/release/lib$(LIB_NAME).a" \
			-output "$(MAKEFILE_DIR)lib$(LIB_NAME).a"
	@mkdir -p "$(MAKEFILE_DIR)include"
	@cp "$(OUT_DIR)/solver_coreFFI.h" "$(MAKEFILE_DIR)include/SolverCoreRS.h" || true
	@cp "$(OUT_DIR)/solver_coreFFI.modulemap" "$(MAKEFILE_DIR)include/module.modulemap" || true
	@sed -i '' 's/solver_coreFFI/SolverCoreRS/g' "$(MAKEFILE_DIR)include/module.modulemap" || true
	@sed -i '' 's/solver_coreFFI/SolverCoreRS/g' "$(OUT_DIR)/solver_core.swift" || true
	@sed -i '' 's/private var initializationResult/private let initializationResult/g' "$(OUT_DIR)/solver_core.swift" || true
	@rm -f "$(OUT_DIR)/solver_coreFFI.h" "$(OUT_DIR)/solver_coreFFI.modulemap"
	@rm -rf "$(MAKEFILE_DIR)$(FRAMEWORK_NAME).xcframework"
	DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild -create-xcframework \
		-library "$(MAKEFILE_DIR)lib$(LIB_NAME).a" \
		-headers "$(MAKEFILE_DIR)include/" \
		-output "$(MAKEFILE_DIR)$(FRAMEWORK_NAME).xcframework"
	@echo "✅ $(FRAMEWORK_NAME).xcframework created"

clean:
	cd "$(CRATE_DIR)" && cargo clean
	rm -rf "$(MAKEFILE_DIR)$(FRAMEWORK_NAME).xcframework" "$(MAKEFILE_DIR)lib$(LIB_NAME).a" "$(MAKEFILE_DIR)include/"
	rm -rf "$(OUT_DIR)"
