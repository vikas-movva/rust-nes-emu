pub mod bus;
pub mod rom;
pub mod cpu;
pub mod opcodes;
pub mod ppu;
pub mod mapper;

use bus::BUS;
use rom::ROM;
use cpu::CPU;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::keyboard::Scancode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::EventPump;

use std::time::{Duration, Instant};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;

// NES palette (approximate NTSC colors)
// Source: https://www.nesdev.org/wiki/PPU_palettes#Color_$00-$0F
const NES_PALETTE: [(u8, u8, u8); 64] = [
    (0x75, 0x75, 0x75), (0x27, 0x1B, 0x8F), (0x00, 0x00, 0xAB), (0x47, 0x00, 0x9F),
    (0x8F, 0x00, 0x77), (0xAB, 0x00, 0x13), (0xA7, 0x00, 0x00), (0x7F, 0x0B, 0x00),
    (0x43, 0x2F, 0x00), (0x00, 0x47, 0x00), (0x00, 0x51, 0x00), (0x00, 0x3F, 0x17),
    (0x1B, 0x3F, 0x5F), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
    (0xBC, 0xBC, 0xBC), (0x00, 0x73, 0xEF), (0x23, 0x3B, 0xEF), (0x83, 0x00, 0xF3),
    (0xBF, 0x00, 0xBF), (0xE7, 0x00, 0x5B), (0xDB, 0x2B, 0x00), (0xCB, 0x4F, 0x0F),
    (0x8B, 0x73, 0x00), (0x00, 0x97, 0x00), (0x00, 0xAB, 0x00), (0x00, 0x93, 0x3B),
    (0x00, 0x83, 0x8B), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
    (0xFF, 0xFF, 0xFF), (0x3F, 0xBF, 0xFF), (0x5F, 0x97, 0xFF), (0xA7, 0x8B, 0xFF),
    (0xF7, 0x7B, 0xFF), (0xFF, 0x77, 0xB7), (0xFF, 0x77, 0x63), (0xFF, 0x9B, 0x3B),
    (0xF3, 0xBF, 0x3F), (0x83, 0xD3, 0x13), (0x4F, 0xDF, 0x4B), (0x58, 0xF8, 0x98),
    (0x00, 0xEB, 0xDB), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
    (0xFF, 0xFF, 0xFF), (0xAB, 0xE7, 0xFF), (0xC7, 0xD7, 0xFF), (0xD7, 0xCB, 0xFF),
    (0xFF, 0xC7, 0xFF), (0xFF, 0xC7, 0xDB), (0xFF, 0xBF, 0xB3), (0xFF, 0xDB, 0xAB),
    (0xFF, 0xE7, 0xA3), (0xE3, 0xFF, 0xA3), (0xAB, 0xF3, 0xBF), (0xB3, 0xFF, 0xCF),
    (0x9F, 0xFF, 0xF3), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
];

// Convert the PPU's 256x240 palette-index frame buffer into an RGB24 byte
// vector suitable for an SDL texture. Each palette index (0-63) is mapped
// through `NES_PALETTE`. This is real PPU rendering — the frame buffer is
// produced by `PPU::render_scanline` each visible line and made available
// once per frame via `frame_ready()`.
fn ppu_frame_to_rgb(ppu: &ppu::PPU) -> Vec<u8> {
    let frame = ppu.frame();
    let mut rgb = vec![0u8; 256 * 240 * 3];
    for i in 0..(256 * 240) {
        let idx = (frame[i] & 0x3F) as usize;
        let (r, g, b) = NES_PALETTE[idx];
        rgb[i * 3] = r;
        rgb[i * 3 + 1] = g;
        rgb[i * 3 + 2] = b;
    }
    rgb
}

fn handle_user_input(cpu: &mut CPU, event_pump: &mut EventPump) {
    // Drain the SDL event queue. ESC / window-close exits. For real NES
    // games input is read via $4016/$4017 (the joypad shift register) which
    // the bus already routes to `joypad1`/`joypad2`. Webuild the 8-bit
    // joypad state here from held keys.
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => std::process::exit(0),
            _ => {}
        }
    }

    // Build joypad state from currently-held keys.
    // Button bit order (as read from $4016): A B Select Start Up Down Left Right.
    let keys = event_pump.keyboard_state();
    let pressed = |sc: Scancode| keys.is_scancode_pressed(sc);

    let mut joypad_state: u8 = 0;

    if pressed(Scancode::J) || pressed(Scancode::Space) {
        joypad_state |= 1 << 0; // A
    }
    if pressed(Scancode::K) || pressed(Scancode::LShift) {
        joypad_state |= 1 << 1; // B
    }
    if pressed(Scancode::RShift) {
        joypad_state |= 1 << 2; // Select
    }
    if pressed(Scancode::Return) {
        joypad_state |= 1 << 3; // Start
    }
    if pressed(Scancode::W) || pressed(Scancode::Up) {
        joypad_state |= 1 << 4; // Up
    }
    if pressed(Scancode::S) || pressed(Scancode::Down) {
        joypad_state |= 1 << 5; // Down
    }
    if pressed(Scancode::A) || pressed(Scancode::Left) {
        joypad_state |= 1 << 6; // Left
    }
    if pressed(Scancode::D) || pressed(Scancode::Right) {
        joypad_state |= 1 << 7; // Right
    }

    cpu.bus.set_joypad1(joypad_state);
}

fn main() {
    // ROM path: first CLI arg, else default to `snake.nes` for backward compat.
    let rom_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "snake.nes".to_string());

    let bytes = std::fs::read(&rom_path).unwrap_or_else(|e| {
        eprintln!("Failed to read ROM '{}': {}", rom_path, e);
        std::process::exit(1);
    });
    let rom = ROM::new(&bytes).unwrap_or_else(|e| {
        eprintln!("Invalid iNES ROM '{}': {}", rom_path, e);
        std::process::exit(1);
    });

    println!(
        "Loaded '{}': mapper {}, PRG {}KB, CHR {}KB, mirroring {:?}",
        rom_path,
        rom.mapper,
        rom.prg_rom.len() / 1024,
        rom.chr_rom.len() / 1024,
        rom.screen_mirroring,
    );

    // init sdl2
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("NES Emu", 256 * 3, 240 * 3)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(3.0, 3.0).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 256, 240)
        .unwrap();

    let bus = BUS::new(rom);
    let mut cpu = CPU::new(bus);
    cpu.reset();

    // run the game cycle.
    //
    // `run_with_callback` executes one CPU instruction per iteration, ticks
    // the PPU 3 cycles per CPU cycle (driving scanline rendering + NMI), then
    // calls us back. We poll input every instruction (cheap) and present a
    // frame only when the PPU signals `frame_ready()` (once per ~1/60 s).
    let frame_time = Duration::from_nanos(1_000_000_000 / 60);
    let mut last_frame = Instant::now();

    cpu.run_with_callback(move |cpu| {
        handle_user_input(cpu, &mut event_pump);

        if cpu.bus.ppu().frame_ready() {
            let rgb_frame = ppu_frame_to_rgb(cpu.bus.ppu());
            texture.update(None, &rgb_frame, 256 * 3).unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
            cpu.bus.ppu_mut().reset_frame_ready();

            // Cap to ~60 FPS.
            let elapsed = last_frame.elapsed();
            if elapsed < frame_time {
                std::thread::sleep(frame_time - elapsed);
            }
            last_frame = Instant::now();
        }
    });
}
