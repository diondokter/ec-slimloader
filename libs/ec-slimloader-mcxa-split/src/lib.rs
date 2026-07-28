#![no_std]

use ec_slimloader::{Board, BootStatePolicy};
use ec_slimloader_state::flash::FlashJournal;
use embassy_embedded_hal::{adapter::BlockingAsync, flash::partition::Partition};
use embassy_mcxa::{
    bind_interrupts,
    flexspi::{self, Async, ClockConfig, FlashConfig, IoError},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_storage_async::nor_flash::{NorFlash, NorFlashErrorKind};

pub use embassy_mcxa;
use static_cell::StaticCell;

const INTERNAL_FLASH_RANGE: core::ops::Range<u32> = 0x0000_0000..0x0020_0000;
const EXTERNAL_FLASH_RANGE: core::ops::Range<u32> = 0x8000_0000..0x9000_0000;

bind_interrupts!(struct Irqs {
    FLEXSPI0 => flexspi::InterruptHandler<peripherals::FLEXSPI0>;
});

pub struct McxaBoard {
    flash_journal: FlashJournal<Partition<'static, NoopRawMutex, ExternalFlash>>,
}

impl Board for McxaBoard {
    type Config = McxaConfig;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        let p = embassy_mcxa::init(Default::default());

        config.check();

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

        static INTERNAL_FLASH: StaticCell<Mutex<NoopRawMutex, BlockingAsync<embassy_mcxa::flash::Flash>>> =
            StaticCell::new();
        let internal_flash = INTERNAL_FLASH.init(Mutex::new(BlockingAsync::new(
            embassy_mcxa::flash::Flash::new().unwrap(),
        )));

        let slot_0a_partition = Partition::new(
            internal_flash,
            config.slot_0a.start,
            config.slot_0a.end - config.slot_0a.start,
        );
        let slot_0b_partition = Partition::new(
            external_flash,
            config.slot_0b.start - EXTERNAL_FLASH_RANGE.start,
            config.slot_0b.end - config.slot_0b.start,
        );
        let slot_1a_partition = Partition::new(
            internal_flash,
            config.slot_1a.start,
            config.slot_1a.end - config.slot_1a.start,
        );
        let slot_1b_partition = Partition::new(
            external_flash,
            config.slot_1b.start - EXTERNAL_FLASH_RANGE.start,
            config.slot_1b.end - config.slot_1b.start,
        );
        let journal_partition = Partition::new(
            external_flash,
            config.journal.start - EXTERNAL_FLASH_RANGE.start,
            config.journal.end - config.journal.start,
        );
        let scratch_partition = Partition::new(
            external_flash,
            config.scratch_space.start - EXTERNAL_FLASH_RANGE.start,
            config.scratch_space.end - config.scratch_space.start,
        );
        let swap_partition = Partition::new(
            external_flash,
            config.swap_log.start - EXTERNAL_FLASH_RANGE.start,
            config.swap_log.end - config.swap_log.start,
        );

        let flash_journal = FlashJournal::new::<JOURNAL_BUFFER_SIZE>(journal_partition)
            .await
            .unwrap();

        Self { flash_journal }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.flash_journal
    }

    async fn check_and_boot<const JOURNAL_BUFFER_SIZE: usize>(
        &mut self,
        slot: &ec_slimloader_state::state::Slot,
    ) -> ec_slimloader::BootError {
        if u8::from(*slot) > 1 {
            return ec_slimloader::BootError::SlotUnknown;
        }

        todo!()
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
    /// Must be in *external* flash
    pub scratch_space: core::range::Range<u32>,
    /// Must be in *external* flash
    pub swap_log: core::range::Range<u32>,

    pub external_flash_config: FlashConfig,
}

impl BootStatePolicy for McxaConfig {}

impl McxaConfig {
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
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&self.scratch_space.start),
            "scratch_space.start not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&(self.scratch_space.end - 1)),
            "scratch_space.end not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&self.swap_log.start),
            "swap_log.start not in EXTERNAL_FLASH_RANGE"
        );
        assert!(
            EXTERNAL_FLASH_RANGE.contains(&(self.swap_log.end - 1)),
            "swap_log.end not in EXTERNAL_FLASH_RANGE"
        );
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
