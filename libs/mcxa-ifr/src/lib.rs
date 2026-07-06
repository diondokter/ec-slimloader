//! Data layout for the IFR CFPA & CMPA
#![no_std]

use core::ops::{Index, IndexMut};

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
    /// Authentication failure counter.
    ///
    /// This monotonic counter field is incremented by boot ROM on authentication failure during boot, SB3, or debug authentication (Monotonic counter).
    pub err_auth_fail_count: u32,
    /// Tamper event counter.
    ///
    /// This monotonic counter field is incremented by boot ROM whenever the reset cause during boot is detected as
    /// - ITRC reset caused by security sensors or
    /// - WDT 0/1 reset or
    /// - Tamper pin reset.
    pub err_itrc_count: u32,
    pub _reserved1: [u32; 2],
    /// Monotonic erase counter for IPED region 0 through 7.
    ///  
    /// This value is used by bootloader to dynamically compute region IV.
    /// This counter will increment by one, during each erase cycle of the corresponding flash region.
    /// User should not write anything in this field. This field is entirely handled by ROM.
    ///
    /// Final IV value for given region used by IPED for encryption/decryption is computed by ROM bootloader and incorporates device UUID, IPED region number and MCTR_IPED_CTXn.
    /// Application should always use ROM APIs to erase whole Prince region to keep IV consistent.
    pub mctr_iped_ctx: [u32; 8],
    /// Monotonic counter for application use.
    ///
    /// ROM enforces monotonic increment check during CFPA-CMAC page update.
    /// If the new counter value is less than the value in active CFPA page, then the update is rejected and CMAC signing is skipped.
    pub mctr_cust_ctr: [u32; 8],
    /// Monotonic flags for application use.
    ///
    /// Once a bit is set in this field it should be set on sub-sequent updates of the page. ROM emulates One Time Programmable (OTP) bits behavior during CFPA-CMAC update.
    /// Compared to current value, if the new value has bit cleared, then the update is rejected and CMAC signing is skipped.
    pub mflag_cust: [u32; 8],
    pub flash_acl: [FlashAcl; 8],
    pub _reserved2: [u32; 8],
    /// CMAC of hash (SHA384/256) of authenticated image manifest.
    pub sbl_img0_cmac_cache: ReverseArray<u32, 4>,
    /// CMAC of hash (SHA384/256) of authenticated image manifest.
    pub img1_cmac_cache: ReverseArray<u32, 4>,
    pub _reserved3: [u32; 4],
    /// Vector address when waking from power-down states when CMPA.LP_SEC_BOOT is set to 2b'10.
    pub lp_vector_addr: u32,
    pub _reserved4: [u32; 23],
    /// Additional Authentication Data for IPED context
    pub iped_gcm_aad_ctx: [u32; 8],
    pub _reserved5: [u32; 20],
}

impl CFPA {
    pub const DEVCFG_ADDR: u32 = 0x0100_0010;
    pub const SCRATCH_ADDR: u32 = 0x0100_2010;
    pub const ZERO: Self = Self {
        header: Header::ZERO,
        cfpa_page_version: 0,
        image_key_revoke: 0,
        dbg_revoke_vu: DbgRevokeVu::ZERO,
        ee0_fw_version: 0,
        ee1_fw_version: 0,
        ee2_fw_version: 0,
        ee3_fw_version: 0,
        fmc_sbl_fw_version: 0,
        recovery_sb3_version: 0,
        update_sb3_version: 0,
        lp_fw_version: 0,
        rotk_revoke: RotkRevoke::ZERO,
        _reserved0: [0; _],
        err_auth_fail_count: 0,
        err_itrc_count: 0,
        _reserved1: [0; _],
        mctr_iped_ctx: [0; _],
        mctr_cust_ctr: [0; _],
        mflag_cust: [0; _],
        flash_acl: [FlashAcl::ZERO; _],
        _reserved2: [0; _],
        sbl_img0_cmac_cache: ReverseArray::new([0; _]),
        img1_cmac_cache: ReverseArray::new([0; _]),
        _reserved3: [0; _],
        lp_vector_addr: 0,
        _reserved4: [0; _],
        iped_gcm_aad_ctx: [0; _],
        _reserved5: [0; _],
    };
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

/// Active image indicator used in ISP mode to manage SWAP enable before reciev-sb command.
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

/// Select one of the 8 pre-defined access control attributes for the given sector. Access attributes control read, write and execute access along with sticky lock protection.
///
/// After a locked access level is selected, the sub-sequent updates of this field can be done with higher lock level only.
/// - If current sector ACL value (GLBACn index) is greater than the new value, then it is permitted except if the new value is 4 or 5.
///   - 7 (___L) > 6 (__XL) > 3 (R__L) > 2 (R_XL) > 1 (RW_L) > 0 (RWX_)
/// - If current sector ACL value is 0, 4, or 5 then any new value is permitted.
#[bitfield(u32)]
pub struct FlashAcl {
    #[bits(0..=2, rw, stride = 4)]
    pub acl_sec: [AclSec; 8],
}

#[bitenum(u3, exhaustive = true)]
pub enum AclSec {
    /// Default flash memory behavior: R/W unlocked
    Default = 0,
    /// Data flash memory with this setting: R/W + locked
    Data = 1,
    /// ROM with this setting: RX + locked
    RomData = 2,
    /// Data read-only memory (DROM) with this setting: ROM + locked
    DataReadOnly = 3,
    /// ROM with this setting: RX unlocked
    Rom = 4,
    /// XOM with this setting: XOM unlocked
    Xom = 5,
    /// XOM with this setting: XOM + locked
    XomData = 6,
    /// Hidden (no access + locked)
    Hidden = 7,
}

#[repr(C)]
pub struct CMPA {}

impl CMPA {
    pub const DEVCFG_ADDR: u32 = 0x0100_0200;
    pub const SCRATCH_ADDR: u32 = 0x0100_2200;
}

/// An array that stores the elements in reverse order, but is interacted with in normal order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct ReverseArray<T, const N: usize>([T; N]);

impl<T, const N: usize> ReverseArray<T, N> {
    /// Create a reverse array based on an array in the normal order
    pub const fn new(mut data: [T; N]) -> Self {
        data.reverse();
        Self(data)
    }
}

impl<T, const N: usize> Index<usize> for ReverseArray<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[N - 1 - index]
    }
}

impl<T, const N: usize> IndexMut<usize> for ReverseArray<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[N - 1 - index]
    }
}

impl<T, const N: usize> IntoIterator for ReverseArray<T, N> {
    type Item = T;

    type IntoIter = core::iter::Rev<core::array::IntoIter<T, N>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().rev()
    }
}
