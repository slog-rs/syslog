PKG_NAME=$(shell grep name Cargo.toml | head -n 1 | awk -F \" '{print $$2}')
DOCS_DEFAULT_MODULE=$(subst -,_,$(PKG_NAME))
ifeq (, $(shell which cargo-check 2> /dev/null))
DEFAULT_TARGET=build
else
DEFAULT_TARGET=build
endif

default: $(DEFAULT_TARGET)

ALL_TARGETS += build $(EXAMPLES) test doc
ifneq ($(RELEASE),)
$(info RELEASE BUILD: $(PKG_NAME))
CARGO_FLAGS += --release
else
$(info DEBUG BUILD: $(PKG_NAME); use `RELEASE=true make [args]` for release build)
endif
CARGO_FEATURES=serde

EXAMPLES = $(shell cd examples 2>/dev/null && ls *.rs 2>/dev/null | sed -e 's/.rs$$//g' )

all: $(ALL_TARGETS)

.PHONY: run test build doc clean clippy
run test build:
	cargo $@ --features $(CARGO_FEATURES) $(CARGO_FLAGS)

clean:
	cargo $@ $(CARGO_FLAGS)

check:
	$(info Running check; use `make build` to actually build)
	cargo $@ --features $(CARGO_FEATURES) $(CARGO_FLAGS)

clippy:
	cargo build --features clippy,$(CARGO_FEATURES)

.PHONY: bench
bench:
	cargo $@ --features $(CARGO_FEATURES) $(filter-out --release,$(CARGO_FLAGS))

.PHONY: travistest
travistest: test

.PHONY: longtest
longtest:
	@echo "Running longtest. Press Ctrl+C to stop at any time"
	@sleep 2
	@i=0; while i=$$((i + 1)) && echo "Iteration $$i" && make test ; do :; done

.PHONY: $(EXAMPLES)
$(EXAMPLES):
	cargo build --example $@ $(CARGO_FLAGS)

.PHONY: doc
doc: FORCE
	cargo --features $(CARGO_FEATURES) doc

.PHONY: publishdoc
publishdoc:
	rm -rf target/doc
	make doc
	echo '<meta http-equiv="refresh" content="0;url='${DOCS_DEFAULT_MODULE}'/index.html">' > target/doc/index.html
	ghp-import -n target/doc
	git push -f origin gh-pages

.PHONY: docview
docview: doc
	xdg-open target/doc/$(PKG_NAME)/index.html

.PHONY: FORCE
FORCE:
