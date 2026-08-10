pub mod registers;

use crate::rom::Mirroring;
use registers::control::ControlRegister;
use registers::mask::MaskRegister;
use registers::status::StatusRegister;
use registers::scroll::ScrollRegister;
use registers::address::AddressRegister;


pub struct PPU{
    chr_rom: Vec<u8>,
    // CHR-RAM (8 KB) used when chr_rom is empty
    chr_ram: [u8; 0x2000],
    pub mirroring: Mirroring,
    pub control: ControlRegister,
    pub mask: MaskRegister,
    pub status: StatusRegister,
    pub address: AddressRegister,
    pub scroll: ScrollRegister,
    pub vram: [u8; 0x800],
    pub oam_data: [u8; 0x100],
    pub oam_addr: u8,

    pub palette_table: [u8; 0x20],
    
    internal_buffer: u8,

    // Scanline-based timing. One NTSC frame is 262 scanlines × 341 dots.
    // scanline: 0–239 visible, 240 post-render, 241–260 vblank, 261 pre-render.
    // We track `cycle` modulo 341 and `scanline` modulo 262.
    scanline: u16,
    cycle: u16,
    // True for exactly one tick when VBlank becomes active and NMI is armed.
    nmi_pending: bool,

    // Frame buffer: 256 x 240 pixels, stored as palette indices (0-63).
    // Updated once per frame after scanline 239.
    frame: [u8; 256 * 240],
    // True when a new frame is ready.
    frame_ready: bool,
}

pub trait PPUInterface{
    fn write_to_control(&mut self, value: u8);
    fn write_to_mask(&mut self, value: u8);
    fn read_from_status(&mut self) -> u8;
    fn write_to_oam_addr(&mut self, value: u8);
    fn write_to_oam_data(&mut self, value: u8);
    fn read_from_oam_data(&mut self) -> u8;
    fn write_to_scroll(&mut self, value: u8);
    fn write_to_address(&mut self, value: u8);
    fn read_from_data(&mut self) -> u8;
    fn write_to_data(&mut self, value: u8);
    fn write_to_oam_dma(&mut self, data: &[u8; 0x100]);
}

impl PPU{
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> PPU{
        PPU{
            chr_rom,
            mirroring,
            control: ControlRegister::new(),
            mask: MaskRegister::new(),
            status: StatusRegister::new(),
            address: AddressRegister::new(),
            scroll: ScrollRegister::new(),
            vram: [0; 0x800],
            oam_data: [0; 0x100],
            oam_addr: 0,
            palette_table: [0; 0x20],
            internal_buffer: 0,
            scanline: 0,
            cycle: 0,
            nmi_pending: false,
            frame: [0; 256 * 240],
            frame_ready: false,
            chr_ram: [0; 0x2000],
        }
    }

    /// Read a byte from CHR space (0x0000–0x1FFF). Uses CHR-ROM if present,
    /// otherwise falls back to CHR-RAM.
    fn read_chr(&self, addr: u16) -> u8 {
        if !self.chr_rom.is_empty() {
            self.chr_rom[addr as usize]
        } else {
            self.chr_ram[addr as usize]
        }
    }

    pub fn new_empty_rom() -> PPU{
        PPU::new(vec![0;0x800], Mirroring::HORIZONTAL)
    }

    // Vertical:
    //   [ A ] [ B ]
    //   [ a ] [ b ]

    // Horizontal:
    //   [ A ] [ a ]
    //   [ B ] [ b ]

    pub fn mirror_vram_address(&self, addr: u16) -> u16{
        let mirrored = addr & 0x2FFF;
        let vram_index = mirrored - 0x2000;
        let name_table = vram_index / 0x400;

        match (&self.mirroring, name_table){
            (Mirroring::VERTICAL, 2) | (Mirroring::VERTICAL, 3) => vram_index - 0x800,
            (Mirroring::HORIZONTAL, 2) => vram_index - 0x400,
            (Mirroring::HORIZONTAL, 1) => vram_index - 0x400,
            (Mirroring::HORIZONTAL, 3) => vram_index - 0x800,
            _ => vram_index,
        }
    }

