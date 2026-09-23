#![no_std]

use core::convert::Infallible;

use ec_slimloader_mcxa::certificate::derive_image_rkth_pair;
use ec_slimloader_mcxa::header::ImageHeader;
use embassy_mcxa::rom::FlashError;
use embassy_mcxa::{peripherals, Peri};
use mcxa_ifr::{
    BootCfg0, BootCfg1, DiceCsrKeyType, LifeCycleState, LspiQflashCfg0, QspiPort, ReverseArray, RotkRevoke, RotkUsage,
    RotkUsageVal, SecureBootCfg, Update, CFPA, IFR,
};

pub struct Provisioner {
    ifr: IFR,
    secure_alias: bool,
    cmpa_updated: bool,
    cfpa_updated: bool,
}

impl Provisioner {
    const DEFAULT_CC_SOCU_PIN: u32 = 0x1FFFE000; // default value for CC_SOCU_PIN in CMPA, for breakdown, refer to NXP secure provisioning guide.
    const DEFAULT_CC_SOCU_DFLT: u32 = 0xBFFF4000; // default value for CC_SOCU_DFLT in CMPA, for breakdown, refer to NXP secure provisioning guide.

    /// # Safety
    /// Only sound to call on an mcxa5xx device
    pub unsafe fn new_from_page(secure_alias: bool) -> Self {
        let disk_ifr = if secure_alias {
            IFR::secure_current()
        } else {
            IFR::current()
        };

        Self {
            ifr: disk_ifr.clone(),
            secure_alias,
            cmpa_updated: false,
            cfpa_updated: false,
        }
    }

    pub fn commit_and_reboot(&mut self) -> Result<Infallible, FlashError> {
        match (self.cfpa_updated, self.cmpa_updated) {
            (false, false) => {
                // Nothing to commit
            }
            (_, true) => {
                self.erase_scratch()?;
                self.commit_cmpa_and_cfpa()?;
            }
            (true, false) => {
                self.erase_scratch()?;
                self.commit_cfpa_only()?;
            }
        }

        cortex_m::peripheral::SCB::sys_reset()
    }

    fn erase_scratch(&self) -> Result<(), FlashError> {
        let ifr_scratch_addr = if self.secure_alias {
            IFR::SECURE_SCRATCH_ADDR
        } else {
            IFR::SCRATCH_ADDR
        };

        let mut drv = embassy_mcxa::rom::get().flash()?;

        drv.erase_sector(ifr_scratch_addr, 8 * 1024)?;
        drv.ifr_verify_erase_sector(ifr_scratch_addr, 8 * 1024)?;

        Ok(())
    }

    fn commit_cfpa_only(&mut self) -> Result<(), FlashError> {
        let scratch_addr = if self.secure_alias {
            IFR::SECURE_SCRATCH_ADDR
        } else {
            IFR::SCRATCH_ADDR
        };

        self.ifr.update.devcfg_upd_type = mcxa_ifr::UpdateType::CfpaUpdated;

        let ifr_subslice = &self.ifr.as_array().as_slice()[..size_of::<Update>() + size_of::<CFPA>()];

        let mut drv = embassy_mcxa::rom::get().flash()?;

        drv.program_phrase(scratch_addr, ifr_subslice)?;
        drv.verify_program(scratch_addr, ifr_subslice)?;

        Ok(())
    }

    fn commit_cmpa_and_cfpa(&mut self) -> Result<(), FlashError> {
        let scratch_addr = if self.secure_alias {
            IFR::SECURE_SCRATCH_ADDR
        } else {
            IFR::SCRATCH_ADDR
        };

        self.ifr.update.devcfg_upd_type = mcxa_ifr::UpdateType::CmpaUpdated;

        let ifr_slice = self.ifr.as_array().as_slice();

        let mut drv = embassy_mcxa::rom::get().flash()?;

        drv.program_phrase(scratch_addr, ifr_slice)?;
        drv.verify_program(scratch_addr, ifr_slice)?;

        Ok(())
    }

    pub fn bump_auth_fail_count(&mut self) -> &mut Self {
        self.ifr.cfpa.err_auth_fail_count = self.ifr.cfpa.err_auth_fail_count.wrapping_add(1);
        self.cfpa_updated = true;
        self
    }

