PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo

.PHONY: build check install uninstall dist
build:
	$(CARGO) build --release --locked

check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --locked -- -D warnings
	$(CARGO) test --locked

install:
	test -f target/release/game-super-runtime
	install -Dm755 target/release/game-super-runtime "$(DESTDIR)$(PREFIX)/bin/game-super-runtime"
	install -Dm644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/game-super-runtime/LICENSE"
	install -Dm644 packaging/game-super-runtime.1 "$(DESTDIR)$(PREFIX)/share/man/man1/game-super-runtime.1"
	install -Dm644 packaging/game-super-runtime.bash "$(DESTDIR)$(PREFIX)/share/bash-completion/completions/game-super-runtime"
	install -d "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/examples" "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/docs"
	install -m644 README.md "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/"
	install -m644 docs/*.md docs/*.json "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/docs/"
	install -m644 examples/*.yaml "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/examples/"

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/game-super-runtime"
	rm -f "$(DESTDIR)$(PREFIX)/share/licenses/game-super-runtime/LICENSE"
	rm -f "$(DESTDIR)$(PREFIX)/share/man/man1/game-super-runtime.1"
	rm -f "$(DESTDIR)$(PREFIX)/share/bash-completion/completions/game-super-runtime"
	rm -f "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/README.md"
	for name in AI_GUIDE.md reference.md architecture.md packaging.md validation.md config.schema.json; do rm -f "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/docs/$$name"; done
	for name in defaults native 1091500 standalone-proton scripts wrappers; do rm -f "$(DESTDIR)$(PREFIX)/share/doc/game-super-runtime/examples/$$name.yaml"; done

dist:
	bash tools/make-dist.sh
