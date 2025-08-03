use crate::instructions::Instruction;
use crate::{bus::Bus, instructions::LOOKUP};
use bitflags::bitflags;
use std::ops::Not;
use std::{cell::RefCell, rc::Rc};

bitflags! {
    #[derive(Clone, Copy)]
    pub struct StatusFlags: u8 {
        const CARRY             = (1 << 0);
        const ZERO              = (1 << 1);
        const INTERRUPT_DISABLE = (1 << 2);
        const DECIMAL_MODE      = (1 << 3);
        const BREAK             = (1 << 4);
        const UNUSED            = (1 << 5);
        const OVERFLOW          = (1 << 6);
        const NEGATIVE          = (1 << 7);
    }
}

pub struct Cpu {
    bus: Option<Rc<RefCell<Bus>>>,

    // Registers
    a: u8,               // Accumulator Register
    x: u8,               // X Register
    y: u8,               // Y Register
    stkp: u8,            // Stack Pointer (points to location on bus)
    pc: u16,             // Program Counter
    status: StatusFlags, // Status Register

    fetched: u8,
    addr_abs: u16,
    addr_rel: u16,
    opcode: u8,
    cycles: u8,

    lookup: [Instruction; 256],
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            bus: None,
            status: StatusFlags::all(),

            a: 0x00,
            x: 0x00,
            y: 0x00,
            stkp: 0x00,
            pc: 0x0000,

            fetched: 0x00,
            addr_abs: 0x0000,
            addr_rel: 0x0000,
            opcode: 0x00,
            cycles: 0,