    pub fn bump_firmware_version(&mut self) -> &mut Self {
        self.ifr.cfpa.ee0_fw_version = self.ifr.cfpa.ee0_fw_version.wrapping_add(1);
        self.cfpa_updated = true;
        self
    }

    pub fn with_rotk_revoke(&mut self, val: RotkRevoke) -> &mut Self {
        self.ifr.cfpa.rotk_revoke = val;
        self.cfpa_updated = true;
        self
    }

    pub fn advance_lifecycle(&mut self, desired_lifecycle: LifeCycleState) -> Result<&mut Self, ProvisionError> {
        let current_lifecycle = self
            .ifr
            .cfpa
            .header
            .cfpa_lc_state()
            .map_err(|_| ProvisionError::UnexpectedIfrState)?;

        if desired_lifecycle.rank() >= current_lifecycle.rank() && desired_lifecycle != current_lifecycle {
            self.ifr.cfpa.header.set_cfpa_lc_state(desired_lifecycle);
            self.ifr.cfpa.header.set_inv_cfpa_lc_state(desired_lifecycle.inverse());
            self.cfpa_updated = true;
            Ok(self)
        } else {
            Err(ProvisionError::InvalidLifeCycle)
        }
    }

    /// Sets a "default" secure Boot configuration for CMPA. This is intended for first-time provisioning of a DEV unit, and must only be called in the Develop lifecycle state.
    /// Not intended for production or factory use.
    pub fn with_default_secure_cmpa_config(&mut self, config: CmpaDefaultConfig) -> Result<&mut Self, ProvisionError> {
        if self.ifr.cfpa.header.cfpa_lc_state() != Ok(LifeCycleState::Develop) {
            return Err(ProvisionError::InvalidLifeCycle);
        }

        self.ifr.cmpa.boot_cfg0 = config.boot_cfg0.with_marker(0x5963);
        self.ifr.cmpa.boot_cfg1 = config.boot_cfg1;
        self.ifr.cmpa.rotk_usage = config.rotk_usage;
        self.ifr.cmpa.secure_boot_cfg = SecureBootCfg::builder()
            .with_sec_boot_en(mcxa_ifr::SecBootEn::OnlyPki)
            .with_lp_sec_boot(mcxa_ifr::LpSecBoot::Cold)
            .with_dice_csr_key_type(config.dice_csr_key_type)
            .with_enf_cnsa(mcxa_ifr::EnfCnsa::CNSA2)
            .with_enf_tzm_preset(config.enf_tzm_preset.into())
            .with_fast_boot_en(mcxa_ifr::InverseBigBool::False)
            .with_active_img_prot(mcxa_ifr::ActiveImgProt::GLBAC2)
            .with_fips_sha_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_aes_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_ecdsa_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_drbg_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_cmac_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_kdf_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_dis_nxp_fw(mcxa_ifr::DisNxpFw::DisableNxpSignedSb3Fw)
            .build();
        self.ifr.cmpa.sbl_start_addr = config.sbl_start_addr;
        self.ifr.cmpa.rotkh = ReverseArray::new(config.rotkh);
        self.ifr.cmpa.pqc_rotkh = ReverseArray::new(config.pqc_rotkh);

        self.ifr.cmpa.cc_socu_pin = Self::DEFAULT_CC_SOCU_PIN;
        self.ifr.cmpa.cc_socu_dflt = Self::DEFAULT_CC_SOCU_DFLT;

        self.cmpa_updated = true;

        Ok(self)
    }