    fn increment_vram_addr(&mut self){
        let increment = self.control.vram_add_increment();
        self.address.increment(increment);
    }

    // ── Timing ──────────────────────────────────────────────────────────
    // NTSC PPU: 262 scanlines per frame, 341 dots per scanline.
    //   Lines  0–239:  visible rendering
    //   Line   240:    post-render (idle)
    //   Lines 241–260: vertical blanking
    //   Line  261:     pre-render (idle)
    //
    // VBlank flag is set at the start of scanline 241 (dot 1) and cleared at
    // the start of the pre-render line (dot 1). NMI fires on the same edge if
    // the control register's generate-NMI bit is set.
    /// Advance the PPU by one dot. Returns `true` if an NMI should be
    /// delivered to the CPU on this tick.
    pub fn tick(&mut self) -> bool {
        self.nmi_pending = false;

        // Advance dot counter first.
        self.cycle += 1;
        if self.cycle == 341 {
            // End of scanline — render if visible
            if self.scanline < 240 {
                self.render_scanline();
            }
            if self.scanline == 239 {
                self.end_of_frame();
            }

            self.cycle = 0;
            self.scanline = (self.scanline + 1) % 262;
        }

        // Rising edge entering scanline 241 at dot 1 → set VBlank
        if self.scanline == 241 && self.cycle == 1 {
            self.status.set_vblank_status(true);
            if self.control.generate_nmi() {
                self.nmi_pending = true;
            }
        }

        // Rising edge entering pre-render (line 261) at dot 1 → clear VBlank
        if self.scanline == 261 && self.cycle == 1 {
            self.status.set_vblank_status(false);
            // also clear sprite-zero-hit and sprite overflow per spec
            self.status.set_sprite_zero_hit(false);
            self.status.set_sprite_overflow(false);
        }

        self.nmi_pending
    }

    /// Convenience: advance N PPU dots.
    pub fn tick_n(&mut self, n: u32) -> bool {
        let mut nmi = false;
        for _ in 0..n {
            if self.tick() {
                nmi = true;
            }
        }
        nmi
    }

    pub fn scanline(&self) -> u16 { self.scanline }
    pub fn cycle(&self) -> u16 { self.cycle }

    /// Returns a reference to the current frame buffer (256×240 palette indices).
    /// Call this after `frame_ready()` returns `true`.
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    /// Returns `true` if a new frame has been rendered since last call.
    pub fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    /// Clears the `frame_ready` flag (call after consuming the frame).
    pub fn reset_frame_ready(&mut self) {
        self.frame_ready = false;
    }

