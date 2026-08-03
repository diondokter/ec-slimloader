#![no_std]

use core::ptr::null_mut;

use ec_slimloader::{Board, BootError, BootStatePolicy};
use ec_slimloader_state::{flash::FlashJournal, state::Slot};
use embassy_embedded_hal::flash::partition::Partition;
use embassy_mcxa::{
    bind_interrupts,
    flexspi::{self, Async, ClockConfig, FlashConfig, IoError},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_storage_async::nor_flash::{NorFlash, NorFlashErrorKind};

pub use embassy_mcxa;
use static_cell::StaticCell;

use crate::rom::kb_options_t;

const INTERNAL_FLASH_RANGE: core::ops::Range<u32> = 0x0000_0000..0x0020_0000;
const EXTERNAL_FLASH_RANGE: core::ops::Range<u32> = 0x8000_0000..0x9000_0000;

mod rom;

bind_interrupts!(struct Irqs {
    FLEXSPI0 => flexspi::InterruptHandler<peripherals::FLEXSPI0>;
});

pub struct McxaBoard {
    flash_journal: FlashJournal<Partition<'static, NoopRawMutex, ExternalFlash>>,
    memory: MemoryRanges,
}

impl Board for McxaBoard {
    type Config = McxaConfig;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        let p = embassy_mcxa::init(Default::default());

        config.memory.check();

        static EXTERNAL_FLASH: StaticCell<Mutex<NoopRawMutex, ExternalFlash>> = StaticCell::new();
        let external_flash = EXTERNAL_FLASH.init(Mutex::new(ExternalFlash(flexspi::NorFlash::new(
            flexspi::Flexspi::new_async(
                p.FLEXSPI0,
                p.P3_0,
                p.P3_7,
                p.P3_6,
                p.P3_8,
                p.P3_9,
                p.P3_10,
                p.P3_11,
                Irqs,
                ClockConfig::default(),
                config.external_flash_config,
            )
            .unwrap(),
        ))));

        let journal_partition = Partition::new(
            external_flash,
            config.memory.journal.start - EXTERNAL_FLASH_RANGE.start,
            config.memory.journal.end - config.memory.journal.start,
        );

        let flash_journal = FlashJournal::new::<JOURNAL_BUFFER_SIZE>(journal_partition)
            .await
            .unwrap();

        Self {
            flash_journal,
            memory: config.memory,
        }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.flash_journal
    }

    async fn check_and_boot<const JOURNAL_BUFFER_SIZE: usize>(
        &mut self,
        slot: &ec_slimloader_state::state::Slot,
    ) -> Result<core::convert::Infallible, BootError> {
        if u8::from(*slot) > 1 {
            return Err(BootError::SlotUnknown);
        }

        let kb_api = unsafe { *rom::rom_api().kb_api };

        let mut buffer = [0u8; 8192];
        let mut kb_session = null_mut();
        let kb_options = kb_options_t {
            version: 1,
            buffer: buffer.as_mut_ptr(),
            buffer_length: buffer.len() as u32,
            op: rom::KbOperation::KRomAuthenticateImage {
                profile: 0,
                min_build_number: 0,
                max_image_length: 0x1000_0000,
                user_rhk: null_mut(),
            },
        };
        let status = (kb_api.kb_init)(&raw mut kb_session, &raw const kb_options);
        defmt_or_log::debug!("Init KB api: {}", status);
        status.into_result()?;

        let (image_ptr, image_len) = self.memory.get_ptr_len_a(*slot);
        let status = (kb_api.kb_execute)(kb_session, image_ptr, image_len);
        defmt_or_log::debug!("Execute KB api: {}", status);
        status.into_result()?;

        self.memory.jump_to(*slot)
    }

    fn abort(&mut self) -> ! {
        loop {
            cortex_m::asm::wfe();
        }
    }

    fn reboot(&mut self) -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}

pub struct McxaConfig {
    pub memory: MemoryRanges,
    pub external_flash_config: FlashConfig,
}

impl BootStatePolicy for McxaConfig {
    fn is_valid_state(state: &ec_slimloader_state::state::State) -> bool {
        matches!(
            state.target(),
            ec_slimloader_state::state::Slot::S0 | ec_slimloader_state::state::Slot::S1
        ) && matches!(
            state.backup(),
            ec_slimloader_state::state::Slot::S0 | ec_slimloader_state::state::Slot::S1
        )
    }
}