    /// First boot IFR (CMPA / CFPA) provisioning for DEV and/or factory floor, non-security critical.
    /// Keeps secure boot, DICE and TZM disabled, and sets a safe default for other fields.
    pub fn with_initial_config(&mut self) -> &mut Self {
        self.ifr.cmpa.boot_cfg0 = BootCfg0::builder()
            .with_marker(0x5963)
            .with_boot_speed(mcxa_ifr::BootSpeed::MdMode)
            .with_rec_boot_en(false.into())
            .with_rec_lspi(false)
            .with_rec_flexspi(false)
            .with_eflash_dual_en(false)
            .with_eflash_booten(true)
            .with_iflash_dual_en(false)
            .with_iflash_booten(true)
            .build();
        self.ifr.cmpa.boot_cfg1 = BootCfg1::builder()
            .with_ext_cmpa_32b_size(160) // Max size
            .with_flash_remap_size(0u8.into())
            .with_isp_ft_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_api_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_dm_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_pin_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_usb_en(false)
            .with_isp_i2c_en(false)
            .with_isp_can_en(false)
            .with_isp_spi_en(false)
            .with_isp_uart_en(true)
            .build();
        self.ifr.cmpa.rotk_usage = RotkUsage::builder()
            .with_dice_inc_nxp_field_cfg(false)
            .with_dice_inc_cust_cfg(false)
            .with_dice_inc_nxp_cfg(false)
            .with_disable_dice(true) // Dice not yet set up
            .with_rotk_usage([RotkUsageVal::All; 4])
            .build();
        self.ifr.cmpa.secure_boot_cfg = SecureBootCfg::builder()
            .with_sec_boot_en(mcxa_ifr::SecBootEn::AllAllowed)
            .with_lp_sec_boot(mcxa_ifr::LpSecBoot::Cold)
            .with_dice_csr_key_type(DiceCsrKeyType::Sha384AndMLDSA)
            .with_enf_cnsa(mcxa_ifr::EnfCnsa::CNSA2)
            .with_enf_tzm_preset(false.into())
            .with_fast_boot_en(mcxa_ifr::InverseBigBool::False)
            .with_active_img_prot(mcxa_ifr::ActiveImgProt::FlashAcl)
            .with_fips_sha_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_aes_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_ecdsa_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_drbg_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_cmac_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_kdf_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_dis_nxp_fw(mcxa_ifr::DisNxpFw::DisableNxpSignedSb3Fw)
            .build();
        self.ifr.cmpa.lspi_qflash_cfg0 = LspiQflashCfg0::builder()
            .with_qspi_port(cfg_select! {
                feature = "mcxa5xxevk" => QspiPort::PortA1, // TODO: Make a separate feature for this instead of an evk feature
                _ => QspiPort::PortB1,
            })
            .with_qspi_pwr_hold_time(mcxa_ifr::QspiPwrHoldTime::NoDelay)
            .with_qspi_hold_time(mcxa_ifr::QspiHoldTime::WaitFor500Microseconds)
            .with_qspi_reset_gpio_pin(0u8.into())
            .with_qspi_reset_gpio_port(0u8.into())
            .with_qspi_reset_enable(false)
            .with_qspi_frequency(mcxa_ifr::QspiFrequency::Freq100Mhz)
            .with_qspi_dummy_cycles(0u8.into())
            .with_qspi_auto_probe_en(true)
            .build();
        self.ifr.cmpa.sbl_start_addr = 0x1000_0000;

        self.ifr.cmpa.cc_socu_pin = Self::DEFAULT_CC_SOCU_PIN;
        self.ifr.cmpa.cc_socu_dflt = Self::DEFAULT_CC_SOCU_DFLT;

        self.cmpa_updated = true;

        self
    }
}

pub enum ProvisionError {
    UnexpectedIfrState,
    InvalidLifeCycle,
}

#[derive(Clone, Copy, Debug)]
pub struct CmpaDefaultConfig {
    pub boot_cfg0: BootCfg0,
    pub boot_cfg1: BootCfg1,
    pub rotk_usage: RotkUsage,
    pub dice_csr_key_type: DiceCsrKeyType,
    pub enf_tzm_preset: bool,
    pub sbl_start_addr: u32,
    pub rotkh: [u32; 12],
    pub pqc_rotkh: [u32; 12],
}

