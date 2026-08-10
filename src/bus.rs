use crate::rom::ROM;
use crate::cpu::Mem;
use crate::ppu::PPU;
use crate::ppu::PPUInterface;
use crate::mapper::{Mapper, Nrom};

//  _______________ $10000  _______________
// | PRG-ROM       |       |               |
// | Upper Bank    |       |               |
// |_ _ _ _ _ _ _ _| $C000 | PRG-ROM       |
// | PRG-ROM       |       |               |
// | Lower Bank    |       |               |
// |_______________| $8000 |_______________|
// | SRAM          |       | SRAM          |
// |_______________| $6000 |_______________|
// | Expansion ROM |       | Expansion ROM |
// |_______________| $4020 |_______________|
// | I/O Registers |       |               |
// |_ _ _ _ _ _ _ _| $4000 |               |
// | Mirrors       |       | I/O Registers |
// | $2000-$2007   |       |               |
// |_ _ _ _ _ _ _ _| $2008 |               |
// | I/O Registers |       |               |
// |_______________| $2000 |_______________|
// | Mirrors       |       |               |
// | $0000-$07FF   |       |               |
// |_ _ _ _ _ _ _ _| $0800 |               |
// | RAM           |       | RAM           |
// |_ _ _ _ _ _ _ _| $0200 |               |
// | Stack         |       |               |
// |_ _ _ _ _ _ _ _| $0100 |               |
// | Zero Page     |       |               |
// |_______________| $0000 |_______________|
const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;

pub struct BUS {
    cpu_vram: [u8; 2048],
    mapper: Box<dyn Mapper>,
    ppu: PPU,
    // Joypad state (2 controllers, 8 buttons each)
    joypad1: u8,
    joypad2: u8,
    joypad1_shift: u8,
    joypad2_shift: u8,
    joypad_strobe: bool,
    // CPU-visible cycles accumulated since power-on; used by the host loop to
    // drive the PPU 3 cycles per CPU cycle.
    pub cpu_cycles: u64,
}

impl BUS {
    pub fn new(rom: ROM) -> Self {
        let mirroring = rom.screen_mirroring.clone();
        let ppu = PPU::new(rom.chr_rom.clone(), rom.screen_mirroring);
        let mapper: Box<dyn Mapper> = match rom.mapper {
            0 => Box::new(Nrom::new(rom.prg_rom, rom.chr_rom, mirroring)),
            _ => panic!("Mapper {} is not supported yet", rom.mapper),
        };
        BUS {
            cpu_vram: [0; 2048],
            ppu,
            mapper,
            joypad1: 0,
            joypad2: 0,
            joypad1_shift: 0,
            joypad2_shift: 0,
            joypad_strobe: false,
            cpu_cycles: 0,
        }
    }

    pub fn ppu(&self) -> &PPU {
        &self.ppu
    }

    pub fn ppu_mut(&mut self) -> &mut PPU {
        &mut self.ppu
    }

    pub fn mapper(&self) -> &dyn Mapper {
        self.mapper.as_ref()
    }

    pub fn mapper_mut(&mut self) -> &mut dyn Mapper {
        self.mapper.as_mut()
    }

    fn read_prg_rom(&mut self, addr: u16) -> u8 {
        self.mapper.read_prg(addr)
    }

    /// Write to the owned PRG-ROM copy. ROM is normally read-only on real
    /// hardware, but the emulator owns its own copy and the test harness
    /// (`CPU::load`) writes a program there before running; we permit it.
    fn write_prg_rom(&mut self, addr: u16, data: u8) {
        self.mapper.write_prg(addr, data);
    }

    /// Read from $4016 (controller 1) or $4017 (controller 2).
    /// Returns the current bit (0 or 1) and shifts the register for the next read.
    fn read_joypad(&mut self, controller: usize) -> u8 {
        let (shift, joypad) = if controller == 0 {
            (&mut self.joypad1_shift, self.joypad1)
        } else {
            (&mut self.joypad2_shift, self.joypad2)
        };

        if self.joypad_strobe {
            // While strobe is high, keep returning bit 0
            (joypad & 1) as u8
        } else {
            // Return current bit and shift for next read
            let bit = *shift & 1;
            *shift >>= 1;
            bit
        }
    }

    /// Write to $4016 (joypad strobe).
    /// When bit 0 transitions from 1 to 0, latch the current joypad state into shift registers.
    fn write_joypad_strobe(&mut self, data: u8) {
        let strobe = (data & 1) != 0;
        if self.joypad_strobe && !strobe {
            // Falling edge: latch joypad state
            self.joypad1_shift = self.joypad1;
            self.joypad2_shift = self.joypad2;
        }
        self.joypad_strobe = strobe;
    }

