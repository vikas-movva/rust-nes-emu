use crate::opcodes;
use crate::memory::Memory;
use crate::bus::BUS;

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
        const BREAK2 = 0b0010_0000;
        const OVERFLOW = 0b0100_0000;
        const NEGATIVE = 0b1000_0000;
    }
}

// Implementing the Memory Trait for the CPU
impl Memory for CPU{

    fn m_read(&self, addr: u16) -> u8 {
        self.bus.m_read(addr)
    }

    fn m_write(&mut self, addr: u16, data: u8) {
        self.bus.m_write(addr, data)
    }

    fn m_read_u16(&self, addr: u16) -> u16 {
        self.bus.m_read_u16(addr)
    }

    fn m_write_u16(&mut self, addr: u16, data: u16) {
        self.bus.m_write_u16(addr, data)
    }

}

// Stack constants
const STACK_OFFSET: u16 = 0x0100;
const STACK_RESET: u8 = 0xFD;

// CPU emulating the 2A03 chip
pub struct CPU{
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub program_counter: u16,
    pub status_register: CpuFlags,
    pub bus: BUS,
}

#[derive(Debug)]
pub enum AddressingMode{
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    Indirect,
    NoneAddressing,
}


impl CPU {
    pub fn new() -> CPU {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: STACK_RESET,  
            program_counter: 0,
            status_register: CpuFlags::from_bits_truncate(0b100100),
            bus: BUS::new(),
        }
    }

    fn get_op_addr(&self, mode: &AddressingMode) -> u16 {

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

            _ => panic!("Invalid addressing mode, {:?} not supported", mode),
        }
    }

    pub fn reset(&mut self) {
        // reset registers
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.stack_pointer = STACK_RESET;
        self.status_register = CpuFlags::from_bits_truncate(0b100100);
        // get start address from 0xFFFC
        self.program_counter = self.m_read_u16(0xFFFC);
    }
    
    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x6000..(0x6000 + program.len())].clone_from_slice(&program[..]);
        self.m_write_u16(0xFFFC, 0x6000);
    }

    pub fn run(&mut self) {
        self.run_callback(|_| {});
    }

    pub fn run_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut CPU),
    {
        let ref opcodes: HashMap<u8, &'static opcodes::Opcode> = *opcodes::OPCODES_MAP;

        loop{
            let code = self.m_read(self.program_counter);
            self.program_counter += 1;
            let pc_state = self.program_counter;
            let opcode = opcodes.get(&code).unwrap();

            match code {
                0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => {
                    self.adc(&opcode.mode);
                }
                0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => {
                    self.and(&opcode.mode);
                }
                0x0A | 0x06 | 0x16 | 0x0E | 0x1E => {
                    self.asl(&opcode.mode);
                }
                0x90 => {
                    self.bcc();
                }
                0xB0 => {
                    self.bcs();
                }
                0xF0 => {
                    self.beq();
                }
                0x24 | 0x2C => {
                    self.bit(&opcode.mode);
                }
                0x30 => {
                    self.bmi();
                }
                0xD0 => {
                    self.bne();
                }
                0x10 => {
                    self.bpl();
                }
                0x00 => {
                    return;
                }
                0x50 => {
                    self.bvc();
                }
                0x70 => {
                    self.bvs();
                }
                0x18 => {
                    self.clc();
                }
                0xD8 => {
                    self.cld();
                }
                0x58 => {
                    self.cli();
                }
                0xB8 => {
                    self.clv();
                }
                0xC9 | 0xC5 | 0xD5 | 0xCD | 0xDD | 0xD9 | 0xC1 | 0xD1 => {
                    self.cmp(&opcode.mode);
                }
                0xE0 | 0xE4 | 0xEC => {
                    self.cpx(&opcode.mode);
                }
                0xC0 | 0xC4 | 0xCC => {
                    self.cpy(&opcode.mode);
                }
                0xC6 | 0xD6 | 0xCE | 0xDE => {
                    self.dec(&opcode.mode);
                }
                0xCA => {
                    self.dex();
                }
                0x88 => {
                    self.dey();
                }
                0x49 | 0x45 | 0x55 | 0x4D | 0x5D | 0x59 | 0x41 | 0x51 => {
                    self.eor(&opcode.mode);
                }
                0xE6 | 0xF6 | 0xEE | 0xFE => {
                    self.inc(&opcode.mode);
                }
                0xE8 => {
                    self.inx();
                }
                0xC8 => {
                    self.iny();
                }
                0x4C | 0x6C => {
                    self.jmp(&opcode.mode);
                }
                0x20 => {
                    self.jsr();
                }
                0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
                    self.lda(&opcode.mode);
                }
                0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => {
                    self.ldx(&opcode.mode);
                }
                0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => {
                    self.ldy(&opcode.mode);
                }
                0x4A | 0x46 | 0x56 | 0x4E | 0x5E => {
                    self.lsr(&opcode.mode);
                }
                0xEA => {
                    self.nop();
                }
                0x09 | 0x05 | 0x15 | 0x0D | 0x1D | 0x19 | 0x01 | 0x11 => {
                    self.ora(&opcode.mode);
                }
                0x48 => {
                    self.pha();
                }
                0x08 => {
                    self.php();
                }
                0x68 => {
                    self.pla();
                }
                0x28 => {
                    self.plp();
                }
                0x2A | 0x26 | 0x36 | 0x2E | 0x3E => {
                    self.rol(&opcode.mode);
                }
                0x6A | 0x66 | 0x76 | 0x6E | 0x7E => {
                    self.ror(&opcode.mode);
                }
                0x40 => {
                    self.rti();
                }
                0x60 => {
                    self.rts();
                }
                0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xE1 | 0xF1 => {
                    self.sbc(&opcode.mode);
                }
                0x38 => {
                    self.sec();
                }
                0xF8 => {
                    self.sed();
                }
                0x78 => {
                    self.sei();
                }
                0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => {
                    self.sta(&opcode.mode);
                }
                0x86 | 0x96 | 0x8E => {
                    self.stx(&opcode.mode);
                }
                0x84 | 0x94 | 0x8C => {
                    self.sty(&opcode.mode);
                }
                0xAA => {
                    self.tax();
                }
                0xA8 => {
                    self.tay();
                }
                0xBA => {
                    self.tsx();
                }
                0x8A => {
                    self.txa();
                }
                0x9A => {
                    self.txs();
                }
                0x98 => {
                    self.tya();
                }
                _ => {
                    println!("Unknown opcode: {:X}", code);
                    return;
                }
            }

            if pc_state == self.program_counter{
                self.program_counter += (opcode.bytes - 1) as u16;
            }

            callback(self);
        }
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    // Utility functions
    
    // set register a
    fn set_reg_a(&mut self, data: u8) {
        // set register a to data
        self.register_a = data;

        // update zero and negative flags
        self.set_zero_and_negative_flag(self.register_a);
    }
    
    // add data to register a
    fn add_reg_a(&mut self, data: u8){
        // get the sum of the data and the register a plus the carry flag
        let result = self.register_a as u16 + data as u16 + (if self.status_register.contains(CpuFlags::CARRY) {1} else {0}) as u16;
        
        // set the carry flag if the result is greater than 0xFF
        self.set_carry_flag(result > 0xFF);

        // set overflow flag
        self.set_overflow_flag((data ^ result as u8) & (result as u8 ^ self.register_a) & 0x80 != 0);
        
        // set the register a to the result
        self.set_reg_a(result as u8);
    }
    
    // set status register flags
    fn set_zero_and_negative_flag(&mut self, value: u8) {
        self.set_zero_flag(value);
        self.set_negative_flag(value);
    }

    // set zero flag
    fn set_zero_flag(&mut self, value: u8) {
        if value == 0 {
            self.status_register.insert(CpuFlags::ZERO);
        } else {
            self.status_register.remove(CpuFlags::ZERO);
        }
    }

    // set negative flag
    fn set_negative_flag(&mut self, value: u8) {
        if value >> 7 == 1 {
            self.status_register.insert(CpuFlags::NEGATIVE);
        } else {
            self.status_register.remove(CpuFlags::NEGATIVE);
        }
    }
    
    // set carry flag
    fn set_carry_flag(&mut self, value: bool) {
        if value {
            self.status_register.insert(CpuFlags::CARRY);
        } else {
            self.status_register.remove(CpuFlags::CARRY);
        }
    }
    
    // set overflow flag
    fn set_overflow_flag(&mut self, value: bool) {
        if value {
            self.status_register.insert(CpuFlags::OVERFLOW);
        } else {
            self.status_register.remove(CpuFlags::OVERFLOW);
        }
    }
    
    // Branching
    fn branch(&mut self, condition: bool){
        if condition{
            // get the address to branch to
            let addr = self.program_counter.wrapping_add(1).wrapping_add((self.m_read(self.program_counter) as i8) as u16);
            
            // set program counter to the address
            self.program_counter = addr;
        }
    }

    // Stack functions

        // pop from stack
        fn pop_stack(&mut self) -> u8 {
            self.stack_pointer = self.stack_pointer.wrapping_add(1);
            self.m_read(self.stack_pointer as u16)
        }
        
        // pop u16 from stack
        fn pop_stack_u16(&mut self) -> u16 {
            // get the low byte
            let l = self.pop_stack();

            // get the high byte
            let h = self.pop_stack();

            // return the u16
            (h as u16) << 8 | l as u16
        }

        // push data to stack
        fn push_stack(&mut self, data: u8) {
            // write data to the stack
            self.m_write(STACK_OFFSET + self.stack_pointer as u16, data);

            // decrement stack pointer
            self.stack_pointer = self.stack_pointer.wrapping_sub(1);
        }

        // push u16 to stack
        fn push_stack_u16(&mut self, data: u16) {
            // push high byte
            self.push_stack((data >> 8) as u8);
            
            // push low byte
            self.push_stack((data & 0xFF) as u8);
        }

    // compare function
    fn compare(&mut self, mode: &AddressingMode, value: u8) {
        // get the address
        let addr = self.get_op_addr(mode);

        // get the data from the address
        let data = self.m_read(addr);

        // set the carry flag
        self.set_carry_flag(value >= data);

        // set the zero and negative flags
        self.set_zero_and_negative_flag(value.wrapping_sub(data));
    }

     // Opcodes -----------------------------------------------

    // ADC - Add with Carry
    fn adc(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // add the data to the register a
        self.add_reg_a(data);
    }

    // AND - Logical AND
    fn and(&mut self, mode: &AddressingMode){
        
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // set the register a to the result of the logical and
        self.set_reg_a(self.register_a & data);
    }

    // ASL - Arithmetic Shift Left
    fn asl(&mut self, mode: &AddressingMode){
        match mode {
            AddressingMode::Accumulator => {
                // set carry flag
                self.set_carry_flag(self.register_a >> 7 == 1);
                
                // shift register a
                self.set_reg_a(self.register_a << 1);
            }
            _ => {
                // get the address of the operand and read the data
                let addr = self.get_op_addr(mode);
                let data = self.m_read(addr);
                
                // set the carry flag
                self.set_carry_flag(data >> 7 == 1);

                // write the data to the address
                self.m_write(addr, data << 1);

                // set the zero and negative flags
                self.set_zero_and_negative_flag(data << 1);
            }
        }    
    }

    // BCC - Branch if Carry Clear
    fn bcc(&mut self){
        self.branch(!self.status_register.contains(CpuFlags::CARRY));
    }

    // BCS - Branch if Carry Set
    fn bcs(&mut self){
        self.branch(self.status_register.contains(CpuFlags::CARRY));
    }

    // BEQ - Branch if Equal
    fn beq(&mut self){
        self.branch(self.status_register.contains(CpuFlags::ZERO));
    }

    // BIT - Bit Test
    fn bit(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // set the zero flag
        self.set_zero_flag(self.register_a & data);

        // set the negative flag
        self.set_negative_flag(data);

        // set the overflow flag
        self.set_overflow_flag(data & 0x40 != 0);
    }

    // BMI - Branch if Minus
    fn bmi(&mut self){
        self.branch(self.status_register.contains(CpuFlags::NEGATIVE));
    }

    // BNE - Branch if Not Equal
    fn bne(&mut self){
        self.branch(!self.status_register.contains(CpuFlags::ZERO));
    }

    // BPL - Branch if Positive
    fn bpl(&mut self){
        self.branch(!self.status_register.contains(CpuFlags::NEGATIVE));
    }

    // TODO:
    // BRK - Force Interrupt
    // fn brk(&mut self){
    //     // increment program counter
    //     self.program_counter += 1;
    //     // push program counter to stack
    //     self.push_stack((self.program_counter >> 8) as u8);
    //     self.push_stack(self.program_counter as u8);
    //     // set break flag
    //     self.status_register.insert(CpuFlags::BREAK);
    //     // push status register to stack
    //     self.push_stack(self.status_register.bits());
    //     // set interrupt disable flag
    //     self.status_register.insert(CpuFlags::INTERRUPT_DISABLE);
    //     // set program counter to the interrupt vector
    //     self.program_counter = self.m_read(0xFFFE) as u16 | (self.m_read(0xFFFF) as u16) << 8;
    // }

    // BVC - Branch if Overflow Clear
    fn bvc(&mut self){
        self.branch(!self.status_register.contains(CpuFlags::OVERFLOW));
    }

    // BVS - Branch if Overflow Set
    fn bvs(&mut self){
        self.branch(self.status_register.contains(CpuFlags::OVERFLOW));
    }

    // CLC - Clear Carry Flag
    fn clc(&mut self){
        self.status_register.remove(CpuFlags::CARRY);
    }

    // CLD - Clear Decimal Mode
    fn cld(&mut self){
        self.status_register.remove(CpuFlags::DECIMAL);
    }

    // CLI - Clear Interrupt Disable
    fn cli(&mut self){
        self.status_register.remove(CpuFlags::INTERRUPT_DISABLE);
    }

    // CLV - Clear Overflow Flag
    fn clv(&mut self){
        self.status_register.remove(CpuFlags::OVERFLOW);
    }

    // CMP - Compare
    fn cmp(&mut self, mode: &AddressingMode){
        // compare the data with the register a
        self.compare(mode, self.register_a);
    }

    // CPX - Compare X Register
    fn cpx(&mut self, mode: &AddressingMode){
        // compare the data with the register x
        self.compare(mode, self.register_x);
    }

    // CPY - Compare Y Register
    fn cpy(&mut self, mode: &AddressingMode){
        // compare the data with the register y
        self.compare(mode, self.register_y);
    }

    // DEC - Decrement Memory
    fn dec(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // decrement the data
        let data = data.wrapping_sub(1);

        // write the data to the address
        self.m_write(addr, data);

        // set the zero and negative flags
        self.set_zero_and_negative_flag(data);
    }

    // DEX - Decrement X Register
    fn dex(&mut self){
        // decrement the register x
        self.register_x = self.register_x.wrapping_sub(1);
        self.set_zero_and_negative_flag(self.register_x);
    }

    // DEY - Decrement Y Register
    fn dey(&mut self){
        // decrement the register y
        self.register_y = self.register_y.wrapping_sub(1);
        self.set_zero_and_negative_flag(self.register_y);
    }

    // EOR - Exclusive OR
    fn eor(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // exclusive or the data with the register a
        self.set_reg_a(self.register_a ^ data);
    }

    // INC - Increment Memory
    fn inc(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // increment the data
        let data = data.wrapping_add(1);

        // write the data to the address
        self.m_write(addr, data);

        // set the zero and negative flags
        self.set_zero_and_negative_flag(data);
    }

    // INX - Increment X Register
    fn inx(&mut self){
        // increment the register x
        self.register_x = self.register_x.wrapping_add(1);
        self.set_zero_and_negative_flag(self.register_x);
    }

    // INY - Increment Y Register
    fn iny(&mut self){
        // increment the register y
        self.register_y = self.register_y.wrapping_add(1);
        self.set_zero_and_negative_flag(self.register_y);
    }

    // JMP - Jump
    fn jmp(&mut self, mode: &AddressingMode){
        match mode {
            AddressingMode::Absolute => {
                // get the address of the operand
                let addr = self.get_op_addr(mode);
                // set the program counter to the address
                self.program_counter = addr;
            },
            AddressingMode::Indirect => {
                let addr = self.m_read_u16(self.program_counter);
                self.program_counter = if addr & 0x00FF == 0x00FF {
                    // simulate page boundary hardware bug
                    (self.m_read(addr & 0xFF00) as u16) << 8 | self.m_read(addr) as u16
                } else {
                    self.m_read_u16(addr)
                };
            },
            _ => panic!("Invalid addressing mode for JMP instruction"),
        }   
    }

    // JSR - Jump to Subroutine
    fn jsr(&mut self){
        // push program counter to stack
        self.push_stack_u16(self.program_counter - 1);
        // set program counter to the address of the operand
        self.program_counter = self.m_read_u16(self.program_counter);
    }
    
    // LDA - Load Accumulator
    fn lda(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // load the data into the register a
        self.set_reg_a(data);
    }

    // LDX - Load X Register
    fn ldx(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // load the data into the register x
        self.register_x = data;
        self.set_zero_and_negative_flag(self.register_x);
    }

    // LDY - Load Y Register
    fn ldy(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // load the data into the register y
        self.register_y = data;
        self.set_zero_and_negative_flag(self.register_y);
    }

    // LSR - Logical Shift Right
    fn lsr(&mut self, mode: &AddressingMode){
        match mode {
            AddressingMode::Accumulator => {
                // shift the register a
                self.set_carry_flag(self.register_a & 0x01 == 0x01);
                self.set_reg_a(self.register_a >> 1);
            },
            _ => {
                // get the address of the operand and read the data
                let addr = self.get_op_addr(mode);
                let data = self.m_read(addr);

                // set the carry flag
                self.set_carry_flag(data & 0x01 == 0x01);

                // write the data to the address
                self.m_write(addr, data >> 1);

                // set the zero and negative flags
                self.set_zero_and_negative_flag(data >> 1);
            }
        }
    }

    // NOP - No Operation
    fn nop(&mut self){
        // do nothing
    }

    // ORA - Logical Inclusive OR
    fn ora(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // logical inclusive or the data with the register a
        self.set_reg_a(self.register_a | data);
    }

    // PHA - Push Accumulator
    fn pha(&mut self){
        // push the register a to the stack
        self.push_stack(self.register_a);
    }

    // PHP - Push Processor Status
    fn php(&mut self){
        // get the processor status
        let mut status = self.status_register.clone();
        status.insert(CpuFlags::BREAK);
        status.insert(CpuFlags::BREAK2);

        // push the processor status to the stack
        self.push_stack(status.bits());
    }

    // PLA - Pull Accumulator
    fn pla(&mut self){
        let data = self.pop_stack();
        // pull the register a from the stack
        self.set_reg_a(data);
    }

    // PLP - Pull Processor Status
    fn plp(&mut self){
        // pull the processor status from the stack
        self.status_register.bits = self.pop_stack();
        self.status_register.remove(CpuFlags::BREAK);
        self.status_register.insert(CpuFlags::BREAK2);
    }

    // ROL - Rotate Left
    fn rol(&mut self, mode: &AddressingMode){
        match mode {
            AddressingMode::Accumulator => {
                // rotate the register a
                let carry = self.status_register.contains(CpuFlags::CARRY);
                self.set_carry_flag(self.register_a >>7 == 0x01);
                self.set_reg_a(if carry { self.register_a << 1 | 0x01 } else { self.register_a << 1 });
            },
            _ => {
                // get the address of the operand and read the data
                let addr = self.get_op_addr(mode);
                let data = self.m_read(addr);

                // rotate the data
                let carry = self.status_register.contains(CpuFlags::CARRY);
                self.set_carry_flag(data >> 7 == 0x01);

                // write the data to the address
                self.m_write(addr, if carry { data << 1 | 0x01 } else { data << 1 });

                // set the zero and negative flags
                self.set_zero_and_negative_flag(data);
            }
        }
    }

    // ROR - Rotate Right
    fn ror(&mut self, mode: &AddressingMode){
        match mode {
            AddressingMode::Accumulator => {
                // rotate the register a
                let carry = self.status_register.contains(CpuFlags::CARRY);
                self.set_carry_flag(self.register_a & 0x01 == 0x01);
                self.set_reg_a(if carry { self.register_a >> 1 | 0x80 } else { self.register_a >> 1 });
            },
            _ => {
                // get the address of the operand and read the data
                let addr = self.get_op_addr(mode);
                let data = self.m_read(addr);

                // rotate the data
                let carry = self.status_register.contains(CpuFlags::CARRY);
                self.set_carry_flag(data & 0x01 == 0x01);

                // write the data to the address
                self.m_write(addr, if carry { data >> 1 | 0x80 } else { data >> 1 });

                // set the zero and negative flags
                self.set_zero_and_negative_flag(data);
            }
        }
    }

    // RTI - Return from Interrupt
    fn rti(&mut self){
        // pull the processor status from the stack
        self.status_register.bits = self.pop_stack();
        self.status_register.remove(CpuFlags::BREAK);
        self.status_register.insert(CpuFlags::BREAK2);

        // pull the program counter from the stack
        self.program_counter = self.pop_stack_u16();
    }

    // RTS - Return from Subroutine
    fn rts(&mut self){
        // pull the program counter from the stack
        self.program_counter = self.pop_stack_u16() + 1;
    }

    // SBC - Subtract with Carry
    fn sbc(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        self.add_reg_a((data as i8).wrapping_neg().wrapping_sub(1) as u8);
    }

    // SEC - Set Carry Flag
    fn sec(&mut self){
        // set the carry flag
        self.set_carry_flag(true);
    }

    // SED - Set Decimal Flag
    fn sed(&mut self){
        self.status_register.insert(CpuFlags::DECIMAL);
    }

    // SEI - Set Interrupt Disable
    fn sei(&mut self){
        // set the interrupt disable flag
        self.status_register.insert(CpuFlags::INTERRUPT_DISABLE);
    }

    // STA - Store Accumulator
    fn sta(&mut self, mode: &AddressingMode){
        // get the address of the operand and write the data
        let addr = self.get_op_addr(mode);
        self.m_write(addr, self.register_a);
    }

    // STX - Store X Register
    fn stx(&mut self, mode: &AddressingMode){
        // get the address of the operand and write the data
        let addr = self.get_op_addr(mode);
        self.m_write(addr, self.register_x);
    }

    // STY - Store Y Register
    fn sty(&mut self, mode: &AddressingMode){
        // get the address of the operand and write the data
        let addr = self.get_op_addr(mode);
        self.m_write(addr, self.register_y);
    }

    // TAX - Transfer Accumulator to X
    fn tax(&mut self){
        // transfer the register a to the register x
        self.register_x = self.register_a;
        self.set_zero_and_negative_flag(self.register_x);
    }

    // TAY - Transfer Accumulator to Y
    fn tay(&mut self){
        // transfer the register a to the register y
        self.register_y = self.register_a;
        self.set_zero_and_negative_flag(self.register_y);
    }

    // TSX - Transfer Stack Pointer to X
    fn tsx(&mut self){
        // transfer the stack pointer to the register x
        self.register_x = self.stack_pointer;
        self.set_zero_and_negative_flag(self.register_x);
    }

    // TXA - Transfer X to Accumulator
    fn txa(&mut self){
        // transfer the register x to the register a
        self.register_a = self.register_x;
        self.set_zero_and_negative_flag(self.register_a);
    }

    // TXS - Transfer X to Stack Pointer
    fn txs(&mut self){
        // transfer the register x to the stack pointer
        self.stack_pointer = self.register_x;
    }

    // TYA - Transfer Y to Accumulator
    fn tya(&mut self){
        // transfer the register y to the register a
        self.register_a = self.register_y;
        self.set_zero_and_negative_flag(self.register_a);
    }

    // Unofficial Instructions
    #[allow(dead_code)]
    // ANC - AND with Carry
    fn anc(&mut self, mode: &AddressingMode){
        // get the address of the operand and read the data
        let addr = self.get_op_addr(mode);
        let data = self.m_read(addr);

        // and the data with the register a
        self.register_a &= data;

        // set the carry flag
        self.set_carry_flag(self.register_a & 0x80 == 0x80);

        // set the zero and negative flags
        self.set_zero_and_negative_flag(self.register_a);
    }

}

// Test CPU methods
#[cfg(test)]
// TODO: Add tests for all CPU methods
mod test {
    use super::*;

    #[test]
    fn test_0xa9_lda_immidiate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 5);
        assert!(cpu.status_register.bits() & 0b0000_0010 == 0b00);
        assert!(cpu.status_register.bits() & 0b1000_0000 == 0);
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

    #[test]
    fn test_lda_from_memory() {
        let mut cpu = CPU::new();
        cpu.m_write(0x10, 0x55);

        cpu.load_and_run(vec![0xa5, 0x10, 0x00]);

        assert_eq!(cpu.register_a, 0x55);
    }
}