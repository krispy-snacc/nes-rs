use std::{cell::RefCell, rc::Rc};

use crate::cpu::Cpu;

pub struct Bus {
    cpu: Cpu,
    ram: Vec<u8>,
}

impl Bus {
    pub fn new() -> Rc<RefCell<Self>> {
        let bus = Rc::new(RefCell::new(Bus {
            cpu: Cpu::new(),
            ram: vec![0x00; 64 * 1024],
        }));
        bus.borrow_mut().cpu.connect_bus(Rc::clone(&bus));
        bus
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize] = data;
    }

    pub fn read(&self, addr: u16, b_read_only: bool) -> u8 {
        self.ram[addr as usize]
    }
}
