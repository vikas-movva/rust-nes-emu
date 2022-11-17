mod Memory;
// cpu emulating the 2A03 chip
pub struct CPU{
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub program_counter: u16,
    pub status_register: u8,
    memory: [u8; 0xFFFF],
}

impl Memory::Memory for CPU{
    fn m_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }
    fn m_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: 0,
            program_counter: 0,
            status_register: 0,
            memory: [0; 0xFFFF],
        }
    }
    pub fn interpret(&mut self, program: Vec<u8>) {
        // interpret the instructions
        loop{
            let instruction = program[self.program_counter as usize];
            self.program_counter += 1;
            match instruction {
                0xA9 => self.lda(program[self.program_counter as usize]),
                0xAA => self.tax(),
                0xE8 => self.inx(),
                0x00 => {
                    return;
                },
                _ => {
                    println!("Unknown instruction: {}", instruction);
                    break;
                }
            }
        }
    }
    // opcodes -----------------------------------------------
    //0xA9 LDA Immediate - Load the accumulator with a constant
    fn lda(&mut self, value: u8) {
        self.register_a = value;
        self.set_zero_flag(self.register_a);
        self.set_negative_flag(self.register_a);
        self.program_counter += 1;
    }
    //0xAA TAX - Transfer the accumulator to the X register
    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.set_zero_flag(self.register_x);
        self.set_negative_flag(self.register_x);
    }
    //0xE8 INX - Increment the X register by one
    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.set_zero_flag(self.register_x);
        self.set_negative_flag(self.register_x);
        self.set_overflow_flag(self.register_x);
    }




    // set status register flags
    fn set_zero_flag(&mut self, value: u8) {
        if value == 0 {
            self.status_register |= 0b0000_0010;
        } else {
            self.status_register &= 0b1111_1101;
        }
    }
    fn set_negative_flag(&mut self, value: u8) {
        if value & 0b1000_0000 != 0 {
            self.status_register |= 0b1000_0000;
        } else {
            self.status_register &= 0b0111_1111;
        }
    }
    fn set_carry_flag(&mut self, value: u8) {
        if value & 0b0000_0001 == 0b0000_0001 {
            self.status_register |= 0b0000_0001;
        } else {
            self.status_register &= 0b1111_1110;
        }
    }
    fn set_overflow_flag(&mut self, value: u8) {
        if value & 0b0100_0000 == 0b0100_0000 {
            self.status_register |= 0b0100_0000;
        } else {
            self.status_register &= 0b1011_1111;
        }
    }

}


#[cfg(test)]
mod cpu_test{
    use super::*;
    #[test]
    fn test_lda_immediate(){
        let mut cpu = CPU::new();
        let program = vec![0xA9, 0x05, 0x00];
        cpu.interpret(program);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status_register & 0b0000_0010 == 0b00);
        assert!(cpu.status_register & 0b1000_0000 == 0);
    }
    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();
        cpu.register_a = 10;
        cpu.interpret(vec![0xaa, 0x00]);
    
        assert_eq!(cpu.register_x, 10)
    }
    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);
    
        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.register_x = 0xff;
        cpu.interpret(vec![0xe8, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }
}