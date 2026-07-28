#![no_std]
#![no_main]

#[cfg(any(feature = "defmt", feature = "log"))]
use defmt_or_log::info;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use ec_slimloader_mcxa_split::embassy_mcxa::flexspi::lookup::opcodes::sdr::{CMD, DUMMY, MODE8, RADDR, READ, WRITE};
use ec_slimloader_mcxa_split::embassy_mcxa::flexspi::lookup::{Command, Instr, LookupTable, Pads, SequenceBuilder};
use ec_slimloader_mcxa_split::embassy_mcxa::flexspi::{DeviceCommand, FlashConfig};
use embassy_executor::Spawner;
use panic_halt as _;

const JOURNAL_BUFFER_SIZE: usize = 4096;

#[cfg(feature = "defmt")]
defmt::timestamp!("{=u32}", 0);

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    #[cfg(any(feature = "defmt", feature = "log"))]
    info!("Starting MCXA bootloader");

    ec_slimloader::start::<ec_slimloader_mcxa_split::McxaBoard, JOURNAL_BUFFER_SIZE>(
        ec_slimloader_mcxa_split::McxaConfig {
            slot_0a: (0x00010000..0x00100000).into(),
            slot_0b: (0x80010000..0x80100000).into(),
            slot_1a: (0x00110000..0x00200000).into(),
            slot_1b: (0x80110000..0x80200000).into(),
            journal: (0x00100000..0x00110000).into(),
            scratch_space: (0x80000000..0x80010000).into(),
            swap_log: (0x80100000..0x80110000).into(),
            external_flash_config: FLASH_CONFIG,
        },
    )
    .await
}

pub const FLASH_PAGE_SIZE: usize = 256;
pub const FLASH_SECTOR_SIZE: usize = 4096;
const ENTER_OPI_SEQ: u8 = Command::WriteStatus as u8;

/// Flash configuration for the FRDM-MCXA577's on-board NOR flash.
///
/// The board carries a Winbond **W25Q64** (64 Mbit = 8 MiB), driven here in
/// 1-4-4 quad mode. The read/erase/program LUT sequences below all use a 3-byte
/// (24-bit) address, which reaches 16 MiB and so covers this part in full; a
/// flash larger than 16 MiB would need 4-byte-address sequences instead.
///
/// The FlexSPI hardware is not the limiting factor: the AHB memory-mapped window
/// spans 256 MiB (secure `0x9000_0000..=0x9FFF_FFFF`) and IP commands carry a
/// full 32-bit address in `IPCR0.SFAR`.
pub const FLASH_CONFIG: FlashConfig = FlashConfig {
    // W25Q64 == 8 MiB == 8192 KiB. `flash_size_kbytes` programs FLSHCR0.FLSHSZ,
    // which bounds every IP and memory-mapped access; sizing it to the real chip
    // makes the controller reject out-of-range accesses instead of wrapping.
    flash_size_kbytes: 0x2000,
    page_size: FLASH_PAGE_SIZE,
    busy_status_polarity: true,
    busy_status_offset: 0,
    lookup_table: LookupTable::new()
        .command(
            Command::Read,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0xEB))
                .instr(Instr::new(RADDR, Pads::Four, 0x18))
                .instr(Instr::new(MODE8, Pads::Four, 0xF0))
                .instr(Instr::new(DUMMY, Pads::Four, 0x04))
                .instr(Instr::new(READ, Pads::Four, 0x00))
                .build(),
        )
        .command(
            Command::ReadStatus,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x05))
                .instr(Instr::new(READ, Pads::One, 0x00))
                .build(),
        )
        .command(
            Command::WriteEnable,
            SequenceBuilder::new().instr(Instr::new(CMD, Pads::One, 0x06)).build(),
        )
        .command(
            Command::ReadId,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x9F))
                .instr(Instr::new(READ, Pads::One, 0x00))
                .build(),
        )
        .command(
            Command::EraseSector,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x20))
                .instr(Instr::new(RADDR, Pads::One, 0x18))
                .build(),
        )
        .command(
            Command::PageProgram,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x02))
                .instr(Instr::new(RADDR, Pads::One, 0x18))
                .instr(Instr::new(WRITE, Pads::One, 0x00))
                .build(),
        )
        .command(
            Command::WriteStatus,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x05))
                .instr(Instr::new(READ, Pads::One, 0x00))
                .build(),
        ),
    read_seq: Command::Read as u8,
    read_status_seq: Command::ReadStatus as u8,
    write_enable_seq: Command::WriteEnable as u8,
    read_id_seq: Command::ReadId as u8,
    erase_sector_seq: Command::EraseSector as u8,
    page_program_seq: Command::PageProgram as u8,
    reset_sequence: Some(
        SequenceBuilder::new()
            .instr(Instr::new(CMD, Pads::One, 0x66))
            .instr(Instr::new(CMD, Pads::One, 0x99))
            .build(),
    ),
    device_mode_command: Some(DeviceCommand::new(ENTER_OPI_SEQ, [0xE7, 0, 0, 0], 1, true)),
};
