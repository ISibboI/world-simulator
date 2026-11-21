//! The icosphere subdivisions the web version starts with when the URL does not give them.
//!
//! The simulation is single-threaded, and no browser reveals how fast one core is (no CPU model
//! or clock rate, for privacy; the core count says nothing about one core). What all browsers do
//! reveal is the kind of device, and phones are slower per core than desktops and laptops, so
//! phones and tablets (and, in Chromium, devices with little memory) start with
//! [`PHONE_SUBDIVISIONS`], everything else with [`DESKTOP_SUBDIVISIONS`]. The step-time hint of
//! the view corrects a wrong guess. See `notes/issue-78/research.md`.
//!
//! A value the user chose (a restart from the parameter panel or the hint) is remembered in the
//! browser's local storage and wins over the guess on the next visit; a value in the URL wins
//! over both. Only the logic lives here; reading the browser is in `crate::web`.

/// Subdivisions for phones, tablets and low-end computers.
pub const PHONE_SUBDIVISIONS: u32 = 5;
/// Subdivisions for desktops and laptops.
pub const DESKTOP_SUBDIVISIONS: u32 = 6;
/// Key of the remembered subdivisions in the local storage of the page.
pub const STORAGE_KEY: &str = "world-simulator.subdivisions";
/// Devices reporting at most this much memory (GiB; `navigator.deviceMemory`, Chromium only)
/// count as low-end.
const LOW_MEMORY: f64 = 4.0;
/// Largest subdivisions accepted from the storage (as checked by `Cli::check`).
const MAX_SUBDIVISIONS: u32 = 8;

/// What the browser tells about the device.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceSignals {
    /// `navigator.userAgent`.
    pub user_agent: String,
    /// `navigator.userAgentData.mobile` (Chromium only).
    pub mobile: Option<bool>,
    /// `navigator.maxTouchPoints`.
    pub max_touch_points: u32,
    /// Whether `(pointer: coarse)` and `(hover: none)` match: a touch screen is the main input.
    pub touch_only: bool,
    /// `navigator.deviceMemory` in GiB (Chromium only).
    pub device_memory: Option<f64>,
}

impl DeviceSignals {
    /// Whether the device is a phone or tablet. iPads report a Mac user agent by default, but
    /// Macs have no touch screen.
    pub fn phone_or_tablet(&self) -> bool {
        let agent = &self.user_agent;
        self.mobile == Some(true)
            || ["Android", "iPhone", "iPad", "Mobile"]
                .iter()
                .any(|word| agent.contains(word))
            || (agent.contains("Macintosh") && self.max_touch_points > 1)
            || self.touch_only
    }

    /// Whether the device reports little memory (only Chromium tells).
    pub fn low_memory(&self) -> bool {
        self.device_memory
            .is_some_and(|memory| memory <= LOW_MEMORY)
    }

    /// The guessed subdivisions for this device.
    pub fn guess(&self) -> u32 {
        if self.phone_or_tablet() || self.low_memory() {
            PHONE_SUBDIVISIONS
        } else {
            DESKTOP_SUBDIVISIONS
        }
    }
}

/// Where the subdivisions of the first simulation come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The URL gives them.
    Url,
    /// Remembered from an earlier visit.
    Stored(u32),
    /// The guess for the device.
    Guess,
}

/// Decides where the subdivisions come from: the URL wins over the remembered value, which
/// wins over the guess.
pub fn source(in_url: bool, stored: Option<u32>) -> Source {
    match (in_url, stored) {
        (true, _) => Source::Url,
        (false, Some(stored)) => Source::Stored(stored),
        (false, None) => Source::Guess,
    }
}

/// The remembered subdivisions from the stored text, if it is a valid value.
pub fn parse_stored(text: Option<&str>) -> Option<u32> {
    text?
        .trim()
        .parse()
        .ok()
        .filter(|&subdivisions| subdivisions <= MAX_SUBDIVISIONS)
}

/// The value to remember after the user started a simulation with `subdivisions` (a restart
/// from the parameter panel or the hint): `None` (forget it) if it is the guess anyway, so that
/// "Defaults" and a restart return to the guess.
pub fn to_store(subdivisions: u32, guess: u32) -> Option<u32> {
    (subdivisions != guess).then_some(subdivisions)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME_ANDROID: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/150.0.0.0 Mobile Safari/537.36";
    const SAFARI_IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) \
        AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
    const SAFARI_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
        AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
    const FIREFOX_WINDOWS: &str =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";
    const CHROME_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

    fn agent(user_agent: &str) -> DeviceSignals {
        DeviceSignals {
            user_agent: user_agent.to_owned(),
            ..DeviceSignals::default()
        }
    }

    #[test]
    fn phones_and_tablets_get_5() {
        assert_eq!(agent(CHROME_ANDROID).guess(), 5);
        assert_eq!(agent(SAFARI_IPHONE).guess(), 5);
        // An iPad with the default desktop user agent: a Mac with a touch screen.
        let ipad = DeviceSignals {
            max_touch_points: 5,
            ..agent(SAFARI_MAC)
        };
        assert_eq!(ipad.guess(), 5);
        // Chromium's client hint, whatever the user agent says ("Request desktop site").
        let hinted = DeviceSignals {
            mobile: Some(true),
            ..agent(CHROME_LINUX)
        };
        assert_eq!(hinted.guess(), 5);
        // A touch screen as the only input.
        let touch = DeviceSignals {
            touch_only: true,
            ..agent(CHROME_LINUX)
        };
        assert_eq!(touch.guess(), 5);
    }

    #[test]
    fn computers_get_6_unless_they_report_little_memory() {
        assert_eq!(agent(SAFARI_MAC).guess(), 6);
        assert_eq!(agent(FIREFOX_WINDOWS).guess(), 6);
        let desktop = DeviceSignals {
            mobile: Some(false),
            max_touch_points: 0,
            device_memory: Some(16.0),
            ..agent(CHROME_LINUX)
        };
        assert_eq!(desktop.guess(), 6);
        // A touch laptop whose main pointer is the touchpad stays a computer.
        let touch_laptop = DeviceSignals {
            max_touch_points: 10,
            ..desktop.clone()
        };
        assert_eq!(touch_laptop.guess(), 6);
        let low_end = DeviceSignals {
            device_memory: Some(4.0),
            ..desktop
        };
        assert_eq!(low_end.guess(), 5);
    }

    #[test]
    fn the_url_wins_over_the_stored_value_which_wins_over_the_guess() {
        assert_eq!(source(true, Some(4)), Source::Url);
        assert_eq!(source(true, None), Source::Url);
        assert_eq!(source(false, Some(4)), Source::Stored(4));
        assert_eq!(source(false, None), Source::Guess);
    }

    #[test]
    fn only_valid_stored_values_are_used() {
        assert_eq!(parse_stored(Some("6")), Some(6));
        assert_eq!(parse_stored(Some(" 0 ")), Some(0));
        assert_eq!(parse_stored(Some("9")), None);
        assert_eq!(parse_stored(Some("six")), None);
        assert_eq!(parse_stored(Some("")), None);
        assert_eq!(parse_stored(None), None);
    }

    #[test]
    fn the_guess_is_not_stored() {
        assert!(STORAGE_KEY.starts_with("world-simulator."));
        assert_eq!(to_store(6, 6), None);
        assert_eq!(to_store(5, 6), Some(5));
        assert_eq!(to_store(7, 5), Some(7));
    }
}
