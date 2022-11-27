// Scroll register
pub struct ScrollRegister{
    pub scroll_x: u8,
    pub scroll_y: u8,
    pub latch: bool,
}

impl ScrollRegister{
    pub fn new() -> ScrollRegister{
        ScrollRegister{
            scroll_x: 0,
            scroll_y: 0,
            latch: false,
        }
    }

    pub fn write(&mut self, value: u8){
        if !self.latch{
            self.scroll_x = value;
        }else{
            self.scroll_y = value;
        }
        self.latch = !self.latch;
    }

    pub fn reset_latch(&mut self){
        self.latch = false;
    }
    
}