use crate::opcodes;
use std::collections::HashMap;

bitflags! {
    /// Status Register (P)
    /// 
    ///7 6 5 4 3 2 1 0
    ///N V _ B D I Z C
    ///| |   | | | | |
    ///| |   |   | | | +--Carry
    ///| |   |   | | +----Zero
    ///| |   |   | +------Interrupt Disable
    ///| |   |   +--------Decimal
    ///| |    +------------No CPU effect, see: the B flag
    ///| +----------------Overflow
    ///+------------------Negative
    ///
    pub struct CpuFlags: u8 {
        const CARRY = 0b0000_0001;
        const ZERO = 0b0000_0010;
        const INTERRUPT_DISABLE = 0b0000_0100;
        const DECIMAL = 0b0000_1000;
        const BREAK = 0b0001_0000;
        const UNUSED = 0b0010_0000;
        const OVERFLOW = 0b0100_0000;
        const NEGATIVE = 0b1000_0000;
    }
}


//Memory Trait
trait Memory{
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

// Implementing the Memory Trait for the CPU
impl Memory for CPU{
    fn m_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }
    fn m_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}
// CPU emulating the 2A03 chip
pub struct CPU{
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub program_counter: u16,
    pub status_register: u8,
    memory: [u8; 0xFFFF],
}

#[derive(Debug)]
pub enum AddressingMode{
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    NoneAddressing,
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

    fn get_op_addr(&self, mode: AddressingMode) -> u16 {
        match mode{
            // counter address
            AddressingMode::Immediate => self.program_counter, 
            
            // zero page address
            AddressingMode::ZeroPage => self.m_read(self.program_counter) as u16, 
            
            // absolute address
            AddressingMode::Absolute => self.m_read_u16(self.program_counter), 

            // zero page address + register x
            AddressingMode::ZeroPageX => {
                let addr = self.m_read(self.program_counter);
                (addr.wrapping_add(self.register_x)) as u16
            },

            // zero page address + register y
            AddressingMode::ZeroPageY => {
                let addr = self.m_read(self.program_counter);
                (addr.wrapping_add(self.register_y)) as u16
            },

            // absolute address + register x
            AddressingMode::AbsoluteX => {
                let addr = self.m_read_u16(self.program_counter);
                addr.wrapping_add(self.register_x as u16)
            },

            // absolute address + register y
            AddressingMode::AbsoluteY => {
                let addr = self.m_read_u16(self.program_counter);
                addr.wrapping_add(self.register_y as u16)
            },

            // indirect address + register x
            AddressingMode::IndirectX => {
                let addr = self.m_read(self.program_counter);
                let p: u8 = addr.wrapping_add(self.register_x);
                let l = self.m_read(p as u16);
                let h = self.m_read((p as u16).wrapping_add(1));
                (h as u16) << 8 | l as u16
            },
            AddressingMode::IndirectY => {
                let addr = self.m_read(self.program_counter);
                let l = self.m_read(addr as u16);
                let h = self.m_read((addr as u8).wrapping_add(1) as u16);
                let deref = (h as u16) << 8 | l as u16;
                deref.wrapping_add(self.register_y as u16)
            },

            AddressingMode::NoneAddressing => panic!("Invalid addressing mode, {:?} not supported", mode),
        }
    }

    pub fn reset(&mut self) {
        // reset registers
        self.register_a = 0;
        self.register_x = 0;
        self.status_register = 0;
        // get start address from 0xFFFC
        self.program_counter = self.m_read_u16(0xFFFC);
    }
    
    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].clone_from_slice(&program[..]);
        self.m_write_u16(0xFFFC, 0x8000);
    }

    pub fn run(&mut self) {
        loop {
            let opcode = self.m_read(self.program_counter);
            self.program_counter += 1;
            match opcode {
                0x00 => break,
                _ => panic!("Unknown opcode: {:02X}", opcode),
            }
        }
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }


    // opcodes -----------------------------------------------
    //0xA9 LDA Immediate - Load the accumulator with a constant
    fn lda(&mut self, mode: AddressingMode) {
        let addr = self.get_op_addr(mode);
        self.register_a = self.m_read(addr);
        self.set_zero_flag(self.register_a);
        self.set_negative_flag(self.register_a);
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
        cpu.load_and_run(program);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status_register & 0b0000_0010 == 0b00);
        assert!(cpu.status_register & 0b1000_0000 == 0);
    }
    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();
        cpu.register_a = 10;
        cpu.load_and_run(vec![0xaa, 0x00]);
    
        assert_eq!(cpu.register_x, 10)
    }
    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);
    
        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.register_x = 0xff;
        cpu.load_and_run(vec![0xe8, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }
}