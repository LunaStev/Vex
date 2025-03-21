VERSION := $(shell grep '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

TARGET_DIR := ./target
LINUX_TARGET := x86_64-unknown-linux-gnu
WINDOWS_TARGET := x86_64-pc-windows-gnu
DARWIN_TARGETS := x86_64-apple-darwin aarch64-apple-darwin

build-windows:
	cargo build --target $(WINDOWS_TARGET) --release

build-linux:
	cargo build --target $(LINUX_TARGET) --release

package-linux:
	tar -czvf vex-v$(VERSION)-linux.tar.gz -C $(TARGET_DIR)/$(LINUX_TARGET)/release vex

package-windows:
	zip vex-v$(VERSION)-windows.zip $(TARGET_DIR)/$(WINDOWS_TARGET)/release/wave.exe

build-all: build-linux # build-windows

package-all: package-linux # package-windows

release: build-all package-all

build-darwin:
	$(foreach target, $(DARWIN_TARGETS), cargo build --target $(target) --release;)

package-darwin:
	$(foreach target, $(DARWIN_TARGETS), tar -czvf wave-$(VERSION)-$(target).tar.gz -C $(TARGET_DIR)/$(target)/release wave)

clean:
	rm -rf $(TARGET_DIR)