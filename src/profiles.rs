//! Built-in profiles: the browser presets published by the wreq project (wreq-util 3.0.0-rc.10).
//!
//! Each profile is a JSON file in `profiles/`, compiled into the crate. Pick one by name, then
//! change whatever you need:
//!
//! ```
//! use emulation_config::Profile;
//!
//! let profile: Profile = "chrome_145_windows".parse()?;
//! let mut config = profile.config();
//! if let Some(tls) = config.transport.tls_options.as_mut() {
//!     tls.record_size_limit = Some(16_385);
//! }
//! let emulation = wreq::Emulation::from(&config);
//! # let _ = emulation;
//! # Ok::<(), emulation_config::UnknownVariant>(())
//! ```

use crate::{EmulationConfig, string_enum::string_enum};

/// Declares the profiles, each backed by `profiles/<name>.json`.
macro_rules! profiles {
    ($($(#[$meta:meta])* $variant:ident => $name:literal,)+) => {
        string_enum! {
            /// A built-in profile. The serde form is the profile name, e.g. `"chrome_145_macos"`.
            pub enum Profile("profile") {
                $($(#[$meta])* $variant => $name,)+
            }
        }

        impl Profile {
            /// The profile in the wire format, exactly as the SDK would send it.
            #[must_use]
            pub const fn json(self) -> &'static str {
                match self {
                    $(Self::$variant => include_str!(concat!("../profiles/", $name, ".json")),)+
                }
            }
        }
    };
}

profiles! {
    /// Chrome 145 on macOS, from the `wreq-util` presets.
    Chrome145Macos => "chrome_145_macos",
    /// Chrome 145 on Windows, from the `wreq-util` presets.
    Chrome145Windows => "chrome_145_windows",
    /// Chrome 145 on Android, from the `wreq-util` presets.
    Chrome145Android => "chrome_145_android",
    /// Firefox 147 on macOS, from the `wreq-util` presets.
    Firefox147Macos => "firefox_147_macos",
    /// Firefox 147 on Windows, from the `wreq-util` presets.
    Firefox147Windows => "firefox_147_windows",
    /// Firefox 135 on Android, from the `wreq-util` presets.
    FirefoxAndroid135 => "firefox_android_135",
    /// Safari 26.2 on macOS, from the `wreq-util` presets.
    Safari26_2 => "safari_26_2",
    /// Safari 26.2 on iPhone, from the `wreq-util` presets.
    SafariIos26_2 => "safari_ios_26_2",
    /// Safari 26.2 on iPad, from the `wreq-util` presets.
    SafariIpad26_2 => "safari_ipad_26_2",
}

impl Profile {
    /// The profile as a config you can change.
    ///
    /// # Panics
    ///
    /// Never in practice: every embedded profile is parsed by the test suite.
    #[must_use]
    pub fn config(self) -> EmulationConfig {
        serde_json::from_str(self.json()).expect("built-in profiles are valid")
    }
}
