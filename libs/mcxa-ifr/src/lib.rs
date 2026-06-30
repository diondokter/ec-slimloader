//! Data layout for the IFR CFPA & CMPA
#![no_std]

use bitbybit::{bitenum, bitfield};

#[repr(C)]
pub struct IFR {
    pub update: Update,
    pub cfpa: CFPA,
    pub cmpa: CMPA,
}

impl IFR {
    const _SIZE_CHECK: () = const {
        if core::mem::size_of::<Update>() as u32 != CFPA::DEVCFG_ADDR - Update::DEVCFG_ADDR {
            panic!("Update has wrong size");
        }
        if core::mem::size_of::<CFPA>() as u32 != CMPA::DEVCFG_ADDR - CFPA::DEVCFG_ADDR {
            panic!("CFPA has wrong size");
        }
        if core::mem::size_of::<CMPA>() as u32 != 0x200 {
            panic!("CMPA has wrong size");
        }
        if core::mem::size_of::<Self>() as u32 != 0x400 {
            panic!("IFR has wrong size");
        }
    };
}

#[repr(C)]
pub struct Update {
    /// Device configuration update request type.
    pub devcfg_upd_type: UpdateType,
    pub _reserved: [u32; 3],
}

impl Update {
    pub const DEVCFG_ADDR: u32 = 0x0100_0000;
    pub const SCRATCH_ADDR: u32 = 0x0100_2000;
}

#[bitenum(u32)]
pub enum UpdateType {
    CfpaUpdated = 0x55504446,
    CmpaUpdated = 0x5550444D,
    Image0Updated = 0x494D4730,
    Image1Updated = 0x494D4731,
    NfpaUpdated = 0x4E465041,
}

#[repr(C)]
pub struct CFPA {
    pub header: Header,
    /// CFPA page version.
    /// ROM enforces monotonic increment check during CFPA-CMAC page update. If the new counter value is less than the value in active CFPA page,
    /// then the update is rejected and CMAC signing is skipped. CFPA_PAGE_VERSION may begin at 0x1 and user may manually increment the value by 1 or use value 0xFF_FFFF and ROM will auto-increment this field.
    pub cfpa_page_version: u32,
    /// Image signing key revocation counter.
    /// This monotonically increasing unary encode counter (e.g., 0x0, 0x1, 0x3, 0x7, 0xF...) field is used by boot ROM to enforce image signing key (ISK) certificate revocation policy.
    /// Boot ROM accepts an ISK certificate as valid only if the "constraint" field in ISK certificate is the same or greater than the ISK revocation counter.
    /// The 32-bit ISK revocation counter is formed using 8 bits of IMAGE_KEY_REVOKE[7:0] field in OTP as MSB bits and lower 24 bits from this field.
    /// ISK certificate validation is done as part of boot, FW update (SB3) and recovery (SB3) image authentication.
    pub image_key_revoke: u32,
    pub dbg_revoke_vu: DbgRevokeVu,
    /// Secure Firmware version. Rollback counter for Execution Environment 0 firmware (Monotonic counter) - CPU0 primary image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee0_fw_version: u32,
    /// Secure Firmware version. Rollback counter for Execution Environment 1 firmware (Monotonic counter) - eg.,  CPU0 NS image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee1_fw_version: u32,
    /// Secure Firmware version. Rollback counter for Execution Environment 2 firmware (Monotonic counter)  - eg., CPU1  secure image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee2_fw_version: u32,
    /// Secure Firmware version. Rollback counter for Execution Environment 3 firmware (Monotonic counter)  - eg., CPU1 NS image low-power wake image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee3_fw_version: u32,
    /// NBU Firmware version. FMC_SBL firmware version (Monotonic counter)
    ///
    /// This monotonic counter field can be used to track the firmware version of NBU partition.
    /// An antirollback check can be enforced during update by using this field and "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_NBU' counter ID parameter, in SB3 file.
    pub fmc_sbl_fw_version: u32,
    /// Secure Firmware version. Rollback counter for SB3 file used for recovery (Monotonic counter)
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub recovery_sb3_version: u32,
    /// Secure Firmware version. Rollback counter for SB3 file used for FW update (Monotonic counter)
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub update_sb3_version: u32,
    /// Secure Firmware version. Rollback counter for low-power wake image (Monotonic counter)
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub lp_fw_version: u32,
    pub rotk_revoke: RotkRevoke,
    pub _reserved0: [u32; 3],
    pub err_auth_fail_count: u32,
    pub err_itrc_count: u32,
    pub _reserved1: [u32; 2],
    pub mctr_iped_ctx0: u32,
    pub mctr_iped_ctx1: u32,
    pub mctr_iped_ctx2: u32,
    pub mctr_iped_ctx3: u32,
    pub mctr_iped_ctx4: u32,
    pub mctr_iped_ctx5: u32,
    pub mctr_iped_ctx6: u32,
    pub mctr_iped_ctx7: u32,
    pub mctr_cust_ctr0: u32,
    pub mctr_cust_ctr1: u32,
    pub mctr_cust_ctr2: u32,
    pub mctr_cust_ctr3: u32,
    pub mctr_cust_ctr4: u32,
    pub mctr_cust_ctr5: u32,
    pub mctr_cust_ctr6: u32,
    pub mctr_cust_ctr7: u32,
    pub mflag_cust_0: u32,
    pub mflag_cust_1: u32,
    pub mflag_cust_2: u32,
    pub mflag_cust_3: u32,
    pub mflag_cust_4: u32,
    pub mflag_cust_5: u32,
    pub mflag_cust_6: u32,
    pub mflag_cust_7: u32,
    pub flash_acl_0_7: u32,
    pub flash_acl_8_15: u32,
    pub flash_acl_16_23: u32,
    pub flash_acl_24_31: u32,
    pub flash_acl_32_39: u32,
    pub flash_acl_40_47: u32,
    pub flash_acl_48_55: u32,
    pub flash_acl_55_63: u32,
    pub _reserved2: [u32; 8],
    pub sbl_img0_cmac_cache_127_96: u32,
    pub sbl_img0_cmac_cache_95_64: u32,
    pub sbl_img0_cmac_cache_63_32: u32,
    pub sbl_img0_cmac_cache_31_0: u32,
    pub img1_cmac_cache_127_96: u32,
    pub img1_cmac_cache_95_64: u32,
    pub img1_cmac_cache_63_32: u32,
    pub img1_cmac_cache_31_0: u32,
    pub _reserved3: [u32; 4],
    pub lp_vector_addr: u32,
    pub _reserved4: [u32; 23],
    pub iped_gcm_aad_ctx0: u32,
    pub iped_gcm_aad_ctx1: u32,
    pub iped_gcm_aad_ctx2: u32,
    pub iped_gcm_aad_ctx3: u32,
    pub iped_gcm_aad_ctx4: u32,
    pub iped_gcm_aad_ctx5: u32,
    pub iped_gcm_aad_ctx6: u32,
    pub iped_gcm_aad_ctx7: u32,
    pub _reserved5: [u32; 20],
}

