# LWSC2 Makefile
# Last War: Survival Game Automation Bot & State Detection Engine

export PATH := $(HOME)/.cargo/bin:$(PATH)

CARGO ?= cargo
STATES_DIR ?= roi
ROI_DIR ?= roi
MARGIN ?= 0.05

.PHONY: all build build-release bot run calc-roi calc-roi-apply test check clean help

all: build

## 🔨 Build
build:
	@$(CARGO) build

build-release:
	@$(CARGO) build --release

check:
	@$(CARGO) check

## 🤖 Run
bot:
	@$(CARGO) run --bin bot -- $(ARGS)

bot-headless:
	@DISPLAY=:0 $(CARGO) run --bin bot -- --headless $(ARGS)

bot-on-target:
	@DISPLAY=:0 $(CARGO) run --bin bot -- $(ARGS)

run:
	@$(CARGO) run --bin lwsc2 -- $(ARGS)

## 📐 ROI Tool
calc-roi:
	@$(CARGO) run --bin calc_roi -- $(STATES_DIR) --margin $(MARGIN)

calc-roi-apply:
	@$(CARGO) run --bin calc_roi -- $(STATES_DIR) --margin $(MARGIN) --apply

## 🧪 Tests & Quality
test:
	@$(CARGO) test

clean:
	@$(CARGO) clean

## 📖 Help
help:
	@echo ""
	@echo "LWSC2 - Makefile Commands:"
	@echo "------------------------------------------------------------------"
	@echo "  make build             Compile all binaries (debug)"
	@echo "  make build-release     Compile all binaries (optimized release)"
	@echo "  make check             Fast syntax and type checking"
	@echo ""
	@echo "  make bot               Launch the main game automation bot"
	@echo "  make bot-headless      Launch the bot in headless mode (perfect for SSH, no GUI)"
	@echo "  make bot-on-target     Launch the bot on target DISPLAY=:0 (GUI starts hidden, opens on target via Ctrl+O)"
	@echo "  make run ARGS=\"...\"    Run the lwsc2 CLI tool with custom arguments"
	@echo ""
	@echo "  make calc-roi          Calculate ROIs for states (dry-run)"
	@echo "                         Options: STATES_DIR=... MARGIN=0.05"
	@echo "  make calc-roi-apply    Calculate ROIs and apply directly to states.yaml"
	@echo ""
	@echo "  make test              Run the test suite"
	@echo "  make clean             Clean build artifacts (target/)"
	@echo "------------------------------------------------------------------"
	@echo ""
