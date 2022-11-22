use core::panic;

use crate::memory::Memory;
use crate::rom::ROM;

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

pub struct BUS{
    cpu_vram: [u8; 2048],
    rom: ROM,
}

impl BUS{
    pub fn new(rom: ROM) -> BUS{
        BUS{
            cpu_vram: [0; 2048],
            rom: rom,
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8{
        addr -= 0x8000;
        if self.rom.prg_rom.len() == 0x4000 && addr >= 0x4000{
            addr = addr % 0x4000;
        }
        self.rom.prg_rom[addr as usize]
    }
}



impl Memory for BUS{
    fn m_read(&self, addr: u16) -> u8 {
        match addr{
            RAM ..= RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            }
            
            PPU_REGISTERS ..= PPU_REGISTERS_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00000000_00000111;
                todo!("PPU registers not implemented yet")
            }
            
            0x8000 ..= 0xFFFF => {
                self.read_prg_rom(addr)
            }

            _ => {
                println!("Unimplemented memory read at address: {:04X}", addr);
                0
            }
    
        }
    }

    fn m_write(&mut self, addr: u16, data: u8) {
        match addr{
            RAM ..= RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b11111111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            }
            PPU_REGISTERS ..= PPU_REGISTERS_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                todo!("PPU registers not implemented yet")
            }
            0x8000 ..= 0xFFFF => {
                panic!("Attempted to write to ROM space");
            }
            _ => {
                println!("Unimplemented memory write at address: {:04X}", addr);
            }
        }
    }
}