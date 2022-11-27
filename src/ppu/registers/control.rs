bitflags! {

    // 7  bit  0
    // ---- ----
    // VPHB SINN
    // |||| ||||
    // |||| ||++- Base nametable address
    // |||| ||    (0 = $2000; 1 = $2400; 2 = $2800; 3 = $2C00)
    // |||| |+--- VRAM address increment per CPU read/write of PPUDATA
    // |||| |     (0: add 1, going across; 1: add 32, going down)
    // |||| +---- Sprite pattern table address for 8x8 sprites
    // ||||       (0: $0000; 1: $1000; ignored in 8x16 mode)
    // |||+------ Background pattern table address (0: $0000; 1: $1000)
    // ||+------- Sprite size (0: 8x8 pixels; 1: 8x16 pixels)
    // |+-------- PPU master/slave select
    // |          (0: read backdrop from EXT pins; 1: output color on EXT pins)
    // +--------- Generate an NMI at the start of the
    //            vertical blanking interval (0: off; 1: on)
    pub struct ControlRegister: u8 {
        const NAMETABLE1              = 0b00000001;
        const NAMETABLE2              = 0b00000010;
        const VRAM_ADD_INCREMENT      = 0b00000100;
        const SPRITE_PATTERN_ADDR     = 0b00001000;
        const BACKROUND_PATTERN_ADDR  = 0b00010000;
        const SPRITE_SIZE             = 0b00100000;
        const MASTER_SLAVE_SELECT     = 0b01000000;
        const GENERATE_NMI            = 0b10000000;
    }
}

impl ControlRegister{

    pub fn new() -> ControlRegister{
        ControlRegister::from_bits_truncate(0)
    }

    pub fn nametable_addr(&self) -> u16{
        match self.bits & 0b11{
            0 => 0x2000,
            1 => 0x2400,
            2 => 0x2800,
            3 => 0x2C00,
            _ => panic!("Invalid nametable address"),
        }
    }

    pub fn vram_add_increment(&self) -> u8{
        if self.contains(ControlRegister::VRAM_ADD_INCREMENT){
            32
        }else{
            1
        }
    }

    pub fn sprite_pattern_addr(&self) -> u16{
        if self.contains(ControlRegister::SPRITE_PATTERN_ADDR){
            0x1000
        }else{
            0x0000
        }
    }

    pub fn background_pattern_addr(&self) -> u16{
        if self.contains(ControlRegister::BACKROUND_PATTERN_ADDR){
            0x1000
        }else{
            0x0000
        }
    }

    pub fn sprite_size(&self) -> u8{
        if self.contains(ControlRegister::SPRITE_SIZE){
            16
        }else{
            8
        }
    }

    pub fn master_slave_select(&self) -> u8{
        if self.contains(ControlRegister::MASTER_SLAVE_SELECT){
            1 
        }else {
            0
        }
    }

    pub fn generate_nmi(&self) -> bool{
        self.contains(ControlRegister::GENERATE_NMI)
    }

    pub fn update(&mut self, value: u8){
        self.bits = value;
    }
}