impl CFPA {
    pub const DEVCFG_ADDR: u32 = 0x0100_0010;
    pub const SCRATCH_ADDR: u32 = 0x0100_2010;
}

#[bitfield(u32)]
pub struct Header {
    /// CFPA Header marker should be set to 0x9635. After this header is set, all non-zero values will take effect; leaving all values set to 0xff will cause undefined behavior. It is recommended to set all values to 0x00 before setting the CFPA header.
    #[bits(16..=31, rw)]
    pub marker: u16,
    /// Inverted value of CFPA_LC_STATE.
    /// This INV_CPFA_LC_STATE ^ CFPA_LC_STATE[7:0] should be 0xFF, otherwise the CPFA_LC_STATE will not be valid.
    #[bits(8..=15, rw)]
    pub inv_cfpa_lc_state: Option<InvLifeCycleState>,
    /// Life cycle state.
    /// This field is used to advance life cycle state to OEM LC states (Develop, Develop2, In-Field, In-field locked, Bricked, FA).
    #[bits(0..=7, rw)]
    pub cfpa_lc_state: Option<LifeCycleState>,
}

/// Below are the allowed values for this field. Use of other values may lead to bricked state.  
#[bitenum(u8)]
pub enum LifeCycleState {
    DevelopState = 0x03,
    Develop2State = 0x07,
    InFieldState = 0x0F,
    InFieldLockedState = 0xCF,
    FaState = 0xA5,
    BrickedStated = 0x5A,
}

#[bitenum(u8)]
pub enum InvLifeCycleState {
    DevelopState = 0xFC,
    Develop2State = 0xF8,
    InFieldState = 0xF0,
    InFieldLockedState = 0x30,
    FaState = 0x5A,
    BrickedStated = 0xA5,
}

#[bitfield(u32)]
pub struct DbgRevokeVu {
    /// Debug certificate revocation counter.
    /// This monotonic counter field is used for revoking debug certificates.
    /// As part of debug authentication, CC_VU field in debug certificate/credential is checked against DCFG_VENDOR_USAGE value.
    /// The 32-bit DCFG_VENDOR_USAGE is formed using upper 16 bit of CMPA.VENDOR_USAGE and lower 16 bits of this field.  
    #[bits(0..=15, rw)]
    pub debug_certificate_revocation_counter: u16,
}

#[bitfield(u32)]
pub struct RotkRevoke {
    /// This field is managed by ROM and user shouldn't set this field. ROM during update of IMGx_CMAC updates this field.
    #[bits(30..=31, r)]
    pub isp_activ_img: Option<IspActivImg>,
    /// Generate Alias key certificate during boot. This option is only applicable if DICE_ALIAS_KEY_UPD is set.
    ///
    /// `false` - Certificate is not generated
    /// `true` - Certificate is generated
    ///
    /// Note, applications should clear this field after initial certificate generation to reduce boot time.
    #[bit(29, rw)]
    pub dice_upd_alias_cert: bool,
    /// Generate alias key pair during boot in addition to CDI.
    /// 0 - Alias key pair is not generated
    /// 1 - Alias key pair is generate
    #[bit(28, rw)]
    pub dice_upd_alias_key: bool,

    /// RoT Key enable. Determines if ROTK can be used during secure boot.
    #[bits(0..=1, rw, stride = 2)]
    pub rotk_en: [RotkEn; 4],
}

/// Active image indicatior used in ISP mode to manage SWAP enable before reciev-sb command.
#[bitenum(u2)]
pub enum IspActivImg {
    SwapIsDisabled = 0b00,
    SwapIsEnabled = 0b01,
    SblImageI = 0b10,
}

#[bitenum(u2, exhaustive = true)]
pub enum RotkEn {
    Enabled1 = 0b00,
    Enabled2 = 0b01,
    Revoked1 = 0b10,
    Revoked2 = 0b11,
}

#[repr(C)]
pub struct CMPA {}

impl CMPA {
    pub const DEVCFG_ADDR: u32 = 0x0100_0200;
    pub const SCRATCH_ADDR: u32 = 0x0100_2200;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