    // ── Rendering ─────────────────────────────────────────────────────────
    // Called at the end of each visible scanline (0–239) to render one line
    // of background + sprites. Uses a simplified scanline-accurate approach:
    // for each of the 256 pixels on the scanline, we fetch the background
    // tile and sprite pixels and apply priority.
    fn render_scanline(&mut self) {
        let scanline = self.scanline as usize;
        if scanline >= 240 {
            return;
        }

        // Extract scroll from PPU registers
        let fine_y = self.scroll.fine_y() as usize;
        let coarse_y = self.scroll.coarse_y() as usize;
        let _coarse_x = self.scroll.coarse_x() as usize;
        let fine_x = self.scroll.fine_x() as usize;

        // Nametable base from control register
        let nt_base = self.control.nametable_addr();

        // Background pattern table base
        let bg_pt = self.control.background_pattern_addr();

        // Sprite pattern table base (8x8 mode for now)
        let spr_pt = self.control.sprite_pattern_addr();

        // For each of 256 pixels on this scanline
        for x in 0..256 {
            // ── Background ─────────────────────────────────────────────
            // Effective X coordinate in the nametable (with fine X scroll)
            let eff_x = (x + fine_x) % 256;
            let tile_x = eff_x / 8;
            let pixel_x = eff_x % 8;

            // Effective Y coordinate (with coarse/fine Y scroll)
            let eff_y = (scanline + fine_y + coarse_y * 8) % 240;
            let tile_y = eff_y / 8;
            let pixel_y = eff_y % 8;

            // Determine which nametable (horizontal/vertical mirroring handled by mirror_vram_address)
            let nt_addr = nt_base + (tile_y * 32 + tile_x) as u16;
            let tile_idx = self.vram[self.mirror_vram_address(nt_addr) as usize] as u16;

            // Fetch pattern bits for this scanline row of the tile
            // Pattern table: each tile = 16 bytes (8 low bits, 8 high bits)
            let pt_addr = bg_pt + tile_idx * 16 + pixel_y as u16;
            let low_bit = (self.read_chr(pt_addr) >> (7 - pixel_x)) & 1;
            let high_bit = (self.read_chr(pt_addr + 8) >> (7 - pixel_x)) & 1;
            let bg_palette_idx = (high_bit << 1) | low_bit; // 0–3

            // Attribute table: each byte covers 4x4 tiles, 2 bits per tile
            // Attribute offset within $23C0–$23FF (relative to nametable base)
            let at_tile_x = (tile_x / 4) as u16;
            let at_tile_y = (tile_y / 4) as u16;
            let at_addr = nt_base + 0x3C0 + at_tile_y * 8 + at_tile_x;
            let at_byte = self.vram[self.mirror_vram_address(at_addr) as usize];
            // Shift to get the 2 bits for this tile
            let at_shift = ((at_tile_y % 2) * 4 + (at_tile_x % 2) * 2) as u8;
            let at_bits = (at_byte >> at_shift) & 0b11;
            let bg_palette = (at_bits << 2) | bg_palette_idx; // 0–15 (index into palette RAM)

            // Read palette color from $3F00 + bg_palette
            let bg_color = self.palette_table[bg_palette as usize];

            // ── Sprites ───────────────────────────────────────────────
            // OAM: 64 sprites × 4 bytes = 256 bytes
            // Each sprite: Y, Tile, Attributes, X
            // We use the "front-to-back" priority: first sprite in OAM wins
            let mut sprite_color = 0;
            let mut sprite_priority = false; // true = behind background
            let mut sprite_zero_hit = false;

            for i in 0..64 {
                let base = i * 4;
                let spr_y = self.oam_data[base] as usize;
                let spr_tile = self.oam_data[base + 1] as u16;
                let spr_attr = self.oam_data[base + 2];
                let spr_x = self.oam_data[base + 3] as usize;

                // Check if sprite is on this scanline (8x8 sprites for now)
                if scanline >= spr_y && scanline < spr_y + 8 {
                    let row = scanline - spr_y;
                    let flip_v = (spr_attr & 0x80) != 0;
                    let flip_h = (spr_attr & 0x40) != 0;
                    sprite_priority = (spr_attr & 0x20) != 0; // behind background
                    let spr_palette = spr_attr & 0x03;

                    let py = if flip_v { 7 - row } else { row };
                    let pt_addr = spr_pt + spr_tile * 16 + py as u16;
                    // Actually we need to account for pixel_x within the sprite
                    // For simplicity, we check if this pixel x overlaps the sprite
                    if x >= spr_x && x < spr_x + 8 {
                        let px = x - spr_x;
                        let px = if flip_h { 7 - px } else { px };
                        let low = (self.read_chr(pt_addr) >> (7 - px)) & 1;
                        let high = (self.read_chr(pt_addr + 8) >> (7 - px)) & 1;
                        let sp_idx = (high << 1) | low;
                        if sp_idx != 0 {
                            // Sprite palette: $3F10 + spr_palette*4 + sp_idx
                            sprite_color = self.palette_table[0x10 + spr_palette as usize * 4 + sp_idx as usize];
                            sprite_zero_hit = i == 0;
                            break; // first opaque sprite wins
                        }
                    }
                }
            }

            // ── Priority ──────────────────────────────────────────────
            let final_color = if sprite_color != 0 && (!sprite_priority || bg_palette_idx == 0) {
                // Sprite is opaque and either in front of BG or BG is transparent
                if sprite_zero_hit && bg_palette_idx != 0 && x < 255 {
                    self.status.set_sprite_zero_hit(true);
                }
                sprite_color
            } else {
                bg_color
            };

            self.frame[scanline * 256 + x] = final_color;
        }
    }