    /// Update joypad 1 state from SDL input (called from main loop)
    pub fn set_joypad1(&mut self, state: u8) {
        self.joypad1 = state;
    }

    /// Update joypad 2 state from SDL input
    pub fn set_joypad2(&mut self, state: u8) {
        self.joypad2 = state;
    }
}

impl Mem for BUS {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            }
            PPU_REGISTERS..=PPU_REGISTERS_MIRRORS_END => {
                let register = (addr - PPU_REGISTERS) & 0b0000_0000_0000_0111;
                match register {
                    2 => self.ppu.read_from_status(),
                    4 => self.ppu.read_from_oam_data(),
                    7 => self.ppu.read_from_data(),
                    _ => 0, // 0/1/3/5/6 are write-only
                }
            }
            0x4000..=0x4013 => 0,           // APU status / channels (stub)
            0x4015 => 0,                    // APU status read (stub)
            0x4016 => self.read_joypad(0),
            0x4017 => self.read_joypad(1),
            0x4018..=0xFFFF => self.read_prg_rom(addr),

            _ => {
                println!("Ignoring mem access at {}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b11111111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            }
            PPU_REGISTERS..=PPU_REGISTERS_MIRRORS_END => {
                let register = (addr - PPU_REGISTERS) & 0b0000_0000_0000_0111;
                match register {
                    0 => self.ppu.write_to_control(data),
                    1 => self.ppu.write_to_mask(data),
                    3 => self.ppu.write_to_oam_addr(data),
                    4 => self.ppu.write_to_oam_data(data),
                    5 => self.ppu.write_to_scroll(data),
                    6 => self.ppu.write_to_address(data),
                    7 => self.ppu.write_to_data(data),
                    _ => {} // 2 (status) is read-only
                }
            }
            0x4000..=0x4013 => {}          // APU channel regs (stub)
            0x4014 => {
                // OAM DMA: copy 256 bytes from `data * 256` into OAM.
                let page = (data as u16) << 8;
                let mut buf = [0u8; 256];
                for i in 0..256 {
                    buf[i] = self.mem_read(page + i as u16);
                }
                self.ppu.write_to_oam_dma(&buf);
            }
            0x4015 => {}                    // APU status write (stub)
            0x4016 => self.write_joypad_strobe(data),
            0x4017 => {},                       // $4017 is read-only (controller 2)
            0x4018..=0xFFFF => self.write_prg_rom(addr, data),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::rom::test::test_rom;

    #[test]
    fn test_ppu_register_write_via_bus() {
        let mut bus = BUS::new(test_rom());
        // PPUADDR = 0x2006, write 0x23 then 0x05
        bus.mem_write(0x2006, 0x23);
        bus.mem_write(0x2006, 0x05);
        // PPUDATA = 0x2007, write 0x66
        bus.mem_write(0x2007, 0x66);
        // VRAM address 0x2305 should hold 0x66 (vram_index 0x0305)
        assert_eq!(bus.ppu.vram[0x0305], 0x66);
    }

    #[test]
    fn test_ppu_register_read_via_bus() {
        let mut bus = BUS::new(test_rom());
        // seed vram then read through PPUDATA
        bus.ppu_mut().vram[0x0305] = 0x77;
        bus.mem_write(0x2006, 0x23);
        bus.mem_write(0x2006, 0x05);
        bus.mem_read(0x2007); // buffered read (loads into internal_buffer)
        assert_eq!(bus.mem_read(0x2007), 0x77);
    }

    #[test]
    fn test_oam_dma_via_bus() {
        let mut bus = BUS::new(test_rom());
        // Place data at page 0x02 (0x0200..0x0300)
        for i in 0..256u16 {
            bus.cpu_vram[(0x200 + i) as usize] = (i as u8).wrapping_mul(2);
        }
        // OAMADDR = 0; OAMDMA = 0x02 → copy from 0x0200
        bus.mem_write(0x2003, 0x00);
        bus.mem_write(0x4014, 0x02);
        for i in 0..256usize {
            assert_eq!(bus.ppu.oam_data[i], (i as u8).wrapping_mul(2));
        }
    }

    #[test]
    fn test_prg_rom_write_via_load() {
        // Mirrors what CPU::load does: write program bytes into PRG-ROM space,
        // then read them back. This exercises write_prg_rom.
        let mut bus = BUS::new(test_rom());
        bus.mem_write(0x8600, 0xA9);
        bus.mem_write(0x8601, 0x05);
        assert_eq!(bus.mem_read(0x8600), 0xA9);
        assert_eq!(bus.mem_read(0x8601), 0x05);
    }
}
