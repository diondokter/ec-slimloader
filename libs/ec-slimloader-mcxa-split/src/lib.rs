#![no_std]

use ec_slimloader::{Board, BootStatePolicy};
use ec_slimloader_state::flash::FlashJournal;
use embassy_mcxa::{
    bind_interrupts,
    flexspi::{self, Async, ClockConfig, FlashConfig, IoError},
    peripherals,
};
use embedded_storage_async::nor_flash::{NorFlash, NorFlashErrorKind};

pub use embassy_mcxa;

bind_interrupts!(struct Irqs {
    FLEXSPI0 => flexspi::InterruptHandler<peripherals::FLEXSPI0>;
});

pub struct McxaBoard {
    flash_journal: FlashJournal<ExternalFlash>,
}

impl Board for McxaBoard {
    type Config = McxaConfig;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        let p = embassy_mcxa::init(Default::default());

        let external_flash = ExternalFlash(flexspi::NorFlash::new(
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
        ));

        let flash_journal = FlashJournal::new::<JOURNAL_BUFFER_SIZE>(external_flash).await.unwrap();

        Self { flash_journal }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.flash_journal
    }

    async fn check_and_boot<const JOURNAL_BUFFER_SIZE: usize>(
        &mut self,
        slot: &ec_slimloader_state::state::Slot,
    ) -> ec_slimloader::BootError {
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
    pub external_flash_config: FlashConfig,
}

impl BootStatePolicy for McxaConfig {}

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