            lookup: LOOKUP,
        }
    }

    pub fn connect_bus(&mut self, bus: Rc<RefCell<Bus>>) {
        self.bus = Some(bus);
    }

    pub fn read(&self, a: u16) -> u8 {
        let bus_ref = self.bus.as_ref().unwrap();
        let bus = bus_ref.borrow();
        bus.read(a, false)
    }

    pub fn write(&mut self, a: u16, d: u8) {
        if let Some(bus_ref) = self.bus.as_mut() {
            let mut bus = bus_ref.borrow_mut();
            bus.write(a, d);
        }
    }

    pub fn get_flag(&self, f: StatusFlags) -> bool {
        self.status.contains(f)
    }

    pub fn set_flag(&mut self, f: StatusFlags, v: bool) {
        if v {
            self.status.insert(f);
        } else {
            self.status.remove(f);
        }
    }

    pub fn clock(&mut self) {
        if self.cycles == 0 {
            self.opcode = self.read(self.pc);
            self.pc += 1;

            self.cycles = self.lookup[self.opcode as usize].cycles;
            let add_cycles1 = self.get_operand_address(self.lookup[self.opcode as usize].mode);
            let add_cycles2 = 0 as u8; // additional cycles for operation
            self.cycles += add_cycles1 & add_cycles2;
        }
        self.cycles -= 1;
    }

    pub fn reset(&mut self) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.stkp = 0xFD;
        self.status = StatusFlags::empty();

        self.addr_abs = 0xFFFC;
        let lo = self.read(self.addr_abs + 0) as u16;
        let hi = self.read(self.addr_abs + 1) as u16;

        self.pc = (hi << 8) | lo;

        self.addr_rel = 0x0000;
        self.addr_abs = 0x0000;
        self.fetched = 0x00;

        self.cycles = 8;
    }

    pub fn irq(&mut self) {
        if self.get_flag(StatusFlags::INTERRUPT_DISABLE) == false {
            self.write(0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
            self.stkp -= 1;
            self.write(0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
            self.stkp -= 1;

            self.set_flag(StatusFlags::BREAK, false);
            self.set_flag(StatusFlags::UNUSED, true);
            self.set_flag(StatusFlags::INTERRUPT_DISABLE, true);
            self.write(0x0100 + self.stkp as u16, self.status.bits());
            self.stkp -= 1;

            self.addr_abs = 0xFFFE;
            let lo = self.read(self.addr_abs + 0) as u16;
            let hi = self.read(self.addr_abs + 1) as u16;
            self.pc = (hi << 8) | lo;

            self.cycles = 7;
        }
    }

    pub fn nmi(&mut self) {
        self.write(0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
        self.stkp -= 1;
        self.write(0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
        self.stkp -= 1;

        self.set_flag(StatusFlags::BREAK, false);
        self.set_flag(StatusFlags::UNUSED, true);
        self.set_flag(StatusFlags::INTERRUPT_DISABLE, true);
        self.write(0x0100 + self.stkp as u16, self.status.bits());
        self.stkp -= 1;

        self.addr_abs = 0xFFFA;
        let lo = self.read(self.addr_abs + 0) as u16;
        let hi = self.read(self.addr_abs + 1) as u16;
        self.pc = (hi << 8) | lo;

        self.cycles = 8;
    }

    fn fetch(&mut self) -> u8 {
        if !matches!(
            self.lookup[self.opcode as usize].mode,
            AddressingMode::Implied
        ) {
            self.fetched = self.read(self.addr_abs);
        }
        self.fetched
    }
}

#[derive(Clone, Copy)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Accumulator,
    Implied,
    Relative,
}

// Addressing modes
impl Cpu {
    #[rustfmt::skip]
    pub fn get_operand_address(&mut self, mode: AddressingMode) -> u8 {
        match mode {
            AddressingMode::Immediate     => self.addr_imm(),
            AddressingMode::ZeroPage      => self.addr_zp0(),
            AddressingMode::ZeroPageX     => self.addr_zpx(),
            AddressingMode::ZeroPageY     => self.addr_zpy(),
            AddressingMode::Absolute      => self.addr_abs(),
            AddressingMode::AbsoluteX     => self.addr_abx(),
            AddressingMode::AbsoluteY     => self.addr_aby(),
            AddressingMode::Indirect      => self.addr_ind(),
            AddressingMode::IndirectX     => self.addr_izx(),
            AddressingMode::IndirectY     => self.addr_izy(),
            AddressingMode::Relative      => self.addr_rel(),
            AddressingMode::Accumulator   => 0, // special case — doesn't use memory
            AddressingMode::Implied       => 0, // also special — operand implied
        }

    }

    pub fn addr_imp(&mut self) -> u8 {
        self.fetched = self.a;
        0
    }

    pub fn addr_imm(&mut self) -> u8 {
        self.pc += 1;
        self.addr_abs = self.pc;
        0
    }

    pub fn addr_zp0(&mut self) -> u8 {
        self.addr_abs = self.read(self.pc) as u16;
        self.pc += 1;
        self.addr_abs &= 0x00FF;
        0
    }

    pub fn addr_zpx(&mut self) -> u8 {
        self.addr_abs = (self.read(self.pc) + self.x) as u16;
        self.pc += 1;
        self.addr_abs &= 0x00FF;
        0
    }

    pub fn addr_zpy(&mut self) -> u8 {
        self.addr_abs = (self.read(self.pc) + self.y) as u16;
        self.pc += 1;
        self.addr_abs &= 0x00FF;
        0
    }

    pub fn addr_rel(&mut self) -> u8 {
        self.addr_rel = self.read(self.pc) as u16;
        self.pc += 1;
        if self.addr_rel & 0x80 != 0 {
            self.addr_rel |= 0xFF00;
        }
        0
    }

    pub fn addr_abs(&mut self) -> u8 {
        let lo = self.read(self.pc) as u16;
        self.pc += 1;

        let hi = self.read(self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;

        0
    }

    pub fn addr_abx(&mut self) -> u8 {
        let lo = self.read(self.pc) as u16;
        self.pc += 1;
        let hi = self.read(self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;
        self.addr_abs += self.x as u16;

        if (self.addr_abs & 0xFF00) != (hi << 8) {
            1
        } else {
            0
        }
    }

    pub fn addr_aby(&mut self) -> u8 {
        let lo = self.read(self.pc) as u16;
        self.pc += 1;
        let hi = self.read(self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;
        self.addr_abs += self.y as u16;

        if (self.addr_abs & 0xFF00) != (hi << 8) {
            1
        } else {
            0
        }
    }

    pub fn addr_ind(&mut self) -> u8 {
        let ptr_lo = self.read(self.pc) as u16;
        self.pc += 1;
        let ptr_hi = self.read(self.pc) as u16;
        self.pc += 1;

        let ptr = (ptr_hi << 8) | ptr_lo;

        if ptr_lo == 0x00FF {
            self.addr_abs = ((self.read(ptr & 0xFF00) << 8) | self.read(ptr + 0)) as u16;
        } else {
            self.addr_abs = ((self.read(ptr + 1) << 8) | self.read(ptr + 0)) as u16;
        }

        0
    }

    pub fn addr_izx(&mut self) -> u8 {
        let t = self.read(self.pc) as u16;
        self.pc += 1;

        let lo = self.read((t + self.x as u16) & 0x00FF) as u16;
        let hi = self.read((t + self.x as u16 + 1) & 0x00FF) as u16;

        self.addr_abs = (hi << 8) | lo;

        0
    }

    pub fn addr_izy(&mut self) -> u8 {
        let t = self.read(self.pc) as u16;
        self.pc += 1;

        let lo = self.read(t & 0x00FF) as u16;
        let hi = self.read((t + 1) & 0x00FF) as u16;

        self.addr_abs = (hi << 8) | lo;
        self.addr_abs += self.y as u16;

        if (self.addr_abs & 0xFF00) != (hi << 8) {
            1
        } else {
            0
        }
    }
}

// OpCodes
impl Cpu {
    pub fn adc(&mut self) -> u8 {
        self.fetch();
        let temp =
            (self.a as u16) + (self.fetched as u16) + (self.get_flag(StatusFlags::CARRY) as u16);
        self.set_flag(StatusFlags::CARRY, temp > 255);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0);
        self.set_flag(
            StatusFlags::OVERFLOW,
            ((!((self.a as u16) ^ (self.fetched as u16)) & ((self.a as u16) ^ (temp as u16)))
                & 0x0080)
                != 0,
        );
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x80) != 0);
        self.a = (temp & 0x00FF) as u8;
        return 1;
    }

    pub fn sbc(&mut self) -> u8 {
        self.fetch();
        let value = (self.fetched as u16) ^ 0x00FF;

        let temp = (self.a as u16) + value + (self.get_flag(StatusFlags::CARRY) as u16);
        self.set_flag(StatusFlags::CARRY, (temp & 0xFF00) != 0);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0);
        self.set_flag(
            StatusFlags::OVERFLOW,
            ((temp ^ (self.a as u16)) & (temp ^ value) & 0x0080) != 0,
        );
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        self.a = (temp & 0x00FF) as u8;
        return 1;
    }

    pub fn and(&mut self) -> u8 {
        self.fetch();
        self.a = self.a & self.fetched;
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 1;
    }

    pub fn asl(&mut self) -> u8 {
        self.fetch();
        let temp = (self.fetched as u16) << 1;
        self.set_flag(StatusFlags::CARRY, (temp & 0xFF00) > 0);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x80) != 0);
        if matches!(
            self.lookup[self.opcode as usize].mode,
            AddressingMode::Implied
        ) {
            self.a = (temp & 0x00FF) as u8;
        } else {
            self.write(self.addr_abs, (temp & 0x00FF) as u8);
        }
        return 0;
    }

    pub fn bcc(&mut self) -> u8 {
        if self.get_flag(StatusFlags::CARRY) == false {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn bcs(&mut self) -> u8 {
        if self.get_flag(StatusFlags::CARRY) == true {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn beq(&mut self) -> u8 {
        if self.get_flag(StatusFlags::ZERO) == true {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn bit(&mut self) -> u8 {
        self.fetch();
        let temp = self.a & self.fetched;
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.fetched & (1 << 7)) != 0);
        self.set_flag(StatusFlags::OVERFLOW, (self.fetched & (1 << 6)) != 0);
        return 0;
    }

    pub fn bmi(&mut self) -> u8 {
        if self.get_flag(StatusFlags::NEGATIVE) == true {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn bne(&mut self) -> u8 {
        if self.get_flag(StatusFlags::ZERO) == false {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn bpl(&mut self) -> u8 {
        if self.get_flag(StatusFlags::NEGATIVE) == false {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn brk(&mut self) -> u8 {
        self.pc += 1;

        self.set_flag(StatusFlags::INTERRUPT_DISABLE, true);
        self.write(0x0100 + (self.stkp as u16), ((self.pc >> 8) & 0x00FF) as u8);
        self.stkp -= 1;
        self.write(0x0100 + (self.stkp as u16), (self.pc & 0x00FF) as u8);
        self.stkp -= 1;

        self.set_flag(StatusFlags::BREAK, true);
        self.write(0x0100 + (self.stkp as u16), self.status.bits());
        self.stkp -= 1;
        self.set_flag(StatusFlags::BREAK, false);

        self.pc = (self.read(0xFFFE) as u16) | ((self.read(0xFFFF) as u16) << 8);
        return 0;
    }

    pub fn bvc(&mut self) -> u8 {
        if self.get_flag(StatusFlags::OVERFLOW) == false {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn bvs(&mut self) -> u8 {
        if self.get_flag(StatusFlags::OVERFLOW) == true {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }

            self.pc = self.addr_abs;
        }
        return 0;
    }

    pub fn clc(&mut self) -> u8 {
        self.set_flag(StatusFlags::CARRY, false);
        return 0;
    }

    pub fn cld(&mut self) -> u8 {
        self.set_flag(StatusFlags::DECIMAL_MODE, false);
        return 0;
    }

    pub fn cli(&mut self) -> u8 {
        self.set_flag(StatusFlags::INTERRUPT_DISABLE, false);
        return 0;
    }

    pub fn clv(&mut self) -> u8 {
        self.set_flag(StatusFlags::OVERFLOW, false);
        return 0;
    }

    pub fn cmp(&mut self) -> u8 {
        self.fetch();
        let temp = (self.a as u16) - (self.fetched as u16);
        self.set_flag(StatusFlags::CARRY, self.a >= self.fetched);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        return 1;
    }

    pub fn cpx(&mut self) -> u8 {
        self.fetch();
        let temp = (self.x as u16) - (self.fetched as u16);
        self.set_flag(StatusFlags::CARRY, self.x >= self.fetched);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        return 0;
    }

    pub fn cpy(&mut self) -> u8 {
        self.fetch();
        let temp = (self.y as u16) - (self.fetched as u16);
        self.set_flag(StatusFlags::CARRY, self.y >= self.fetched);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        return 0;
    }

    pub fn dec(&mut self) -> u8 {
        self.fetch();
        let temp = self.fetched - 1;
        self.write(self.addr_abs, (temp & 0x00FF) as u8);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        return 0;
    }

    pub fn dex(&mut self) -> u8 {
        self.x -= 1;
        self.set_flag(StatusFlags::ZERO, self.x == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.x & 0x80) != 0);
        return 0;
    }

    pub fn dey(&mut self) -> u8 {
        self.y -= 1;
        self.set_flag(StatusFlags::ZERO, self.y == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.y & 0x80) != 0);
        return 0;
    }

    pub fn eor(&mut self) -> u8 {
        self.fetch();
        self.a = self.a ^ self.fetched;
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 1;
    }

    pub fn inc(&mut self) -> u8 {
        self.fetch();
        let temp = self.fetched + 1;
        self.write(self.addr_abs, (temp & 0x00FF) as u8);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        return 0;
    }

    pub fn inx(&mut self) -> u8 {
        self.x += 1;
        self.set_flag(StatusFlags::ZERO, self.x == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.x & 0x80) != 0);
        return 0;
    }

    pub fn iny(&mut self) -> u8 {
        self.y += 1;
        self.set_flag(StatusFlags::ZERO, self.y == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.y & 0x80) != 0);
        return 0;
    }

    pub fn jmp(&mut self) -> u8 {
        self.pc = self.addr_abs;
        return 0;
    }

    pub fn jsr(&mut self) -> u8 {
        self.pc -= 1;

        self.write(0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
        self.stkp -= 1;
        self.write(0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
        self.stkp -= 1;

        self.pc = self.addr_abs;
        return 0;
    }

    pub fn lda(&mut self) -> u8 {
        self.fetch();
        self.a = self.fetched;
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 1;
    }

    pub fn ldx(&mut self) -> u8 {
        self.fetch();
        self.x = self.fetched;
        self.set_flag(StatusFlags::ZERO, self.x == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.x & 0x80) != 0);
        return 1;
    }

    pub fn ldy(&mut self) -> u8 {
        self.fetch();
        self.y = self.fetched;
        self.set_flag(StatusFlags::ZERO, self.y == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.y & 0x80) != 0);
        return 1;
    }

    pub fn lsr(&mut self) -> u8 {
        self.fetch();
        self.set_flag(StatusFlags::CARRY, (self.fetched & 0x0001) != 0);
        let temp = self.fetched >> 1;
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        if matches!(
            self.lookup[self.opcode as usize].mode,
            AddressingMode::Implied
        ) {
            self.a = temp & 0x00FF;
        } else {
            self.write(self.addr_abs, temp & 0x00FF);
        }
        return 0;
    }

    pub fn nop(&mut self) -> u8 {
        match self.opcode {
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => 1,
            _ => 0,
        }
    }

    pub fn ora(&mut self) -> u8 {
        self.fetch();
        self.a = self.a | self.fetched;
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 1;
    }

    pub fn pha(&mut self) -> u8 {
        self.write(0x0100 + self.stkp as u16, self.a);
        self.stkp -= 1;
        return 0;
    }

    pub fn php(&mut self) -> u8 {
        self.write(
            0x0100 + self.stkp as u16,
            self.status
                .clone()
                .union(StatusFlags::BREAK)
                .union(StatusFlags::UNUSED)
                .bits(),
        );
        self.set_flag(StatusFlags::BREAK, false);
        self.set_flag(StatusFlags::UNUSED, false);
        self.stkp -= 1;
        return 0;
    }

    pub fn pla(&mut self) -> u8 {
        self.stkp += 1;
        self.a = self.read(0x0100 + self.stkp as u16);
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 0;
    }

    pub fn plp(&mut self) -> u8 {
        self.stkp += 1;
        self.status = StatusFlags::from_bits_retain(self.read(0x0100 + self.stkp as u16));
        self.set_flag(StatusFlags::UNUSED, true);
        return 0;
    }

    pub fn rol(&mut self) -> u8 {
        self.fetch();
        let temp = ((self.fetched as u16) << 1) | (self.get_flag(StatusFlags::CARRY) as u16);
        self.set_flag(StatusFlags::CARRY, (temp & 0xFF00) != 0);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        if matches!(
            self.lookup[self.opcode as usize].mode,
            AddressingMode::Implied
        ) {
            self.a = (temp & 0x00FF) as u8;
        } else {
            self.write(self.addr_abs, (temp & 0x00FF) as u8);
        }
        return 0;
    }

    pub fn ror(&mut self) -> u8 {
        self.fetch();
        let temp = ((self.get_flag(StatusFlags::CARRY) as u16) << 7) | ((self.fetched as u16) >> 1);
        self.set_flag(StatusFlags::CARRY, (self.fetched & 0x01) != 0);
        self.set_flag(StatusFlags::ZERO, (temp & 0x00FF) == 0x0000);
        self.set_flag(StatusFlags::NEGATIVE, (temp & 0x0080) != 0);
        if matches!(
            self.lookup[self.opcode as usize].mode,
            AddressingMode::Implied
        ) {
            self.a = (temp & 0x00FF) as u8;
        } else {
            self.write(self.addr_abs, (temp & 0x00FF) as u8);
        }
        return 0;
    }

    pub fn rti(&mut self) -> u8 {
        self.stkp += 1;
        self.status = StatusFlags::from_bits_retain(self.read(0x0100 + self.stkp as u16));
        self.status &= StatusFlags::BREAK.not();
        self.status &= StatusFlags::UNUSED.not();

        self.stkp += 1;
        self.pc = self.read(0x0100 + self.stkp as u16) as u16;
        self.stkp += 1;
        self.pc |= (self.read(0x0100 + self.stkp as u16) as u16) << 8;
        return 0;
    }

    pub fn rts(&mut self) -> u8 {
        self.stkp += 1;
        self.pc = self.read(0x0100 + self.stkp as u16) as u16;
        self.stkp += 1;
        self.pc |= (self.read(0x0100 + self.stkp as u16) as u16) << 8;
        self.pc += 1;
        return 0;
    }

    pub fn sec(&mut self) -> u8 {
        self.set_flag(StatusFlags::CARRY, true);
        return 0;
    }

    pub fn sed(&mut self) -> u8 {
        self.set_flag(StatusFlags::DECIMAL_MODE, true);
        return 0;
    }

    pub fn sei(&mut self) -> u8 {
        self.set_flag(StatusFlags::INTERRUPT_DISABLE, true);
        return 0;
    }

    pub fn sta(&mut self) -> u8 {
        self.write(self.addr_abs, self.a);
        return 0;
    }

    pub fn stx(&mut self) -> u8 {
        self.write(self.addr_abs, self.x);
        return 0;
    }

    pub fn sty(&mut self) -> u8 {
        self.write(self.addr_abs, self.y);
        return 0;
    }

    pub fn tax(&mut self) -> u8 {
        self.x = self.a;
        self.set_flag(StatusFlags::ZERO, self.x == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.x & 0x80) != 0);
        return 0;
    }

    pub fn tay(&mut self) -> u8 {
        self.y = self.a;
        self.set_flag(StatusFlags::ZERO, self.y == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.y & 0x80) != 0);
        return 0;
    }

    pub fn tsx(&mut self) -> u8 {
        self.x = self.stkp;
        self.set_flag(StatusFlags::ZERO, self.x == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.x & 0x80) != 0);
        return 0;
    }

    pub fn txa(&mut self) -> u8 {
        self.a = self.x;
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 0;
    }

    pub fn txs(&mut self) -> u8 {
        self.stkp = self.x;
        return 0;
    }

    pub fn tya(&mut self) -> u8 {
        self.a = self.y;
        self.set_flag(StatusFlags::ZERO, self.a == 0x00);
        self.set_flag(StatusFlags::NEGATIVE, (self.a & 0x80) != 0);
        return 0;
    }

    pub fn xxx(&mut self) -> u8 {
        return 0;
    }
}
