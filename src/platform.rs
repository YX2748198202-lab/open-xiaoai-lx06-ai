//! Device-specific integration points for supported Xiaomi XiaoAI speakers.
//!
//! The shared AI pipeline stays the same across devices.  This module keeps
//! firmware paths and native service names in one place so adding another
//! model does not require duplicating the API, history, routing or fallback
//! implementation.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceModel {
    Lx06,
    Oh2p,
}

impl Default for DeviceModel {
    fn default() -> Self {
        Self::Lx06
    }
}

impl DeviceModel {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_uppercase().as_str() {
            "LX06" | "XIAOAI_PRO" | "XIAOAI-PRO" => Ok(Self::Lx06),
            "OH2P" | "XIAOMI_SPEAKER_PRO" | "XIAOMI-SPEAKER-PRO" => Ok(Self::Oh2p),
            other => Err(format!("不支持的 DEVICE_MODEL={other}，可选值：LX06、OH2P")),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Lx06 => "LX06",
            Self::Oh2p => "OH2P",
        }
    }

    pub fn target_triple(&self) -> &'static str {
        match self {
            Self::Lx06 => "armv7-unknown-linux-gnueabihf",
            Self::Oh2p => "aarch64-unknown-linux-gnu",
        }
    }

    pub fn expected_uname(&self) -> &'static str {
        match self {
            Self::Lx06 => "armv7l",
            Self::Oh2p => "aarch64",
        }
    }

    pub fn is_experimental(&self) -> bool {
        matches!(self, Self::Oh2p)
    }

    /// Candidate files used by the native firmware to expose recognition data.
    /// OH2P paths are candidates from the patched firmware layout and still
    /// require confirmation on real hardware.
    pub fn monitor_paths(&self) -> &'static [&'static str] {
        match self {
            Self::Lx06 => &LX06_MONITOR_PATHS,
            Self::Oh2p => &OH2P_MONITOR_PATHS,
        }
    }

    pub fn native_service(&self) -> &'static str {
        // Present in the LX06 runtime and in the OH2P 1.58.6 firmware layout.
        // OH2P runtime behavior still needs real-device verification.
        "/etc/init.d/mico_aivs_lab"
    }

    pub fn ubus_object(&self) -> &'static str {
        "mibrain"
    }

    pub fn tts_method(&self) -> &'static str {
        "text_to_speech"
    }

    pub fn ai_method(&self) -> &'static str {
        "ai_service"
    }
}

static LX06_MONITOR_PATHS: [&str; 2] = ["/tmp/mico_aivs_lab/instruction.log", "/tmp/log/messages"];

static OH2P_MONITOR_PATHS: [&str; 2] = ["/tmp/mico_aivs_lab/instruction.log", "/tmp/log/messages"];

#[cfg(test)]
mod tests {
    use super::DeviceModel;

    #[test]
    fn parses_supported_models() {
        assert_eq!(DeviceModel::parse("lx06").unwrap(), DeviceModel::Lx06);
        assert_eq!(DeviceModel::parse("OH2P").unwrap(), DeviceModel::Oh2p);
        assert!(DeviceModel::parse("unknown").is_err());
    }
}
