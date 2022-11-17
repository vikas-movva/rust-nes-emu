pub trait Memory{
    fn m_read(&self, addr: u16) -> u8;
    
    fn m_write(&mut self, addr: u16, data: u8);

    fn m_read_u16(&self, addr: u16) -> u16 {
        let low: u16 = self.m_read(addr) as u16;
        let high: u16 = self.m_read(addr + 1) as u16;
        (high << 8) | (low as u16)
    }

    fn m_write_u16(&mut self, addr: u16, data: u16) {
        let low = (data & 0xFF) as u8;
        let high = ((data >> 8) >> 8) as u8;
        self.m_write(addr, low);
        self.m_write(addr + 1, high);
    }
}