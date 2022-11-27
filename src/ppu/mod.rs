pub mod registers;

use crate::rom::Mirroring;
use registers::control::ControlRegister;
use registers::mask::MaskRegister;
use registers::status::StatusRegister;
use registers::scroll::ScrollRegister;
use registers::address::AddressRegister;


pub struct PPU{
    chr_rom: Vec<u8>,
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

}

impl PPUInterface for PPU{

    fn write_to_control(&mut self, value: u8) {
        let before_nmi_status = self.control.generate_nmi();
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
                self.internal_buffer = self.chr_rom[addr as usize];
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
}