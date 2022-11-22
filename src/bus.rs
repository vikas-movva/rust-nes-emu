use crate::memory::Memory;

pub struct BUS{
cpu_vram: [u8; 2048],
}

impl BUS{
    pub fn new() -> BUS{
        BUS{
            cpu_vram: [0; 2048],
        }
    }
}

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;


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
            _ => {
                println!("Unimplemented memory write at address: {:04X}", addr);
            }
        }
    }
}