    /// Called at the end of a frame (scanline 239) to mark frame ready.
    fn end_of_frame(&mut self) {
        self.frame_ready = true;
    }
}

impl PPUInterface for PPU{

    fn write_to_control(&mut self, value: u8) {
        let _before_nmi_status = self.control.generate_nmi();
        self.control.update(value);
    }

    fn write_to_mask(&mut self, value: u8) {
        self.mask.update(value);
    }

    fn read_from_status(&mut self) -> u8 {
        let data = self.status.snapshot();
        self.status.reset_vblank_status();
        self.address.reset_latch();
        self.scroll.reset_latch();
        data
    }

    fn write_to_oam_addr(&mut self, value: u8) {
        self.oam_addr = value;
    }

    fn write_to_oam_data(&mut self, value: u8) {
        self.oam_data[self.oam_addr as usize] = value;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    fn read_from_oam_data(&mut self) -> u8 {
        self.oam_data[self.oam_addr as usize]
    }

    fn write_to_scroll(&mut self, value: u8) {
        self.scroll.write(value);
    }

    fn write_to_address(&mut self, value: u8) {
        self.address.update(value);
    }

    fn write_to_data(&mut self, value: u8) {
        let addr = self.address.get();
        match addr{
            0..=0x1FFF => print!("Attempted to write to CHR-ROM at {:04X}", addr),
            
            0x2000..=0x2FFF => {
                self.vram[self.mirror_vram_address(addr) as usize] = value;
            }

            0x3000..=0x3EFF => unimplemented!("{} shouldnt be written to", addr),

            0x3f10 | 0x3f14 | 0x3f18 | 0x3f1c => {
                let add_mirror = addr - 0x10;
                self.palette_table[(add_mirror - 0x3f00) as usize] = value;
            }

            0x3f00..=0x3fff =>
            {
                self.palette_table[(addr - 0x3f00) as usize] = value;
            }
            
            _ => panic!("Attempted to write to invalid address {:04X}", addr),
        }
        self.increment_vram_addr();
    }

    fn read_from_data(&mut self) -> u8 {
        let addr = self.address.get();
        
        self.increment_vram_addr();

        match addr {
            0..=0x1fff => {
                let result = self.internal_buffer;
                self.internal_buffer = self.read_chr(addr);
                result
            }
            0x2000..=0x2fff => {
                let result = self.internal_buffer;
                self.internal_buffer = self.vram[self.mirror_vram_address(addr) as usize];
                result
            }
            0x3000..=0x3eff => unimplemented!("addr {} shouldn't be used in reallity", addr),

            //Addresses $3F10/$3F14/$3F18/$3F1C are mirrors of $3F00/$3F04/$3F08/$3F0C
            0x3f10 | 0x3f14 | 0x3f18 | 0x3f1c => {
                let add_mirror = addr - 0x10;
                self.palette_table[(add_mirror - 0x3f00) as usize]
            }

            0x3f00..=0x3fff =>
            {
                self.palette_table[(addr - 0x3f00) as usize]
            }
            _ => panic!("unexpected access to mirrored space {}", addr),
        }
    }

    fn write_to_oam_dma(&mut self, data: &[u8; 0x100]) {
        for i in data.iter(){
            self.oam_data[self.oam_addr as usize] = *i;
            self.oam_addr = self.oam_addr.wrapping_add(1);
        }
    }

}

// tests
#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_ppu_vram_writes() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_address(0x23);
        ppu.write_to_address(0x05);
        ppu.write_to_data(0x66);

