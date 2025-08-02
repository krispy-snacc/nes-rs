use crate::bus::Bus;
use crate::instructions::Instruction;
use bitflags::bitflags;
use std::{cell::RefCell, rc::Rc};

bitflags! {
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
    flags: StatusFlags,

    // Registers
    a: u8,      // Accumulator Register
    x: u8,      // X Register
    y: u8,      // Y Register
    stkp: u8,   // Stack Pointer (points to location on bus)
    pc: u16,    // Program Counter
    status: u8, // Status Register

    fetched: u8,
    addr_abs: u16,
    addr_rel: u16,
    opcode: u8,
    cycles: u8,

    lookup: Vec<Instruction>,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            bus: None,
            flags: StatusFlags::all(),

            a: 0x00,
            x: 0x00,
            y: 0x00,
            stkp: 0x00,
            pc: 0x0000,
            status: 0x00,

            fetched: 0x00,
            addr_abs: 0x0000,
            addr_rel: 0x0000,
            opcode: 0x00,
            cycles: 0,

            lookup: vec![],
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

    pub fn get_flag(&self, f: StatusFlags) -> u8 {
        if self.flags.contains(f) { 1 } else { 0 }
    }

    pub fn set_flag(&mut self, f: StatusFlags, v: bool) {
        if v {
            self.flags.insert(f);
        } else {
            self.flags.remove(f);
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
    pub fn reset() {}
    pub fn irq() {}
    pub fn nmi() {}

    fn fetch() {}
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
    pub fn lda(&mut self) {}

    // TODO: add other OpCodes...
}