// /// Enables secure boot policies in CMPA, configures the provided Root of trust hashes and resets the device to apply the changes.
// /// Must ensure that a signed SBL and signed application(s) are present along with the correct ROTK hashes (BOTH ECDSA and MLDSA) as the provided ROTKH values will be matched at least against the SBL, and more images
// /// if configured by input 'starting_addresses' to do so. The first address must be the Secure Boot Loader (SBL) image, which is expected to be the first image in internal flash at 0x0. The caller must ensure that the SBL and application(s)
// // are signed and present in flash and that the correct ROTKH set is provided as inputs before calling this function, otherwise hashes and secure boot will not be provisioned.
// pub fn configure_rotkh_and_enable_secure_boot_policies_and_reset(
//     mut peri: Peri<'_, peripherals::SGI0>,
//     starting_addresses: &[u32],
//     rotkh: &[u32; 12],
//     pqc_rotkh: &[u32; 12],
// ) -> Result<Infallible, CmpaWriteError> {
//     if is_cmpa_erased() || !cmpa_header_marker_is_valid() {
//         return Err(CmpaWriteError::ConfigError);
//     }
//     if load_lifecycle_from_cfpa() != Some(NbootLifecycleState::Develop) {
//         return Err(CmpaWriteError::LCStateInvalid);
//     }
//     if starting_addresses.is_empty() {
//         return Err(CmpaWriteError::InvalidInput);
//     }
//     if starting_addresses[0] != 0x0000_0000 {
//         return Err(CmpaWriteError::InvalidInput); //The first image must be the Secure Boot Loader (SBL) image, which is expected to be the first image in internal flash at 0x0.
//     }

//     let mut cmpa_page = read_cmpa_page_for_update()?;

//     let rotkh_bytes = unsafe { core::slice::from_raw_parts(rotkh.as_ptr() as *const u8, 48) }; // Little endian representation of the 12 u32 words (4 bytes each) = 48 bytes
//     let pqc_rotkh_bytes = unsafe { core::slice::from_raw_parts(pqc_rotkh.as_ptr() as *const u8, 48) };

//     const MAX_PROVISIONED_IMAGE_SIZE: u32 = 2 * 1024 * 1024; // Max 2MB flash.

//     for &starting_address in starting_addresses {
//         let image_base = starting_address as *const u8;
//         let image_header = unsafe { ImageHeader::from_ptr(image_base, MAX_PROVISIONED_IMAGE_SIZE) }
//             .map_err(|_| CmpaWriteError::InvalidImageSlot)?;
//         let (image_ecdsa_rkth, image_pqc_rkth) = derive_image_rkth_pair(
//             peri.reborrow(),
//             image_base,
//             image_header.extended_header_offset(),
//             image_header.image_length(),
//         )
//         .map_err(|_| CmpaWriteError::HashError)?;
//         if image_ecdsa_rkth.as_bytes() != rotkh_bytes {
//             return Err(CmpaWriteError::RotkhMismatch);
//         }
//         if image_pqc_rkth.as_bytes() != pqc_rotkh_bytes {
//             return Err(CmpaWriteError::RotkhMismatch);
//         }
//     }
//     cmpa_page[CmpaUpdateConfigData::Rotkh.byte_range()].copy_from_slice(rotkh_bytes);
//     cmpa_page[CmpaUpdateConfigData::PqcRotkh.byte_range()].copy_from_slice(pqc_rotkh_bytes);

//     let secure_boot_cfg: u32 = (SecureBootLevel::EcdsaMldsaOnly as u32)            // [1:0]  = 0b11
//         | ((LpWakePolicy::FullAuthentication as u32) << 3)          // [4:3]  = 0b00
//         | ((DiceCsrKeyType::EccP384AndMlDsa87 as u32) << 6)         // [7:6]  Hybrid ECDSA+MLDSA DICE only.
//         | ((CnsaLevel::CnsaTwo as u32) << 8)                        // [9:8]  = 0b10
//         | ((TzmPreset::Enforce as u32) << 10)                       // [11:10] Enforce TZM preset (if present)
//         | (0x3 << 12)                                               // [13:12]= 0b11 fast boot disabled
//         | ((XipImageProtect::WriteProtect as u32) << 14)      // [15:14]= 0b10 write protect without sticky lock //TODO : Maybe XOM, but does that get in way of app authenticating the SBL?
//         | (0x1 << 30); // [31:30] Disable NXP signed FW = b01 (Disable any non provisioned FW)

//     cmpa_page[CmpaUpdateConfigData::SecureBootCfg.byte_range()].copy_from_slice(&secure_boot_cfg.to_le_bytes());
//     write_cmpa_page_to_scratch(&cmpa_page)?;
//     cortex_m::peripheral::SCB::sys_reset()
// }
