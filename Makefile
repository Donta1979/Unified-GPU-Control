export CARGO_TARGET_DIR ?= ./target
export CARGO_NET_GIT_FETCH_WITH_CLI ?= true
DESTDIR ?= /
PREFIX ?= /usr/local

.PHONY: build-release
build-release:
	cargo build -p lact --release

.PHONY: build-debug
build-debug:
	cargo build -p lact

.PHONY: build-release-headless
build-release-headless:
	cargo build -p lact --release --no-default-features --features=nvidia

.PHONY: install-resources
install-resources:
	install -Dm644 res/io.github.Donta1979.UnifiedGPUControl.desktop $(DESTDIR)$(PREFIX)/share/applications/io.github.Donta1979.UnifiedGPUControl.desktop
	install -Dm644 res/io.github.Donta1979.UnifiedGPUControl.png $(DESTDIR)$(PREFIX)/share/icons/hicolor/512x512/apps/io.github.Donta1979.UnifiedGPUControl.png

.PHONY: install
install: install-resources
	install -Dm755 target/release/lact $(DESTDIR)$(PREFIX)/bin/ugc

.PHONY: install-debug
install-debug: install-resources
	install -Dm755 target/debug/lact $(DESTDIR)$(PREFIX)/bin/ugc

.PHONY: uninstall
uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/ugc
	rm -f $(DESTDIR)$(PREFIX)/share/applications/io.github.Donta1979.UnifiedGPUControl.desktop
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/512x512/apps/io.github.Donta1979.UnifiedGPUControl.png
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/io.github.Donta1979.UnifiedGPUControl.svg

.PHONY: update-vulkan-schema
update-vulkan-schema:
	curl -o lact-daemon/vulkan_schema.json https://schema.khronos.org/vulkan/profiles-0.8-latest.json

.PHONY: update-nvidia-headers
update-nvidia-headers:
	cd lact-daemon/include/nvidia && ./update_kernel_files.sh
