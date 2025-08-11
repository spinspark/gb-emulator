use crate::interrupt::{Interrupt, InterruptFlags};

const VIDEO_RAM_SIZE: usize = 8 * 1024;
const OAM_SIZE: usize = 0xFE9F - 0xFE00 + 1;

const MEM_LCDC: u16 = 0xFF40;
const MEM_STAT: u16 = 0xFF41;
const MEM_SCY: u16 = 0xFF42;
const MEM_SCX: u16 = 0xFF43;
const MEM_LY: u16 = 0xFF44;
const MEM_LYC: u16 = 0xFF45;
const MEM_BGP: u16 = 0xFF47;
const MEM_OBP0: u16 = 0xFF48;
const MEM_OBP1: u16 = 0xFF49;
const MEM_WY: u16 = 0xFF4A;
const MEM_WX: u16 = 0xFF4B;

const CYCLES_PER_LINE: usize = 456;

#[derive(Debug, Clone, Copy)]
enum Mode {
    HBlank = 0,
    VBlank = 1,
    Scan = 2,
    Draw = 3,
}

#[derive(Debug, Clone)]
pub struct Ppu {
    // VRAM
    video_ram: [u8; VIDEO_RAM_SIZE],
    // OAM
    attribute_ram: [u8; OAM_SIZE],
    // LCDC
    enabled: bool,
    window_map: bool,
    window_enabled: bool,
    tile_addressing_mode: bool,
    bg_map: bool,
    sprite_size: bool,
    sprite_enabled: bool,
    bg_enabled: bool,
    // STAT
    lyc_intr_select: bool,
    mode2_intr_select: bool,
    mode1_intr_select: bool,
    mode0_intr_select: bool,
    mode: Mode,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    // BGP
    background_palette: u8,
    // OBP0
    sprite_palette_0: u8,
    // OBP1
    sprite_palette_1: u8,
    wy: u8,
    wx: u8,
    cycles: usize,
    dma_running: bool,
}

impl Ppu {
    pub const fn new() -> Self {
        Self {
            video_ram: [0; VIDEO_RAM_SIZE],
            attribute_ram: [0; OAM_SIZE],
            enabled: true,
            window_map: false,
            window_enabled: false,
            tile_addressing_mode: true,
            bg_map: false,
            sprite_size: false,
            sprite_enabled: false,
            bg_enabled: true,
            lyc_intr_select: false,
            mode2_intr_select: false,
            mode1_intr_select: false,
            mode0_intr_select: false,
            mode: Mode::HBlank,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            background_palette: 0xFC,
            sprite_palette_0: 0xFF,
            sprite_palette_1: 0xFF,
            wy: 0,
            wx: 0,
            cycles: 0,
            dma_running: false,
        }
    }

    pub const fn step(&mut self, interrupt_flags: &mut InterruptFlags, dma_running: bool) {
        self.dma_running = dma_running;

        if !self.enabled {
            return;
        }

        self.cycles += 4;
        if (self.ly == 0 && self.cycles == 452) || self.cycles == CYCLES_PER_LINE {
            self.cycles = 0;
            self.ly += 1;
            if self.ly > 153 {
                self.ly = 0;
            }
        }

        if self.ly < 145 && self.cycles == 0 {
            self.mode = Mode::HBlank;
        } else if self.ly == 0 && self.cycles == 80 {
            self.mode = Mode::Draw;
        } else if self.ly == 0 && self.cycles == 252 {
            self.mode = Mode::HBlank;
        } else if self.ly > 0 && self.ly < 144 && self.cycles == 4 {
            self.mode = Mode::Scan;
        } else if self.ly > 0 && self.ly < 144 && self.cycles == 84 {
            self.mode = Mode::Draw;
        } else if self.ly > 0 && self.ly < 144 && self.cycles == 256 {
            self.mode = Mode::HBlank;
        } else if self.ly == 144 && self.cycles == 4 {
            self.mode = Mode::VBlank;
            interrupt_flags.set(Interrupt::VBlank, true);
        }
    }

    pub const fn read_vram(&self, addr: u16) -> u8 {
        if matches!(self.mode, Mode::Draw) {
            return 0xFF;
        }

        self.video_ram[addr as usize]
    }