        assert_eq!(ppu.vram[0x0305], 0x66);
    }

    #[test]
    fn test_ppu_vram_reads() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_control(0);
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_address(0x23);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load_into_buffer
        assert_eq!(ppu.address.get(), 0x2306);
        assert_eq!(ppu.read_from_data(), 0x66);
    }

    #[test]
    fn test_ppu_vram_reads_cross_page() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_control(0);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x0200] = 0x77;

        ppu.write_to_address(0x21);
        ppu.write_to_address(0xff);

        ppu.read_from_data(); //load_into_buffer
        assert_eq!(ppu.read_from_data(), 0x66);
        assert_eq!(ppu.read_from_data(), 0x77);
    }

    #[test]
    fn test_ppu_vram_reads_step_32() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_control(0b100);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x01ff + 32] = 0x77;
        ppu.vram[0x01ff + 64] = 0x88;

        ppu.write_to_address(0x21);
        ppu.write_to_address(0xff);

        ppu.read_from_data(); //load_into_buffer
        assert_eq!(ppu.read_from_data(), 0x66);
        assert_eq!(ppu.read_from_data(), 0x77);
        assert_eq!(ppu.read_from_data(), 0x88);
    }

    // Horizontal: https://wiki.nesdev.com/w/index.php/Mirroring
    //   [0x2000 A ] [0x2400 a ]
    //   [0x2800 B ] [0x2C00 b ]
    #[test]
    fn test_vram_horizontal_mirror() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_address(0x24);
        ppu.write_to_address(0x05);

        ppu.write_to_data(0x66); //write to a

        ppu.write_to_address(0x28);
        ppu.write_to_address(0x05);

        ppu.write_to_data(0x77); //write to B

        ppu.write_to_address(0x20);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load into buffer
        assert_eq!(ppu.read_from_data(), 0x66); //read from A

        ppu.write_to_address(0x2C);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load into buffer
        assert_eq!(ppu.read_from_data(), 0x77); //read from b
    }

    // Vertical: https://wiki.nesdev.com/w/index.php/Mirroring
    //   [0x2000 A ] [0x2400 B ]
    //   [0x2800 a ] [0x2C00 b ]
    #[test]
    fn test_vram_vertical_mirror() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::VERTICAL);

        ppu.write_to_address(0x20);
        ppu.write_to_address(0x05);

        ppu.write_to_data(0x66); //write to A

        ppu.write_to_address(0x2C);
        ppu.write_to_address(0x05);

        ppu.write_to_data(0x77); //write to b

        ppu.write_to_address(0x28);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load into buffer
        assert_eq!(ppu.read_from_data(), 0x66); //read from a

        ppu.write_to_address(0x24);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load into buffer
        assert_eq!(ppu.read_from_data(), 0x77); //read from B
    }

    #[test]
    fn test_read_from_status_resets_latch() {
        let mut ppu = PPU::new_empty_rom();
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_address(0x21);
        ppu.write_to_address(0x23);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load_into_buffer
        assert_ne!(ppu.read_from_data(), 0x66);

        ppu.read_from_status();

        ppu.write_to_address(0x23);
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load_into_buffer
        assert_eq!(ppu.read_from_data(), 0x66);
    }

    #[test]
    fn test_ppu_vram_mirroring() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_control(0);
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_address(0x63); //0x6305 -> 0x2305
        ppu.write_to_address(0x05);

        ppu.read_from_data(); //load into_buffer
        assert_eq!(ppu.read_from_data(), 0x66);
        // assert_eq!(ppu.addr.read(), 0x0306)
    }

    #[test]
    fn test_read_from_status_resets_vblank() {
        let mut ppu = PPU::new_empty_rom();
        ppu.status.set_vblank_status(true);

        let status = ppu.read_from_status();

        assert_eq!(status >> 7, 1);
        assert_eq!(ppu.status.snapshot() >> 7, 0);
    }

    #[test]
    fn test_oam_read_write() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_oam_addr(0x10);
        ppu.write_to_oam_data(0x66);
        ppu.write_to_oam_data(0x77);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_from_oam_data(), 0x66);

        ppu.write_to_oam_addr(0x11);
        assert_eq!(ppu.read_from_oam_data(), 0x77);
    }

    #[test]
        fn test_oam_dma() {
            let mut ppu = PPU::new_empty_rom();

            let mut data = [0x66; 256];
            data[0] = 0x77;
            data[255] = 0x88;

            ppu.write_to_oam_addr(0x10);
            ppu.write_to_oam_dma(&data);

            ppu.write_to_oam_addr(0xf); //wrap around
            assert_eq!(ppu.read_from_oam_data(), 0x88);

            ppu.write_to_oam_addr(0x10);
            assert_eq!(ppu.read_from_oam_data(), 0x77);

            ppu.write_to_oam_addr(0x11);
            assert_eq!(ppu.read_from_oam_data(), 0x66);
        }

        #[test]
            fn test_vblank_flag_set_at_scanline_241() {
                let mut ppu = PPU::new_empty_rom();
                // Fast-forward to scanline 240, cycle 340 (one tick before VBlank trigger)
                for _ in 0..240 {
                    ppu.tick_n(341);
                }
                ppu.tick_n(340); // scanline 240, cycle 340

                // Next tick → scanline 241, cycle 0 (no VBlank yet)
                ppu.tick();
                assert!(!ppu.status.is_in_vblank());

                // Next tick → scanline 241, cycle 1 (VBlank set!)
                let nmi = ppu.tick();
                assert!(ppu.status.is_in_vblank());
                assert!(!nmi); // NMI not armed by default
            }

            #[test]
            fn test_vblank_flag_cleared_at_pre_render() {
                let mut ppu = PPU::new_empty_rom();
                // Fast-forward to scanline 260, cycle 340
                for _ in 0..260 {
                    ppu.tick_n(341);
                }
                ppu.tick_n(340);

                // VBlank should be active
                assert!(ppu.status.is_in_vblank());

                // Next tick → scanline 261, cycle 0 (still in VBlank)
                ppu.tick();
                assert!(ppu.status.is_in_vblank());

                // Next tick → scanline 261, cycle 1 (VBlank cleared!)
                ppu.tick();
                assert!(!ppu.status.is_in_vblank());
            }

            #[test]
            fn test_nmi_fires_when_generate_nmi_set() {
                let mut ppu = PPU::new_empty_rom();
                // Enable NMI
                ppu.write_to_control(0x80); // GENERATE_NMI = 1

                // Fast-forward to scanline 240, cycle 340
                for _ in 0..240 {
                    ppu.tick_n(341);
                }
                ppu.tick_n(340);

                // Advance two ticks to hit VBlank at scanline 241, cycle 1
                ppu.tick(); // scanline 241, cycle 0
                let nmi = ppu.tick(); // scanline 241, cycle 1 → NMI!
                assert!(ppu.status.is_in_vblank());
                assert!(nmi);
            }

            #[test]
            fn test_no_nmi_when_generate_nmi_clear() {
                let mut ppu = PPU::new_empty_rom();
                ppu.write_to_control(0x00); // GENERATE_NMI = 0

                for _ in 0..240 {
                    ppu.tick_n(341);
                }
                ppu.tick_n(340);

                ppu.tick(); // scanline 241, cycle 0
                let nmi = ppu.tick(); // scanline 241, cycle 1
                assert!(ppu.status.is_in_vblank());
                assert!(!nmi); // NMI disabled
            }

            #[test]
            fn test_tick_n_returns_nmi_if_any() {
                let mut ppu = PPU::new_empty_rom();
                ppu.write_to_control(0x80);

                // Fast-forward to scanline 240, cycle 339
                for _ in 0..240 {
                    ppu.tick_n(341);
                }
                ppu.tick_n(339);

                // Next 3 ticks: 
                //  1: scanline 240, cycle 340 (no NMI)
                //  2: scanline 241, cycle 0 (no NMI)
                //  3: scanline 241, cycle 1 (NMI!)
                let nmi = ppu.tick_n(3);
                assert!(nmi);
            }
    }