use core::ffi::c_char;

use ec_slimloader::BootError;

pub fn rom_api() -> RomApiRaw {
    const ROM_API_BASE: *const RomApiRaw = 0x1303_D800 as _; // from MCXA Reference Manual.
    unsafe { *ROM_API_BASE }
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RomApiRaw {
    // NXP usage: uint32_t arg = ...; g_bootloaderTree->runBootloader(&arg);
    // The ROM API takes a pointer to the argument word (NULL is allowed for default behavior).
    pub run_bootloader: extern "C" fn(arg: *const ()),
    // Flash driver interface table.
    pub flash_api: *const (),
    pub kb_api: *const kb_interface,
    pub nboot_api: *const (),
    pub flex_spi_api: *const (),
    pub spi_flash_api: *const (),
    pub version: StandardVersion,
    pub copyright: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StandardVersion {
    pub bugfix: u8,
    pub minor: u8,
    pub major: u8,
    pub name: u8,
}

/// Interface for bootloader API functions.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct kb_interface {
    pub kb_init: extern "C" fn(session: *mut *mut kb_session_ref_t, options: *const kb_options_t) -> Status,
    pub kb_deinit: extern "C" fn(session: *mut kb_session_ref_t) -> Status,
    pub kb_execute: extern "C" fn(session: *mut kb_session_ref_t, data: *const u8, data_length: u32) -> Status,
}

/// Memory region definition
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct kb_region_t {
    pub address: u32,
    pub length: u32,
}
/// Details of the operation to be performed by the ROM.
///
/// The [kRomAuthenticateImage] operation requires the entire signed image to be
/// available to the application.
///
#[repr(C, u32)]
#[derive(Debug, Clone, Copy)]
pub enum KbOperation {
    /// Authenticate a signed image
    KRomAuthenticateImage {
        profile: u32,
        min_build_number: u32,
        max_image_length: u32,
        user_rhk: *mut u32,
    } = 1,
    /// Load SB file
    KRomLoadImage {
        profile: u32,
        min_build_number: u32,
        override_sbboot_section_id: u32,
        user_sbkek: *mut u32,
        region_count: u32,
        regions: *const kb_region_t,
    } = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct kb_options_t {
    /// Should be set to [kKbootApiVersion]
    pub version: u32,
    /// Caller-provided buffer used by Kboot
    pub buffer: *mut u8,
    pub buffer_length: u32,
    pub op: KbOperation,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct kb_buffer_desc_t {
    pub buf: *mut u8,
    pub len: u32,
    pub allocated: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct kb_session_ref_t {
    pub options: kb_options_t,
    pub buffer_desc: kb_buffer_desc_t,
    pub op_context: *mut (),
}

#[repr(transparent)]
#[must_use]
pub struct Status(pub u32);

impl Status {
    /// API succesfully finished its operation
    pub const K_STATUS_SUCCESS: u32 = 0;
    /// API failed and it could not finish its operation
    pub const K_STATUS_FAIL: u32 = 1;
    /// API cannot proceed because the input argument value is invalid
    pub const K_STATUS_INVALID_ARGUMENT: u32 = 4;
    /// API needs to be called again with more data (next data chunk)
    pub const K_STATUS_ROM_LDR_DATA_UNDERRUN: u32 = 10109;
    /// API finished its execution but the jump to user code returned
    pub const K_STATUS_ROM_LDR_JUMP_RETURNED: u32 = 10110;
    /// New firmware version is lesser than the present one
    pub const K_STATUS_ROM_LDR_ROLLBACK_BLOCKED: u32 = 10115;
    /// Returned by kb_execute; Call of kb_finish is needed to do the requested jump
    pub const K_STATUS_ROM_LDR_PENDING_JUMP_COMMAND: u32 = 10119;
    /// API cannot proceed because the provided buffer (via call of kb_init in options argument) is not large enough
    pub const K_STATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH: u32 = 10802;
    /// API cannot proceed because the provided buffer pointer is NULL or buffer size is 0
    pub const K_STATUS_ROM_API_INVALID_BUFFER: u32 = 10803;

    pub fn into_result(self) -> Result<(), Self> {
        if self.0 == Self::K_STATUS_FAIL {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl From<Status> for BootError {
    fn from(_value: Status) -> Self {
        BootError::Authenticate
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Status {
    fn format(&self, fmt: defmt::Formatter) {
        use defmt::write;

        match self.0 {
            Self::K_STATUS_SUCCESS => write!(fmt, "K_STATUS_SUCCESS"),
            Self::K_STATUS_FAIL => write!(fmt, "K_STATUS_FAIL"),
            Self::K_STATUS_INVALID_ARGUMENT => write!(fmt, "K_STATUS_INVALID_ARGUMENT"),
            Self::K_STATUS_ROM_LDR_DATA_UNDERRUN => write!(fmt, "K_STATUS_ROM_LDR_DATA_UNDERRUN"),
            Self::K_STATUS_ROM_LDR_JUMP_RETURNED => write!(fmt, "K_STATUS_ROM_LDR_JUMP_RETURNED"),
            Self::K_STATUS_ROM_LDR_ROLLBACK_BLOCKED => write!(fmt, "K_STATUS_ROM_LDR_ROLLBACK_BLOCKED"),
            Self::K_STATUS_ROM_LDR_PENDING_JUMP_COMMAND => write!(fmt, "K_STATUS_ROM_LDR_PENDING_JUMP_COMMAND"),
            Self::K_STATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH => write!(fmt, "K_STATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH"),
            Self::K_STATUS_ROM_API_INVALID_BUFFER => write!(fmt, "K_STATUS_ROM_API_INVALID_BUFFER"),
            _ => write!(fmt, "unknown"),
        }
    }
}

impl core::fmt::Debug for Status {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Self::K_STATUS_SUCCESS => write!(f, "K_STATUS_SUCCESS"),
            Self::K_STATUS_FAIL => write!(f, "K_STATUS_FAIL"),
            Self::K_STATUS_INVALID_ARGUMENT => write!(f, "K_STATUS_INVALID_ARGUMENT"),
            Self::K_STATUS_ROM_LDR_DATA_UNDERRUN => write!(f, "K_STATUS_ROM_LDR_DATA_UNDERRUN"),
            Self::K_STATUS_ROM_LDR_JUMP_RETURNED => write!(f, "K_STATUS_ROM_LDR_JUMP_RETURNED"),
            Self::K_STATUS_ROM_LDR_ROLLBACK_BLOCKED => write!(f, "K_STATUS_ROM_LDR_ROLLBACK_BLOCKED"),
            Self::K_STATUS_ROM_LDR_PENDING_JUMP_COMMAND => write!(f, "K_STATUS_ROM_LDR_PENDING_JUMP_COMMAND"),
            Self::K_STATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH => write!(f, "K_STATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH"),
            Self::K_STATUS_ROM_API_INVALID_BUFFER => write!(f, "K_STATUS_ROM_API_INVALID_BUFFER"),
            _ => write!(f, "unknown"),
        }
    }
}
