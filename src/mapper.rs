use crate::rom::Mirroring;

/// A cartridge mapper controls how CPU addresses map to PRG-ROM
/// and how PPU addresses map to CHR-ROM (or CHR-RAM).
///
/// NROM (mapper 0) is the simplest: fixed, linear banks with
/// optional mirroring of the upper 16 KB when only one PRG bank exists.
pub trait Mapper: Send {
    /// Map a CPU-space address ($8000–$FFFF) to a PRG-ROM byte.
    fn read_prg(&self, addr: u16) -> u8;

    /// Write to PRG-ROM space. On most mappers this routes to PRG-RAM
    /// or mapper registers rather than ROM itself.
    fn write_prg(&mut self, addr: u16, value: u8);

    /// Map a PPU-space address ($0000–$1FFF) to a CHR-ROM byte.
    fn read_chr(&self, addr: u16) -> u8;

    /// Write to CHR space. On mappers with CHR-RAM this writes RAM;
    /// on CHR-ROM mappers it's a no-op.
    fn write_chr(&mut self, addr: u16, value: u8);

    /// Cartridge-controlled nametable mirroring.
    fn mirroring(&self) -> Mirroring;
}

/// NROM — mapper 0. Used by virtually all launch-title cartridge boards:
/// NROM-128 (16 KB PRG, mirrored) and NROM-256 (32 KB PRG).
pub struct Nrom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirroring: Mirroring,
}

impl Nrom {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        Nrom {
            prg_rom,
            chr_rom,
            mirroring,
        }
    }
}

impl Mapper for Nrom {
    fn read_prg(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            // NROM-128: mirror the single 16 KB bank across $C000–$FFFF
            addr %= 0x4000;
        }
        self.prg_rom[addr as usize]
    }

    fn write_prg(&mut self, mut addr: u16, value: u8) {
        addr -= 0x8000;
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr %= 0x4000;
        }
        // NROM has no PRG-RAM; the emulator allows writes for the
        // test harness (CPU::load writes programs into PRG-ROM space).
        self.prg_rom[addr as usize] = value;
    }

    fn read_chr(&self, addr: u16) -> u8 {
        // CHR-ROM addresses are $0000–$1FFF (8 KB max for NROM)
        self.chr_rom[addr as usize]
    }

    fn write_chr(&mut self, _addr: u16, _value: u8) {
        // CHR-ROM is read-only on NROM
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_prg(size: usize, fill: u8) -> Vec<u8> {
        vec![fill; size]
    }

    #[test]
    fn test_nrom_256_read() {
        let prg = make_prg(0x8000, 0xAB);
        let chr = vec![0; 0x2000];
        let mapper = Nrom::new(prg, chr, Mirroring::HORIZONTAL);

        // $8000 → offset 0
        assert_eq!(mapper.read_prg(0x8000), 0xAB);
        // $FFFF → offset 0x7FFF
        assert_eq!(mapper.read_prg(0xFFFF), 0xAB);
    }

    #[test]
    fn test_nrom_128_mirroring() {
        let prg = make_prg(0x4000, 0x42);
        let chr = vec![0; 0x2000];
        let mapper = Nrom::new(prg, chr, Mirroring::VERTICAL);

        // $8000 and $C000 both map to offset 0 (mirror)
        assert_eq!(mapper.read_prg(0x8000), 0x42);
        assert_eq!(mapper.read_prg(0xC000), 0x42);
        // $BFFF (last byte of bank) and $FFFF (mirror) should match
        assert_eq!(mapper.read_prg(0xBFFF), 0x42);
        assert_eq!(mapper.read_prg(0xFFFF), 0x42);
    }

    #[test]
    fn test_nrom_chr_read() {
        let prg = make_prg(0x4000, 0);
        let mut chr = vec![0; 0x2000];
        chr[0x0000] = 0xAA;
        chr[0x1FFF] = 0xBB;
        let mapper = Nrom::new(prg, chr, Mirroring::HORIZONTAL);

        assert_eq!(mapper.read_chr(0x0000), 0xAA);
        assert_eq!(mapper.read_chr(0x1FFF), 0xBB);
    }

    #[test]
    fn test_nrom_chr_write_is_noop() {
        let prg = make_prg(0x4000, 0);
        let chr = vec![0xFF; 0x2000];
        let mut mapper = Nrom::new(prg, chr, Mirroring::HORIZONTAL);

        mapper.write_chr(0x0000, 0x00);
        // CHR-ROM is read-only — value unchanged
        assert_eq!(mapper.read_chr(0x0000), 0xFF);
    }

    #[test]
    fn test_nrom_mirroring_vertical() {
        let mapper = Nrom::new(vec![0], vec![0], Mirroring::VERTICAL);
        assert_eq!(mapper.mirroring(), Mirroring::VERTICAL);
    }

    #[test]
    fn test_nrom_prg_write_round_trip() {
        let prg = make_prg(0x8000, 0);
        let chr = vec![0; 0x2000];
        let mut mapper = Nrom::new(prg, chr, Mirroring::HORIZONTAL);

        mapper.write_prg(0x8000, 0x77);
        assert_eq!(mapper.read_prg(0x8000), 0x77);
    }
}
