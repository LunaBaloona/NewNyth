/// Errors that can occur during Roccat Nyth HID communication.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No Roccat Nyth device found")]
    DeviceNotFound,

    #[error("Failed to open HID device at {path}: {source}")]
    DeviceOpen {
        path: String,
        source: std::io::Error,
    },

    #[error("HID ioctl failed ({operation}): {source}")]
    Ioctl {
        operation: &'static str,
        source: nix::Error,
    },

    #[error("Device returned error status: {0}")]
    DeviceStatus(ControlStatus),

    #[error("Profile index {0} out of range (must be 0–4)")]
    ProfileOutOfRange(u8),

    #[error("CPI value {value} out of range ({min}–{max}, step {step})")]
    CpiOutOfRange {
        value: u16,
        min: u16,
        max: u16,
        step: u16,
    },

    #[error("Checksum mismatch: expected {expected:#06x}, got {actual:#06x}")]
    Checksum { expected: u16, actual: u16 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Status codes returned by the device control register.
///
/// After every write, the driver polls the control register until
/// the device reports OK or an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlStatus {
    Ok,
    Busy,
    Invalid,
    Critical,
    Unknown(u8),
}

impl std::fmt::Display for ControlStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Busy => write!(f, "Busy"),
            Self::Invalid => write!(f, "Invalid"),
            Self::Critical => write!(f, "Critical"),
            Self::Unknown(v) => write!(f, "Unknown({v:#04x})"),
        }
    }
}

impl ControlStatus {
    pub fn from_byte(value: u8) -> Self {
        match value {
            1 => Self::Ok,
            3 => Self::Busy,
            2 => Self::Invalid,
            0 | 4 => Self::Critical,
            v => Self::Unknown(v),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
