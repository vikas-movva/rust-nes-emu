# rust-nes-emu

A cycle-accurate NES emulator written in Rust, following the [Writing NES Emulator in Rust](https://bugzmanov.github.io/nes_ebook/) ebook by bugzmanov.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.80+-orange.svg)
![SDL2](https://img.shields.io/badge/SDL2-2.30+-green.svg)

---

## Features

| Subsystem | Status |
|-----------|--------|
| **CPU (6502)** | ✅ All 151 official + unofficial opcodes; cycle-accurate `step()` |
| **PPU** | ✅ Scanline timing (262×341), VBlank NMI, background + 8×8 sprite rendering, palette indexing |
| **Mapper 0 (NROM)** | ✅ NROM-128 (16 KB mirrored) + NROM-256 (32 KB); CHR-ROM |
| **Bus** | ✅ Full memory map: RAM, PPU regs ($2000–$2007), OAM DMA ($4014), joypad ($4016/$4017), APU stubs |
| **Input** | ✅ Real NES joypad protocol (shift register at $4016/$4017) via SDL2 keyboard |
| **ROM loading** | ✅ iNES v1 header parse; NES2.0 rejected |
| **Tests** | ✅ 39 passing (CPU, PPU, Mapper, Bus, ROM) |

---

## Quick Start

### Prerequisites
- **Rust** 1.80+ (`rustup default stable`)
- **SDL2** native library
  - macOS: `brew install sdl2`
  - Ubuntu/Debian: `sudo apt install libsdl2-dev`
  - Windows: `vcpkg install sdl2` or MSYS2 `pacman -S mingw-w64-x86_64-SDL2`

### Build & Test
```bash
cargo test
```

### Run the bundled Snake ROM (legacy CPU-RAM scrape path)
```bash
cargo run
```

### Run a real NES ROM (Mapper 0 / NROM)
```bash
cargo run -- "path/to/game.nes"
```

**Example (Super Mario Bros. — Mapper 0, NROM-128):**
```bash
cargo run -- "Mario Bros. (World).nes"
```

---

## Controls

| Key | Joypad Button |
|-----|---------------|
| `J` / `Space` | A |
| `K` / `LShift` | B |
| `RShift` | Select |
| `Enter` | Start |
| `W` / `↑` | Up |
| `S` / `↓` | Down |
| `A` / `←` | Left |
| `D` / `→` | Right |
| `Esc` | Quit |

---

## Architecture

```
src/
├── main.rs          # SDL2 host: CLI, window, frame presentation, input
├── cpu.rs           # 6502 CPU: registers, flags, addressing, instructions, step()/nmi()/irq()
├── opcodes.rs       # Opcode table: mnemonic, length, cycles, addressing mode
├── bus.rs           # Memory map: RAM, PPU, APU stubs, OAM DMA, joypad, mapper delegation
├── mapper.rs        # Mapper trait + Nrom (Mapper 0) implementation
├── rom.rs           # iNES header parser
├── ppu/
│   ├── mod.rs       # PPU: scanline timing, VBlank NMI, render_scanline(), frame buffer
│   └── registers/   # Control, Mask, Status, Scroll, Address register bitflags
└── snake.rs         # One-shot test ROM generator (not used at runtime)
```

### CPU ↔ PPU Timing
- NTSC: 262 scanlines × 341 PPU dots per frame
- CPU:PPU cycle ratio = 1:3
- `CPU::step()` returns instruction cycles → `run_with_callback` calls `PPU::tick()` 3× per CPU cycle
- VBlank NMI fires at scanline 241, dot 1 if `CTRL.generate_nmi=1`
- Frame buffer ready after scanline 239

### Mapper System
```rust
pub trait Mapper: Send {
    fn read_prg(&self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, value: u8);
    fn read_chr(&self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, value: u8);
    fn mirroring(&self) -> Mirroring;
}
```
- `Nrom` implements Mapper 0 (NROM-128/256)
- Ready for MMC1, UxROM, CNROM, MMC3, etc.

---

## Tested ROMs

| ROM | Mapper | PRG | CHR | Status |
|-----|--------|-----|-----|--------|
| `snake.nes` | 0 (test ROM) | 32 KB | 0 KB | ✅ Legacy CPU-RAM scrape |
| `Mario Bros. (World).nes` | 0 (NROM-128) | 16 KB | 8 KB | ✅ Full PPU rendering |
| `nestest.nes` | 0 (NROM-256) | 32 KB | 8 KB | ✅ Boots (CI target) |

---

## Roadmap

See [`docs/PROGRESS.md`](docs/PROGRESS.md) for detailed progress and open issues.

**Next milestones:**
1. MMC1 (Mapper 1) — Zelda, Metroid, Mega Man 2
2. UxROM (Mapper 2) — Castlevania, Contra
3. CNROM (Mapper 3) — Arkanoid
4. MMC3 (Mapper 4) — SMB3, Kirby (scanline IRQ)
5. 8×16 sprite mode
6. Fine scrolling (v/t/x internal registers)
7. APU (pulse, triangle, noise, DMC)
8. `nestest.nes` CI integration

---

## License

MIT — see [`LICENSE`](LICENSE) (add one if you want).

---

## Credits

- [bugzmanov — Writing NES Emulator in Rust](https://bugzmanov.github.io/nes_ebook/) — primary reference
- [NESDev Wiki](https://www.nesdev.org/wiki/) — definitive hardware documentation
- [6502 Opcode Reference](https://www.masswerk.at/6502/6502_instruction_set.html)

---

## Screenshots

*(Add screenshots after running `cargo run -- "Mario Bros. (World).nes"`)*