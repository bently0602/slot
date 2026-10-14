// Original 65816 homebrew, with no commercial game data. Paints a red backdrop,
// polls the two console ports (including a port-two multitap) into battery SRAM,
// and counts polling loops. Also exercises interlaced/hi-res video and PAL timing.
pub fn rom(pal: bool, hires: bool) -> Vec<u8> {
    let mut code = vec![0x78, 0x18, 0xfb, 0xe2, 0x30, 0xa2, 0xff, 0x9a];
    for (address, value) in [
        (0x4200u16, 0),
        (0x420c, 0),
        (0x2100, 0x80),
        (0x2105, 0),
        (0x212c, 0),
        (0x2121, 0),
        (0x2122, 0x1f),
        (0x2122, 0),
        (0x2133, if hires { 9 } else { 0 }),
        (0x2100, 15),
    ] {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    let loop_at = 0x8000 + code.len() as u16;
    for (address, value) in [(0x4201u16, 0x80), (0x4016, 1), (0x4016, 0)] {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    for i in 0..16u8 {
        // LDA $4016/7; AND #mask; STA $700000+offset.
        for (port, mask, offset) in [(0x16, 1, i), (0x17, 3, 16 + i)] {
            code.extend([0xad, port, 0x40, 0x29, mask, 0x8f, offset, 0, 0x70]);
        }
    }
    code.extend([0x9c, 0x01, 0x42]); // STZ $4201: select second multitap pair.
    for i in 0..16u8 {
        code.extend([0xad, 0x17, 0x40, 0x29, 3, 0x8f, 32 + i, 0, 0x70]);
    }
    // REP #$20; LDA $700040; INC A; STA $700040; SEP #$20 (16-bit loop count).
    code.extend([
        0xc2, 0x20, 0xaf, 64, 0, 0x70, 0x1a, 0x8f, 64, 0, 0x70, 0xe2, 0x20,
    ]);
    code.extend([0x4c, loop_at as u8, (loop_at >> 8) as u8]);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"SLOT SNES INPUT TEST ");
    rom[0x7fd5..0x7fdb].copy_from_slice(&[0x20, 2, 5, 1, if pal { 2 } else { 1 }, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    let checksum = rom
        .iter()
        .fold(0u16, |sum, &byte| sum.wrapping_add(u16::from(byte)))
        .wrapping_add(510);
    rom[0x7fdc..0x7fde].copy_from_slice(&(!checksum).to_le_bytes());
    rom[0x7fde..0x7fe0].copy_from_slice(&checksum.to_le_bytes());
    rom
}

pub fn buttons(ram: &[u8], player: usize) -> u16 {
    let (offset, bit) = match player {
        0 => (0, 0),
        1 => (16, 0),
        2 => (16, 1),
        3 => (32, 0),
        _ => panic!("only four players"),
    };
    ram[offset..offset + 16].iter().fold(0u16, |held, byte| {
        (held << 1) | u16::from((byte >> bit) & 1)
    })
}

// Original SPC700 homebrew: upload a DSP noise generator through the IPL ports.
// Noise makes each sample sensitive to the audio timeline without external assets.
#[allow(dead_code)]
pub fn audio_rom() -> Vec<u8> {
    let mut spc = Vec::new();
    for (reg, value) in [
        (0x6c, 0x3c),
        (0x0c, 0x60),
        (0x1c, 0x60),
        (0x00, 0x60),
        (0x01, 0x60),
        (0x05, 0),
        (0x07, 0x7f),
        (0x3d, 1),
        (0x4c, 1),
    ] {
        spc.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
    }
    spc.extend([0x2f, 0xfe]);
    let mut code = vec![0x78, 0x18, 0xfb, 0xe2, 0x30, 0xa2, 0xff, 0x9a];
    fn write(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    fn wait(code: &mut Vec<u8>, value: u8) {
        code.extend([0xad, 0x40, 0x21, 0xc9, value, 0xd0, 0xf9]);
    }
    write(&mut code, 0x2100, 0x80);
    wait(&mut code, 0xaa);
    for (address, value) in [(0x2142, 0), (0x2143, 2), (0x2141, 1), (0x2140, 0xcc)] {
        write(&mut code, address, value);
    }
    wait(&mut code, 0xcc);
    for (index, value) in spc.iter().copied().enumerate() {
        write(&mut code, 0x2141, value);
        write(&mut code, 0x2140, index as u8);
        wait(&mut code, index as u8);
    }
    for (address, value) in [
        (0x2142, 0),
        (0x2143, 2),
        (0x2141, 0),
        (0x2140, spc.len() as u8 + 1),
    ] {
        write(&mut code, address, value);
    }
    code.extend([0x80, 0xfe]);
    let mut result = rom(false, false);
    result[..0x7fc0].fill(0);
    result[..code.len()].copy_from_slice(&code);
    result[0x7fc0..0x7fd5].copy_from_slice(b"SLOT SNES AUDIO TEST ");
    result[0x7fdc..0x7fe0].fill(0);
    let checksum = result
        .iter()
        .fold(510u16, |sum, &byte| sum.wrapping_add(u16::from(byte)));
    result[0x7fdc..0x7fde].copy_from_slice(&(!checksum).to_le_bytes());
    result[0x7fde..0x7fe0].copy_from_slice(&checksum.to_le_bytes());
    result
}
