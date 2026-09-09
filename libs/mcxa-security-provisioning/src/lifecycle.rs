use core::mem;

// MCXA configuration flash layout (CFG vs SCRATCH)
//
// NOTE:
// - Reads should use the CFG area.
// - Updates should be staged into the SCRATCH area (then committed by the proper
//   ROM/programming flow; this module only provides addresses and helpers).
//
// CFG (read):
//   CFPA  0x0100_0000 - 0x0100_01FF
//   CMPA  0x0100_0200 - 0x0100_03FF
//   CMPA customer-defined 0x0100_0400 - 0x0100_17FF
//
// SCRATCH (write staging): MUST use secure alias.
//   CFPA  0x1100_2000 - 0x1100_21FF
//   CMPA  0x1100_2200 - 0x1100_23FF
//   CMPA customer-defined 0x1100_2400 - 0x1100_37FF

const CMPA_HEADER_MARKER: u16 = 0x5963;
const CFPA_HEADER_MARKER: u16 = 0x9635;
// Any field reading back this value should be treated as "not provisioned" rather than a real configuration.
const ERASED_WORD: u32 = 0xFFFF_FFFF;

// CFG bases (use for reading)
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IFRConfigAreaBase {
    Cfpa = 0x0100_0000,
    Cmpa = 0x0100_0200,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IFRPage {
    Cfpa,
    Cmpa,
    CmpaCustDefined,
    CmpaAll,
}

impl IFRPage {
    #[inline(always)]
    pub const fn start_offset(self) -> u32 {
        match self {
            Self::Cfpa => 0x0000,
            Self::Cmpa => 0x0200,
            Self::CmpaCustDefined => 0x0400,
            Self::CmpaAll => 0x0200, // start of CMPA, used for operations that cover the entire CMPA including customer-defined area.
        }
    }

    #[inline(always)]
    pub const fn end_offset_inclusive(self) -> u32 {
        match self {
            Self::Cfpa => 0x01FF,
            Self::Cmpa => 0x03FF,
            Self::CmpaCustDefined => 0x17FF,
            Self::CmpaAll => 0x17FF, // end of CMPA customer-defined area, used for operations that cover the entire CMPA including customer-defined area.
        }
    }

    #[inline(always)]
    pub const fn byte_len(self) -> usize {
        (self.end_offset_inclusive() - self.start_offset() + 1) as usize
    }
}

// RoTKH locations in CMPA (absolute addresses provided):
// - ROTKH @ 0x0100_0260 (i.e. IFRConfigAreaBase::Cmpa + 0x60)
// - PQC_ROTKH @ 0x0100_02C0 (i.e. IFRConfigAreaBase::Cmpa + 0xC0)
// CMPA secure-boot related fields (MCXA):
// Base is 0x0100_0200 and offsets below come from the reference table.
// Unused fields from the reference table are omitted here but can be added as needed for future features.
// const CMPA_BOOT_LED_STATUS: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0008; // 0x0100_0208
// const CMPA_BOOT_TIMERS: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x000C; // 0x0100_020C

#[inline(always)]
pub fn load_cmpa_boot_cfg0() -> u32 {
    const CMPA_BOOT_CFG0: u32 = IFRConfigAreaBase::Cmpa as u32; // 0x0100_0200
    unsafe { core::ptr::read_volatile(CMPA_BOOT_CFG0 as *const u32) }
}

// Erased scan: OR-accumulate, no early exit.
pub fn cmpa_erased_deviation() -> u32 {
    // NOTE: ifr_verify_erase_page is NOT a read-only check; it erases then verifies (destructive).
    // Therefore it cannot be used to check erased state of CFG area (protected by ROM).
    // read_volatile is the correct approach for a non-destructive erased check.
    let base = IFRConfigAreaBase::Cmpa as u32;
    let word_count = IFRPage::Cmpa.byte_len() / core::mem::size_of::<u32>();
    let mut d = 0u32;
    for i in 0..word_count {
        d |= unsafe { core::ptr::read_volatile((base + i as u32 * 4) as *const u32) } ^ ERASED_WORD;
    }
    d
}

pub fn is_cmpa_erased() -> bool {
    cmpa_erased_deviation() == 0
}

pub fn is_cfpa_erased() -> bool {
    let base = IFRConfigAreaBase::Cfpa as u32;
    let word_count = IFRPage::Cfpa.byte_len() / core::mem::size_of::<u32>();
    // Skip UPD_TYPE/UPD_PARAM (offsets 0x00-0x0C) and the header word itself (offset 0x10).
    // Check from PAGE_VERSION (offset 0x14, word index 5) onwards.
    // Also skip ROM-maintained error counters — written autonomously by the ROM after auth
    // failures and/or Intrusion detects ; must not be treated as "provisioned config".
    // ERR_AUTH_FAIL_COUNT at absolute offset 0x50, ERR_ITRC_COUNT at absolute offset 0x54.
    const FIRST_WORD_AFTER_HEADER: usize = (0x0010_usize / core::mem::size_of::<u32>()) + 1; // 5
    const ERR_AUTH_FAIL_COUNT_IDX: usize = 0x50 / core::mem::size_of::<u32>(); // 20
    const ERR_ITRC_COUNT_IDX: usize = 0x54 / core::mem::size_of::<u32>(); // 21
    for i in FIRST_WORD_AFTER_HEADER..word_count {
        if i == ERR_AUTH_FAIL_COUNT_IDX || i == ERR_ITRC_COUNT_IDX {
            continue;
        }
        let addr = base + (i as u32 * 4);
        let val = unsafe { core::ptr::read_volatile(addr as *const u32) };
        if val != ERASED_WORD {
            return false;
        }
    }
    true
}

#[inline(always)]
fn load_cmpa_header_marker() -> u16 {
    (load_cmpa_boot_cfg0() >> 16) as u16
}

#[inline(always)]
pub fn cmpa_header_marker_is_valid() -> bool {
    // CMPA BOOT_CFG0 header marker semantics (MCXA):
    // Marker should be set to 0x5963. After this header is set, all non-zero values will take effect;
    // leaving all settings at 0xFF will cause undefined behavior. It is recommended to set all values
    // to 0x00 before setting the CMPA header marker.
    //
    // Layout assumed consistent with CFPA header marker usage: marker stored in bits [31:16].
    load_cmpa_header_marker() == CMPA_HEADER_MARKER
}

// The following CMPA fields are defined in the reference table but not yet used in this module; they can be added as needed for future features:
// const CMPA_ERR_LOG_ADDR: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x005C; // 0x0100_025C
// const CMPA_CUST_MK_SK_KEY_BLOB_START: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0090; // 0x0100_0290
// const CMPA_CUST_MK_SK_KEY_BLOB_WORDS: usize = 12;

#[inline(always)]
fn load_cmpa_secure_boot_cfg() -> u32 {
    const CMPA_SECURE_BOOT_CFG: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0050; // 0x0100_0250
    unsafe { core::ptr::read_volatile(CMPA_SECURE_BOOT_CFG as *const u32) }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecureBootLevel {
    AllAllowed = 0,     // b00
    CrcOrSigned = 1,    // b01
    SignedOnly = 2,     // b10 (CMAC or ECDSA)
    EcdsaMldsaOnly = 3, // b11
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SecureBootState {
    HybridEnforced,
    Classical,
    Disabled,
    Unknown,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CnsaLevel {
    NotEnforced = 0, // b00 (non-CNSA or no enforcement)
    CnsaOne = 1,     // b01: CNSA1.0 (ECDSA p384 and SHA-384, AES-256)
    CnsaTwo = 2,     // b10 or b11 (hybrid PQC with ECDSA-384 and MLDSA-87, SHA-384, ML-KEM, AES-256)
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpWakePolicy {
    FullAuthentication = 0, // b00 (same as normal secure boot, applies to low-power wake too)
    CrcOnly = 1,            // b01 (CRC check only for LP wake, full auth for normal boot)
    Jump = 2,               // b10 (jump to CFPA LP wake address without authentication)
    Cmac = 3,               // b11 (CMAC auth for LP wake, full hybrid auth for normal boot)
}

/// Active image protection mode (CMPA.SECURE_BOOT_CFG bits [15:14]).
/// Controls MBC (Memory Block Controller) protection applied to the active (XIP) image region.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XipImageProtect {
    /// Active image protection via CFPA flash ACL settings.
    FlashAclSettingCfpa = 0,
    /// Write protect active image area with sticky lock (GLBAC2)
    WriteProtectSticky = 1,
    /// Write protect active image area without sticky lock
    WriteProtect = 2,
    /// XOM (eXecute-Only Memory) protect active image area with sticky lock
    XomSticky = 3,
}

/// Helper function to load the secure boot enforcement level from CMPA and interpret it according to the reference table for MCXA.
pub fn secure_boot_level() -> SecureBootLevel {
    // CMPA.SECURE_BOOT_CFG: SEC_BOOT_EN is a 2-bit field in bits [1:0].
    match cmpa_secure_boot_cfg().sec_boot_en {
        0 => SecureBootLevel::AllAllowed,
        1 => SecureBootLevel::CrcOrSigned,
        2 => SecureBootLevel::SignedOnly,
        3 => SecureBootLevel::EcdsaMldsaOnly,
        _ => SecureBootLevel::AllAllowed, // treat invalid values as most permissive that way will be caught by policy validation.
    }
}

pub fn secure_boot_state() -> SecureBootState {
    // Fully erased CMPA = brand new device with no security configured
    if is_cmpa_erased() {
        return SecureBootState::Disabled;
    }
    // Invalid header but not fully erased = partially provisioned or corrupt
    if !cmpa_header_marker_is_valid() {
        return SecureBootState::Unknown;
    }

    match secure_boot_level() {
        SecureBootLevel::EcdsaMldsaOnly => SecureBootState::HybridEnforced,
        SecureBootLevel::SignedOnly => SecureBootState::Classical,
        SecureBootLevel::AllAllowed | SecureBootLevel::CrcOrSigned => SecureBootState::Disabled,
    }
}

/// Helper function that returns true only if secure boot is strictest with hybrid ECDSA+MLDSA AND IFR region not left in an unprovisioned/erased state that could cause undefined behavior.
pub fn hybrid_secure_boot_enforced() -> bool {
    matches!(secure_boot_state(), SecureBootState::HybridEnforced)
}

/// Helper function that returns true if CNSA2.0 is enfocred via CMPA SEC_BOOT_CFG.ENF_CNSA field, and false if not enforced or if CMPA is erased/unprovisioned.
pub fn cnsa_enforced() -> bool {
    let cnsa_level = match cmpa_secure_boot_cfg().enf_cnsa {
        0 => CnsaLevel::NotEnforced,
        1 => CnsaLevel::CnsaOne,
        2 | 3 => CnsaLevel::CnsaTwo,
        _ => CnsaLevel::NotEnforced,
    };
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && cnsa_level == CnsaLevel::CnsaTwo
}

/// Helper function that returns true if fast boot is enabled via CMPA SEC_BOOT_CFG.FAST_BOOT_EN field, and false if disabled or if CMPA is erased/unprovisioned.
pub fn fast_boot_enabled() -> bool {
    // Fast boot is enabled when FAST_BOOT_EN field is 0b00, and disabled otherwise (full auth flow required)
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && cmpa_secure_boot_cfg().fast_boot_en == 0
}

/// Helper function that returns true if low-power wake authentication is enforced via CMPA SEC_BOOT_CFG.LP_SEC_BOOT field, and false if not enforced or if CMPA is erased/unprovisioned.
pub fn low_power_authentication_enforced() -> bool {
    let lp_wake_policy = match cmpa_secure_boot_cfg().lp_sec_boot {
        0 => LpWakePolicy::FullAuthentication,
        1 => LpWakePolicy::CrcOnly,
        2 => LpWakePolicy::Jump,
        3 => LpWakePolicy::Cmac,
        _ => LpWakePolicy::Cmac,
    };
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && lp_wake_policy == LpWakePolicy::FullAuthentication
}

// CMPA.SECURE_BOOT_CFG decoder (MCXA)
//
// 2-bit fields, with 1 bit at bit 2 and bit 5:
// - [1:0]   SEC_BOOT_EN
// - [2]     (reserved)
// - [4:3]   LP_SEC_BOOT
// - [5]     (reserved)
// - [7:6]   DICE_CSR_KEY_TYPE
// - [9:8]   ENF_CNSA
// - [11:10] ENF_TZM_PRESET
// - [13:12] FAST_BOOT_EN
// - [15:14] ACTIVE_IMG_PROT
// - [17:16] FIPS_SHA_STEN
// - [19:18] FIPS_AES_STEN
// - [21:20] FIPS_ECDSA_STEN
// - [23:22] FIPS_DRBG_STEN
// - [25:24] FIPS_CMAC_STEN
// - [27:26] FIPS_KDF_STEN
// - [29:28] Reserved (2-bit)
// - [31:30] DIS_NXP_FW

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CmpaSecureBootCfgDecode {
    raw: u32,
    sec_boot_en: u8,
    lp_sec_boot: u8,
    dice_csr_key_type: u8,
    enf_cnsa: u8,
    enf_tzm_preset: u8,
    fast_boot_en: u8,
    active_img_prot: u8,
    fips_sha_sten: u8,
    fips_aes_sten: u8,
    fips_ecdsa_sten: u8,
    fips_drbg_sten: u8,
    fips_cmac_sten: u8,
    fips_kdf_sten: u8,
    dis_nxp_fw: u8,
}

#[inline(always)]
fn cmpa_secure_boot_cfg() -> CmpaSecureBootCfgDecode {
    let raw = load_cmpa_secure_boot_cfg();

    CmpaSecureBootCfgDecode {
        raw,
        sec_boot_en: (raw & 0x3) as u8,
        // bit 2 is a hole
        lp_sec_boot: ((raw >> 3) & 0x3) as u8,
        // bit 5 is a hole
        dice_csr_key_type: ((raw >> 6) & 0x3) as u8,
        enf_cnsa: ((raw >> 8) & 0x3) as u8,
        enf_tzm_preset: ((raw >> 10) & 0x3) as u8,
        fast_boot_en: ((raw >> 12) & 0x3) as u8,
        active_img_prot: ((raw >> 14) & 0x3) as u8,
        fips_sha_sten: ((raw >> 16) & 0x3) as u8,
        fips_aes_sten: ((raw >> 18) & 0x3) as u8,
        fips_ecdsa_sten: ((raw >> 20) & 0x3) as u8,
        fips_drbg_sten: ((raw >> 22) & 0x3) as u8,
        fips_cmac_sten: ((raw >> 24) & 0x3) as u8,
        fips_kdf_sten: ((raw >> 26) & 0x3) as u8,
        // bits 28-29 are reserved
        dis_nxp_fw: ((raw >> 30) & 0x3) as u8,
    }
}

// Additional CMPA constants
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpaUpdateConfigData {
    BootCfg0,
    BootCfg1,
    LSpiCfg0,
    CcSocuPin,
    CcSocuDflt,
    SecureBootCfg,
    RotkUsage,
    SblStartAddr,
    Rotkh,
    PqcRotkh,
}

impl CmpaUpdateConfigData {
    #[inline(always)]
    pub const fn byte_len(self) -> usize {
        const RKTH_WORDS: usize = 12;
        match self {
            Self::BootCfg0
            | Self::BootCfg1
            | Self::CcSocuPin
            | Self::CcSocuDflt
            | Self::SecureBootCfg
            | Self::RotkUsage
            | Self::SblStartAddr
            | Self::LSpiCfg0 => mem::size_of::<u32>(),
            Self::Rotkh | Self::PqcRotkh => RKTH_WORDS * mem::size_of::<u32>(),
        }
    }

    #[inline(always)]
    pub const fn byte_offset(self) -> usize {
        match self {
            Self::BootCfg0 => 0x00,
            Self::BootCfg1 => 0x04,
            Self::LSpiCfg0 => 0x10,
            Self::CcSocuPin => 0x40,
            Self::CcSocuDflt => 0x44,
            Self::SecureBootCfg => 0x50,
            Self::RotkUsage => 0x54,
            Self::SblStartAddr => 0x58,
            Self::Rotkh => 0x60,
            Self::PqcRotkh => 0xC0,
        }
    }

    #[inline(always)]
    pub const fn byte_range(self) -> core::ops::Range<usize> {
        let start = self.byte_offset();
        start..(start + self.byte_len())
    }
}

// CFPA offsets (MCXA) — CFG region @ 0x0100_0000
//
// 0x00 UPD_TYPE
// 0x04 UPD_PARAM0
// 0x08 UPD_PARAM1
// 0x0C UPD_PARAM2
// 0x10 Header word (marker + INV_LC + LC)
// 0x14 CFPA_PAGE_VERSION
// 0x18 IMAGE_KEY_REVOKE
// 0x1C DBG_REVOKE_VU
// 0x20.. FW version words
// 0x40 ROTK_REVOKE
// 0x50 ERR_AUTH_FAIL_COUNT
// 0x54 ERR_ITRC_COUNT

// The following CFPA fields are documented here for reference and can be localized when
// readers/writers are added for them:
// const CFPA_PAGE_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0014;
// const CFPA_DBG_REVOKE_VU: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x001C;
// const CFPA_EE0_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0020;
// const CFPA_EE1_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0024;
// const CFPA_EE2_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0028;
// const CFPA_EE3_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x002C;
// const CFPA_RECOVERY_SB3_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0034;
// const CFPA_UPDATE_SB3_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0038;
// const CFPA_LP_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x003C;

// Additional CMPA constants

#[inline(always)]
fn cfpa_header_word_is_valid(header: u32) -> bool {
    let marker = (header >> 16) as u16;
    if marker != CFPA_HEADER_MARKER {
        return false;
    }

    let lifecycle = (header & 0xFF) as u8;
    let inv_lifecycle = ((header >> 8) & 0xFF) as u8;
    inv_lifecycle == (!lifecycle)
}

pub fn load_cfpa_header_word_raw() -> u32 {
    const CFPA_HEADER: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0010;
    unsafe { core::ptr::read_volatile(CFPA_HEADER as *const u32) }
}

/// Load CFPA header word and check validity, returning None if header is invalid (e.g. incorrect marker, which could indicate unprovisioned/partially provisioned state or corruption). This is used as a prerequisite check for other CFPA fields since the header validity is an indicator of whether the CFPA contents can be trusted.
#[inline(always)]
pub fn load_cfpa_header_word() -> Option<u32> {
    let h = load_cfpa_header_word_raw();
    cfpa_header_word_is_valid(h).then_some(h)
}

/// Load lifecycle state from CFPA header word: returns the decoded lifecycle state if the header is valid, or None if the header is invalid (e.g. incorrect marker, which could indicate unprovisioned/partially provisioned state or corruption). This is used as a prerequisite check for other CFPA fields since the header validity is an indicator of whether the CFPA contents can be trusted.
/// Returns the decoded lifecylce to be used by the ROM API NBOOT functions, which uses different format that what is encoded in CFPA.
/// The CFPA header encodes lifecycle in the lowest byte, with a separate inverted lifecycle byte as a validity check, and a 2 byte header marker in upper half of the word.
/// The NBOOT ROM API expects a full 32-bit raw value where the lower half is the lifecycle raw value and the upper half is the !inverse. Even in a brand new unit, LC should be Develop.
pub fn load_lifecycle_from_cfpa() -> Option<NbootLifecycleState> {
    let header = load_cfpa_header_word()?;
    NbootLifecycleDiscriminator::from_raw(header as u8).map(NbootLifecycleDiscriminator::state)
}

// Lifecycle state codes (low-byte discriminators) and full CFPA LC_STATE values.
// Per Table 18 (Life Cycle States): LC_STATE is a u32 like 0x9635_FC03.
// Some call sites only carry the low-byte discriminator (e.g. 0x03 for Develop),
// so we keep both representations.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootLifecycleDiscriminator {
    Develop = 0x03,
    Develop2 = 0x07,
    InField = 0x0F,
    InFieldLocked = 0xCF,
    OemFieldReturn = 0x1F,
    FailureAnalysis = 0x3F,
    Bricked = 0x5A,
}

impl NbootLifecycleDiscriminator {
    #[inline(always)]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0x03 => Some(Self::Develop),
            0x07 => Some(Self::Develop2),
            0x0F => Some(Self::InField),
            0xCF => Some(Self::InFieldLocked),
            0x1F => Some(Self::OemFieldReturn),
            0x3F => Some(Self::FailureAnalysis),
            0x5A => Some(Self::Bricked),
            _ => None,
        }
    }

    #[inline(always)]
    pub const fn state(self) -> NbootLifecycleState {
        match self {
            Self::Develop => NbootLifecycleState::Develop,
            Self::Develop2 => NbootLifecycleState::Develop2,
            Self::InField => NbootLifecycleState::InField,
            Self::InFieldLocked => NbootLifecycleState::InFieldLocked,
            Self::OemFieldReturn => NbootLifecycleState::OemFieldReturn,
            Self::FailureAnalysis => NbootLifecycleState::FailureAnalysis,
            Self::Bricked => NbootLifecycleState::Bricked,
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootLifecycleState {
    Develop = 0x9635_FC03,
    Develop2 = 0x9635_F807,
    InField = 0x9635_F00F,
    InFieldLocked = 0x9635_30CF,
    OemFieldReturn = 0x9635_E01F,
    FailureAnalysis = 0x9635_C03F,
    Bricked = 0x9635_A55A,
}
impl TryFrom<u32> for NbootLifecycleState {
    type Error = ();

    fn try_from(raw: u32) -> Result<Self, Self::Error> {
        match raw {
            0x9635_FC03 => Ok(Self::Develop),
            0x9635_F807 => Ok(Self::Develop2),
            0x9635_F00F => Ok(Self::InField),
            0x9635_30CF => Ok(Self::InFieldLocked),
            0x9635_E01F => Ok(Self::OemFieldReturn),
            0x9635_C03F => Ok(Self::FailureAnalysis),
            0x9635_A55A => Ok(Self::Bricked),
            _ => Err(()),
        }
    }
}

impl NbootLifecycleState {
    const fn discriminator(self) -> NbootLifecycleDiscriminator {
        match self {
            Self::Develop => NbootLifecycleDiscriminator::Develop,
            Self::Develop2 => NbootLifecycleDiscriminator::Develop2,
            Self::InField => NbootLifecycleDiscriminator::InField,
            Self::InFieldLocked => NbootLifecycleDiscriminator::InFieldLocked,
            Self::OemFieldReturn => NbootLifecycleDiscriminator::OemFieldReturn,
            Self::FailureAnalysis => NbootLifecycleDiscriminator::FailureAnalysis,
            Self::Bricked => NbootLifecycleDiscriminator::Bricked,
        }
    }

    pub const fn nboot_soc_lifecycle(self) -> u32 {
        let discriminator = self.discriminator() as u16;
        (((!discriminator) as u32) << 16) | (discriminator as u32)
    }

    /// Returns a monotonic rank for forward-only progression checks.
    /// Higher rank = further along the lifecycle.
    const fn rank(self) -> u8 {
        match self {
            Self::Develop => 0,
            Self::Develop2 => 1,
            Self::InField => 2,
            Self::InFieldLocked => 2, // Same rank as InField since locking is not a lifecycle progression.
            Self::OemFieldReturn => 3,
            Self::FailureAnalysis => 4,
            Self::Bricked => 5,
        }
    }

    /// Returns true if advancing to `next` is a valid forward progression (no regressions, no same state).
    pub const fn can_advance_to(self, next: Self) -> bool {
        next.rank() >= self.rank() && (self as u32 != next as u32)
    }
}