pub struct MemoryRanges {
    /// Must be in *internal* flash
    pub slot_0a: core::range::Range<u32>,
    /// Must be in *external* flash
    pub slot_0b: core::range::Range<u32>,
    /// Must be in *internal* flash
    pub slot_1a: core::range::Range<u32>,
    /// Must be in *external* flash
    pub slot_1b: core::range::Range<u32>,
    /// Must be in *external* flash
    pub journal: core::range::Range<u32>,
}

impl MemoryRanges {
    fn get_ptr_len_a(&self, slot: Slot) -> (*const u8, u32) {
        match slot {
            Slot::S0 => (self.slot_0a.start as *const u8, self.slot_0a.end - self.slot_0a.start),
            Slot::S1 => (self.slot_1a.start as *const u8, self.slot_1a.end - self.slot_1a.start),
            _ => unreachable!(),
        }
    }

    fn get_ptr_len_b(&self, slot: Slot) -> (*const u8, u32) {
        match slot {
            Slot::S0 => (self.slot_0b.start as *const u8, self.slot_0b.end - self.slot_0b.start),
            Slot::S1 => (self.slot_1b.start as *const u8, self.slot_1b.end - self.slot_1b.start),
            _ => unreachable!(),
        }
    }

    fn check(&self) {
        assert!(
            INTERNAL_FLASH_RANGE.contains(&self.slot_0a.start),
            "slot_0a.start not in INTERNAL_FLASH_RANGE"
        );
        assert!(
            INTERNAL_FLASH_RANGE.contains(&(self.slot_0a.end - 1)),
            "slot_0a.end not in INTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&self.slot_0b.start),
            "slot_0b.start not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&(self.slot_0b.end - 1)),
            "slot_0b.end not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            INTERNAL_FLASH_RANGE.contains(&self.slot_1a.start),
            "slot_1a.start not in INTERNAL_FLASH_RANGE"
        );
        assert!(
            INTERNAL_FLASH_RANGE.contains(&(self.slot_1a.end - 1)),
            "slot_1a.end not in INTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&self.slot_1b.start),
            "slot_1b.start not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&(self.slot_1b.end - 1)),
            "slot_1b.end not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&self.journal.start),
            "journal.start not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&(self.journal.end - 1)),
            "journal.end not in EXTERNAL_FLASH_RANGE"
        );
    }

    fn jump_to(&self, slot: Slot) -> ! {
        cortex_m::interrupt::disable();
        cortex_m::asm::dsb();

        let entry = self.get_ptr_len_a(slot).0;

        unsafe { (&*cortex_m::peripheral::SCB::PTR).vtor.write(entry as u32) };

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        unsafe { cortex_m::asm::bootload(entry.cast()) }
    }
}

struct ExternalFlash(flexspi::NorFlash<'static, Async>);

#[derive(Debug)]
struct ExternalFlashError(IoError);

impl From<IoError> for ExternalFlashError {
    fn from(value: IoError) -> Self {
        Self(value)
    }
}

impl embedded_storage_async::nor_flash::NorFlashError for ExternalFlashError {
    fn kind(&self) -> NorFlashErrorKind {
        match self.0 {
            IoError::Command { error_code } => match error_code {
                embassy_mcxa::pac::flexspi::Ipcmderrcode::Val1 => NorFlashErrorKind::NotAligned,
                embassy_mcxa::pac::flexspi::Ipcmderrcode::Val6 => NorFlashErrorKind::OutOfBounds,
                _ => NorFlashErrorKind::Other,
            },
            _ => NorFlashErrorKind::Other,
        }
    }
}

impl embedded_storage_async::nor_flash::ErrorType for ExternalFlash {
    type Error = ExternalFlashError;
}

impl embedded_storage_async::nor_flash::ReadNorFlash for ExternalFlash {
    const READ_SIZE: usize = 1;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read_async(offset, bytes).await?;
        Ok(())
    }

    #[track_caller]
    fn capacity(&self) -> usize {
        panic!("No way to query capacity");
    }
}

impl embedded_storage_async::nor_flash::NorFlash for ExternalFlash {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = 4096; // TODO: expose through some config

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        for sector in (from..to)
            .step_by(Self::ERASE_SIZE)
            .map(|addr| addr / Self::ERASE_SIZE as u32)
        {
            self.0.erase_sector_async(sector).await?;
        }

        Ok(())
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.page_program_async(offset, bytes).await?;
        Ok(())
    }
}
