use std::{fs, process::{Command, Stdio}};

use anyhow::Context;
use arbitrary_int::{u3, u5};
use mcxa_ifr::*;

fn main() -> anyhow::Result<()> {
    println!("Generating IFR");
    let ifr = create_working_from_artifacts();
    let ifr_bytes = unsafe { std::mem::transmute::<IFR, [u8; 1024]>(ifr) };
    fs::write("ifr.bin", ifr_bytes).context("writing IFR to disk")?;

    Command::new("blhost")
        .args(["-u", "0x1fc9,0x002a"])
        .args(["flash-erase-region", "0x01002000", "8192"])
        .stdout(Stdio::inherit())
        .output()
        .context("running blhost to erase")?;

    Command::new("blhost")
        .args(["-u", "0x1fc9,0x002a"])
        .args(["write-memory", "0x01002000", "ifr.bin"])
        .stdout(Stdio::inherit())
        .output()
        .context("running blhost to write")?;

    Command::new("blhost")
        .args(["-u", "0x1fc9,0x002a"])
        .args(["reset"])
        .stdout(Stdio::inherit())
        .output()
        .context("running blhost to reset")?;

    Ok(())
}

fn create_working_from_artifacts() -> IFR {
    const ECDSA_P384_PUB_SHA: &[u8; 48] =
        include_bytes!("../../../bootloader-tool/artifacts-mcxa/ecdsa_p384_pub_sha.bin");
    const MLDSA87_PUBLIC_SHA: &[u8; 48] =
        include_bytes!("../../../bootloader-tool/artifacts-mcxa/mldsa87_public_sha.bin");

    // When in doubt, set bits to zero. Everything set to one contains UB according to the docs
    IFR {
        update: Update {
            devcfg_upd_type: UpdateType::CmpaUpdated, // Update both CFPA + CMPA
            _reserved: [0; _],
        },
        cfpa: CFPA {
            header: Header::builder()
                .with_marker(0x9635)
                .with_cfpa_lc_state(LifeCycleState::DevelopState) // IMPORTANT! Keep at develop! Otherwise you risk bricking the device
                .with_inv_cfpa_lc_state(InvLifeCycleState::DevelopState)
                .build(),
            cfpa_page_version: 1,
            image_key_revoke: 0,
            dbg_revoke_vu: DbgRevokeVu::builder()
                .with_debug_certificate_revocation_counter(u16::MAX)
                .build(),
            ee0_fw_version: 0,
            ee1_fw_version: 0,
            ee2_fw_version: 0,
            ee3_fw_version: 0,
            fmc_sbl_fw_version: 0,
            recovery_sb3_version: 0,
            update_sb3_version: 0,
            lp_fw_version: 0,
            rotk_revoke: RotkRevoke::builder()
                .with_dice_upd_alias_cert(false)
                .with_dice_upd_alias_key(false)
                .with_rotk_en([RotkEn::Enabled1; _]) // All certs enabled
                .build(),
            _reserved0: [0; _],
            err_auth_fail_count: 0,
            err_itrc_count: 0,
            _reserved1: [0; _],
            mctr_iped_ctx: [0; _],
            mctr_cust_ctr: [0; _],
            mflag_cust: [0; _],
            flash_acl: [FlashAcl::builder().with_acl_sec([AclSec::Data; _]).build(); _],
            _reserved2: [0; _],
            sbl_img0_cmac_cache: ReverseArray::new([0; _]),
            sbl_img1_cmac_cache: ReverseArray::new([0; _]),
            _reserved3: [0; _],
            lp_vector_addr: 0,
            _reserved4: [0; _],
            iped_gcm_aad_ctx: [0; _],
            _reserved5: [0; _],
        },
        cmpa: CMPA {
            boot_cfg0: BootCfg0::builder()
                .with_marker(0x5963)
                .with_boot_speed(BootSpeed::MdMode)
                .with_rec_boot_en(BigBool::False)
                .with_rec_lspi(false)
                .with_rec_flexspi(false)
                .with_eflash_dual_en(false)
                .with_eflash_booten(false)
                .with_iflash_dual_en(false)
                .with_iflash_booten(true)
                .build(),
            boot_cfg1: BootCfg1::builder()
                .with_ext_cmpa_32b_size(16)
                .with_flash_remap_size(u5::new(0))
                .with_isp_ft_entry(EntryEnabled::Enabled)
                .with_isp_api_entry(EntryEnabled::Enabled)
                .with_isp_dm_entry(EntryEnabled::Enabled)
                .with_isp_pin_entry(EntryEnabled::Enabled)
                .with_isp_usb_en(true)
                .with_isp_i2c_en(true)
                .with_isp_can_en(true)
                .with_isp_spi_en(true)
                .with_isp_uart_en(true)
                .build(),
            boot_led_status: BootLedStatus::builder()
                .with_boot_fail_led(Gpio::builder().with_port(u3::new(2)).with_pin(u5::new(14)).build()) // Red
                .with_isp_boot_led(Gpio::builder().with_port(u3::new(2)).with_pin(u5::new(22)).build()) // Green
                .with_rec_boot_led(Gpio::builder().with_port(u3::new(2)).with_pin(u5::new(23)).build()) // Blue
                .build(),
            boot_timers: BootTimers::builder()
                .with_wdog_timeout_count(0)
                .with_powerdown_timeout_secs(0)
                .build(),
            lspi_qflash_cfg0: LspiQflashCfg0::new_with_raw_value(0),
            lspi_qflash_cfg1: LspiQflashCfg1::new_with_raw_value(0),
            lspi_flash_cfg0: 0,
            lspi_flash_cfg1: 0,
            isp_uart_cfg: 0,
            isp_i2c_cfg: 0,
            isp_can_cfg: 0,
            isp_spi_cfg0: 0,
            isp_spi_cfg1: 0,
            isp_usb_id: 0,
            isp_usb_cfg: 0,
            isp_misc_cfg: 0,
            cc_socu_pin: 0,
            cc_socu_dflt: 0,
            vendor_usage: 0,
            _reserved0: [0; _],
            secure_boot_cfg: SecureBootCfg::builder()
                .with_dis_nxp_fw(DisNxpFw::AllowNxpSignedSb3Fw1)
                .with_fips_kdf_sten(SelfTestEnable::NotIncluded)
                .with_fips_cmac_sten(SelfTestEnable::NotIncluded)
                .with_fips_drbg_sten(SelfTestEnable::NotIncluded)
                .with_fips_ecdsa_sten(SelfTestEnable::NotIncluded)
                .with_fips_aes_sten(SelfTestEnable::NotIncluded)
                .with_fips_sha_sten(SelfTestEnable::NotIncluded)
                .with_active_img_prot(ActiveImgProt::FlashAcl)
                .with_fast_boot_en(InverseBigBool::True)
                .with_enf_tzm_preset(BigBool::False)
                .with_enf_cnsa(EnfCnsa::Performance)
                .with_dice_csr_key_type(DiceCsrKeyType::EccP384)
                .with_lp_sec_boot(LpSecBoot::Cold)
                .with_sec_boot_en(SecBootEn::AllAllowed)
                .build(),
            rotk_usage: RotkUsage::builder()
                .with_dice_inc_nxp_cfg(false)
                .with_dice_inc_cust_cfg(false)
                .with_dice_inc_nxp_field_cfg(false)
                .with_disable_dice(true)
                .with_rotk_usage([RotkUsageVal::All; _])
                .build(),
            sbl_start_addr: 0,
            err_log_addr: 0x2005_0000,
            rotkh: ECDSA_P384_PUB_SHA
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_be_bytes(*c)) // TODO: Or should this be LE?
                .collect(),
            cust_mk_sk_key_blob: [0; _],
            pqc_rotkh: MLDSA87_PUBLIC_SHA
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_be_bytes(*c)) // TODO: Or should this be LE?
                .collect(),
            quick_gpio: [QuickGpio::new_with_raw_value(0); _],
            _reserved1: [0; _],
            iped: [Iped::new_with_raw_value(0); _],
            _reserved2: [0; _],
            dice_x509_sram_buf_len: DiceX509SramBufLen::new_with_raw_value(0),
            dice_x509_ecdsa_sram_addr: 0,
            dice_x509_mldsa_sram_addr: 0,
            dice_alias_key_sram_addr: 0,
            mldsa_cert_temp_addr: 0,
            mldsa_cert_temp_hash: ReverseArray::new([0; _]),
        },
    }
}
