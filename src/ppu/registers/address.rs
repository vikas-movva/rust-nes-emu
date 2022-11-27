pub struct AddressRegister{
    value: (u8, u8),
    hi_ptr: bool,
}

impl AddressRegister{
    pub fn new() -> AddressRegister{
        AddressRegister{
            value: (0, 0),
            hi_ptr: true,
        }
    }

    pub fn update(&mut self, value: u8){
        if self.hi_ptr{
            self.value.0 = value;
        }else{
            self.value.1 = value;
        }

        if self.get() > 0x3FFF{
            self.set(self.get() & 0x3FFF);
        }

        self.hi_ptr = !self.hi_ptr;
    }

    fn set(&mut self, data: u16){
        self.value.0 = (data >> 8) as u8;
        self.value.1 = (data & 0xff) as u8;
    }

    pub fn get(&self) -> u16{
        ((self.value.0 as u16) << 8) | (self.value.1 as u16)
    }

    pub fn increment(&mut self, value: u8){
        let l = self.value.1;
        self.value.1 = self.value.1.wrapping_add(value);

        if l > self.value.1{
            self.value.0 = self.value.0.wrapping_add(1);
        }

        if self.get() > 0x3FFF{
            self.set(self.get() & 0x3FFF);
        }
    }

    pub fn reset_latch(&mut self){
        self.hi_ptr = true;
    }
}