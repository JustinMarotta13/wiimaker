# wiimaker Wii runtime
# Requires: DEVKITPRO with wii-dev (or use ../../tools/wii-build.sh)

ifeq ($(strip $(DEVKITPPC)),)
$(error "Set DEVKITPRO / DEVKITPPC (source /etc/profile.d/devkit-env.sh)")
endif

include $(DEVKITPPC)/wii_rules

GAME      ?= hello-orb
TARGET    := boot
BUILD     := build
GAME_DIR  := ../../games/$(GAME)
WPACK     := $(GAME_DIR)/assets.wpack
WSCN      := $(GAME_DIR)/scene.wscn

# Prefer a Rust staticlib when present; otherwise link the C scene player.
RUSTLIB   := ../../target/powerpc-unknown-eabi/release/lib$(subst -,_,$(GAME)).a
USE_STUB  := $(if $(wildcard $(RUSTLIB)),0,1)

CFLAGS    := -g -O2 -Wall $(MACHDEP) -I$(CURDIR)/include -I$(LIBOGC_INC)
LDFLAGS   := -g $(MACHDEP) -Wl,-Map,$(BUILD)/$(TARGET).map
LIBS      := -lwiiuse -lbte -logc -lm
LIBPATHS  := -L$(LIBOGC_LIB)

ifeq ($(USE_STUB),0)
LIBS      := -L$(dir $(RUSTLIB)) -l$(subst -,_,$(GAME)) $(LIBS)
GAME_OBJ  :=
EMBED_OBJ :=
else
GAME_OBJ  := $(BUILD)/stub_game.o
EMBED_OBJ := $(BUILD)/assets_wpack.o $(BUILD)/scene_wscn.o
endif

OFILES    := $(BUILD)/bootstrap.o $(BUILD)/gx_draw.o $(GAME_OBJ) $(EMBED_OBJ)

.PHONY: all clean

all: $(BUILD) $(TARGET).dol
	@mkdir -p ../../target/wii/$(GAME)
	@cp $(TARGET).dol ../../target/wii/$(GAME)/boot.dol
	@cp ../meta/meta.xml ../../target/wii/$(GAME)/meta.xml 2>/dev/null || true
	@echo "=> ../../target/wii/$(GAME)/boot.dol  (stub=$(USE_STUB))"

$(BUILD):
	@mkdir -p $@

$(BUILD)/bootstrap.o: src/bootstrap.c include/wiimaker_abi.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/gx_draw.o: src/gx_draw.c include/wiimaker_abi.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/stub_game.o: src/stub_game.c include/wiimaker_abi.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

# Copy into build/ first so objcopy symbols are _binary_assets_wpack_* (not path-mangled).
$(BUILD)/assets.wpack: $(WPACK) | $(BUILD)
	@test -f $(WPACK) || (echo "missing $(WPACK) — run: wiimaker cook $(GAME) && wiimaker bake-wii $(GAME)"; exit 1)
	cp $(WPACK) $@

$(BUILD)/scene.wscn: $(WSCN) | $(BUILD)
	@test -f $(WSCN) || (echo "missing $(WSCN) — run: wiimaker cook $(GAME) && wiimaker bake-wii $(GAME)"; exit 1)
	cp $(WSCN) $@

$(BUILD)/assets_wpack.o: $(BUILD)/assets.wpack
	cd $(BUILD) && $(OBJCOPY) -I binary -O elf32-powerpc -B powerpc \
		--rename-section .data=.rodata,alloc,load,readonly,data,contents \
		assets.wpack assets_wpack.o

$(BUILD)/scene_wscn.o: $(BUILD)/scene.wscn
	cd $(BUILD) && $(OBJCOPY) -I binary -O elf32-powerpc -B powerpc \
		--rename-section .data=.rodata,alloc,load,readonly,data,contents \
		scene.wscn scene_wscn.o

$(TARGET).elf: $(OFILES)
	$(CC) $^ $(LDFLAGS) $(LIBPATHS) $(LIBS) -o $@

clean:
	rm -rf $(BUILD) $(TARGET).elf $(TARGET).dol