    pub const fn write_vram(&mut self, addr: u16, data: u8) {
        if matches!(self.mode, Mode::Draw) {
            return;
        }

        self.video_ram[addr as usize] = data;
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        if self.dma_running || matches!(self.mode, Mode::Scan | Mode::Draw) {
            return 0xFF;
        }

        match addr {
            0x0000..=0x009F => self.attribute_ram[addr as usize],
            0x00A0..=0x00FF => 0x00,
            _ => unimplemented!(),
        }
    }

    pub const fn write_oam(&mut self, addr: u16, data: u8) {
        if self.dma_running || matches!(self.mode, Mode::Scan | Mode::Draw) {
            return;
        }

        match addr {
            0x0000..=0x009F => self.attribute_ram[addr as usize] = data,
            0x00A0..=0x00FF => {}
            _ => unimplemented!(),
        }
    }

    pub const fn write_sprite_unchecked(&mut self, addr: u16, data: u8) {
        if let 0x0000..=0x009F = addr {
            self.attribute_ram[addr as usize] = data;
        }
    }

    pub const fn read_display(&self, addr: u16) -> u8 {
        match addr {
            MEM_LCDC => self.read_lcdc(),
            MEM_STAT => self.read_stat(),
            MEM_SCY => self.scy,
            MEM_SCX => self.scx,
            MEM_LY => self.ly,
            MEM_LYC => self.lyc,
            MEM_BGP => self.background_palette,
            MEM_OBP0 => self.sprite_palette_0,
            MEM_OBP1 => self.sprite_palette_1,
            MEM_WY => self.wy,
            MEM_WX => self.wx,
            _ => unreachable!(),
        }
    }

    pub const fn write_display(&mut self, addr: u16, value: u8) {
        match addr {
            MEM_LCDC => self.write_lcdc(value),
            MEM_STAT => self.write_stat(value),
            MEM_SCY => self.scy = value,
            MEM_SCX => self.scx = value,
            MEM_LYC => self.lyc = value,
            MEM_BGP => self.background_palette = value,
            MEM_OBP0 => self.sprite_palette_0 = value,
            MEM_OBP1 => self.sprite_palette_1 = value,
            MEM_WY => self.wy = value,
            MEM_WX => self.wx = value,
            // LY is read-only
            _ => {}
        }
    }

    pub const fn read_lcdc(&self) -> u8 {
        let mut bits = 0;
        if self.enabled {
            bits |= 0x80;
        }
        if self.window_map {
            bits |= 0x40;
        }
        if self.window_enabled {
            bits |= 0x20;
        }
        if self.tile_addressing_mode {
            bits |= 0x10;
        }
        if self.bg_map {
            bits |= 0x08;
        }
        if self.sprite_size {
            bits |= 0x04;
        }
        if self.sprite_enabled {
            bits |= 0x02;
        }
        if self.bg_enabled {
            bits |= 0x01;
        }
        bits
    }

    pub const fn write_lcdc(&mut self, value: u8) {
        let old_enabled = self.enabled;

        self.enabled = value & 0x80 != 0;
        self.window_map = value & 0x40 != 0;
        self.window_enabled = value & 0x20 != 0;
        self.tile_addressing_mode = value & 0x10 != 0;
        self.bg_map = value & 0x08 != 0;
        self.sprite_size = value & 0x04 != 0;
        self.sprite_enabled = value & 0x02 != 0;
        self.bg_enabled = value & 0x01 != 0;

        if old_enabled && !self.enabled {
            self.ly = 0;
            self.cycles = 0;
            self.mode = Mode::HBlank;
        }
    }

    pub const fn read_stat(&self) -> u8 {
        let mut bits = 0x80;
        if self.lyc_intr_select {
            bits |= 0x40;
        }
        if self.mode2_intr_select {
            bits |= 0x20;
        }
        if self.mode1_intr_select {
            bits |= 0x10;
        }
        if self.mode0_intr_select {
            bits |= 0x08;
        }
        if self.ly == self.lyc
        {
            bits |= 0x04;
        }
        bits |= self.mode as u8;
        bits
    }

    pub const fn write_stat(&mut self, value: u8) {
        self.lyc_intr_select = value & 0x40 != 0;
        self.mode2_intr_select = value & 0x20 != 0;
        self.mode1_intr_select = value & 0x10 != 0;
        self.mode0_intr_select = value & 0x08 != 0;
    }
}
