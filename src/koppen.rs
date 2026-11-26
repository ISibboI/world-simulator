//! Köppen–Geiger climate classes (#103): the 30 classes of Beck et al. (2018) from twelve
//! monthly mean temperatures and precipitations ([`classify`]), and the class of every cell of
//! the planet from the model's monthly fields ([`classes`]).
//!
//! The rules are those of Beck et al. (2018, Table 1; the same as Peel et al., 2007): 0 °C
//! between temperate (C) and cold (D) climates, polar (E) before arid (B), arid before the
//! others. The summer half of the year is the warmer of April–September and October–March, so
//! the classes work on both hemispheres.
//!
//! The model has twelve monthly temperatures, but solves the water cycle only for some months
//! (`ClimateParams::months`, 4 by default: January, April, July, October). The months between are
//! interpolated linearly (periodically) from the solved ones ([`monthly_precipitation`]): this
//! keeps the annual precipitation exactly and invents no extremes (the driest and wettest months
//! are solved months), but smooths the seasonality, so the splits that depend on the driest
//! month (Af/Am/Aw, the s/w/f of C and D) are approximate. Land under an ice sheet is EF
//! whatever its temperature; the ocean has no class.
//!
//! Only reads the simulation's state, so it does not change the simulation.

use crate::planet::Planet;

/// A Köppen–Geiger climate class. Its number ([`KoppenClass::id`], 1 to 30) is the order of
/// Beck et al. (2018).
#[allow(clippy::upper_case_acronyms)] // The class codes as they are written: ET, EF.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum KoppenClass {
    Af = 1,
    Am,
    Aw,
    BWh,
    BWk,
    BSh,
    BSk,
    Csa,
    Csb,
    Csc,
    Cwa,
    Cwb,
    Cwc,
    Cfa,
    Cfb,
    Cfc,
    Dsa,
    Dsb,
    Dsc,
    Dsd,
    Dwa,
    Dwb,
    Dwc,
    Dwd,
    Dfa,
    Dfb,
    Dfc,
    Dfd,
    ET,
    EF,
}

/// The main group of a class, its first letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// Tropical.
    A,
    /// Arid.
    B,
    /// Temperate.
    C,
    /// Cold (continental).
    D,
    /// Polar.
    E,
}

impl Group {
    pub const ALL: [Group; 5] = [Group::A, Group::B, Group::C, Group::D, Group::E];

    pub fn letter(self) -> &'static str {
        match self {
            Group::A => "A",
            Group::B => "B",
            Group::C => "C",
            Group::D => "D",
            Group::E => "E",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Group::A => "tropical",
            Group::B => "arid",
            Group::C => "temperate",
            Group::D => "cold",
            Group::E => "polar",
        }
    }
}

impl KoppenClass {
    /// Every class, by id.
    pub const ALL: [KoppenClass; 30] = {
        use KoppenClass::*;
        [
            Af, Am, Aw, BWh, BWk, BSh, BSk, Csa, Csb, Csc, Cwa, Cwb, Cwc, Cfa, Cfb, Cfc, Dsa, Dsb,
            Dsc, Dsd, Dwa, Dwb, Dwc, Dwd, Dfa, Dfb, Dfc, Dfd, ET, EF,
        ]
    };

    /// The number of the class, 1 to 30 (0 is no class in [`classes`]).
    pub fn id(self) -> u8 {
        self as u8
    }

    /// The class of a number, if it is one.
    pub fn from_id(id: u8) -> Option<Self> {
        Self::ALL.get(usize::from(id).checked_sub(1)?).copied()
    }

    /// The code, e.g. "Cfb".
    pub fn code(self) -> &'static str {
        use KoppenClass::*;
        match self {
            Af => "Af",
            Am => "Am",
            Aw => "Aw",
            BWh => "BWh",
            BWk => "BWk",
            BSh => "BSh",
            BSk => "BSk",
            Csa => "Csa",
            Csb => "Csb",
            Csc => "Csc",
            Cwa => "Cwa",
            Cwb => "Cwb",
            Cwc => "Cwc",
            Cfa => "Cfa",
            Cfb => "Cfb",
            Cfc => "Cfc",
            Dsa => "Dsa",
            Dsb => "Dsb",
            Dsc => "Dsc",
            Dsd => "Dsd",
            Dwa => "Dwa",
            Dwb => "Dwb",
            Dwc => "Dwc",
            Dwd => "Dwd",
            Dfa => "Dfa",
            Dfb => "Dfb",
            Dfc => "Dfc",
            Dfd => "Dfd",
            ET => "ET",
            EF => "EF",
        }
    }

    /// The common name, e.g. "temperate oceanic".
    pub fn name(self) -> &'static str {
        use KoppenClass::*;
        match self {
            Af => "tropical rainforest",
            Am => "tropical monsoon",
            Aw => "tropical savanna",
            BWh => "hot desert",
            BWk => "cold desert",
            BSh => "hot steppe",
            BSk => "cold steppe",
            Csa => "hot-summer Mediterranean",
            Csb => "warm-summer Mediterranean",
            Csc => "cold-summer Mediterranean",
            Cwa => "humid subtropical, dry winter",
            Cwb => "subtropical highland, dry winter",
            Cwc => "cold subtropical highland",
            Cfa => "humid subtropical",
            Cfb => "temperate oceanic",
            Cfc => "subpolar oceanic",
            Dsa => "hot-summer continental, dry summer",
            Dsb => "warm-summer continental, dry summer",
            Dsc => "subarctic, dry summer",
            Dsd => "extremely cold subarctic, dry summer",
            Dwa => "hot-summer continental, dry winter",
            Dwb => "warm-summer continental, dry winter",
            Dwc => "subarctic, dry winter",
            Dwd => "extremely cold subarctic, dry winter",
            Dfa => "hot-summer humid continental",
            Dfb => "warm-summer humid continental",
            Dfc => "subarctic",
            Dfd => "extremely cold subarctic",
            ET => "tundra",
            EF => "ice cap",
        }
    }

    pub fn group(self) -> Group {
        match self.code().as_bytes()[0] {
            b'A' => Group::A,
            b'B' => Group::B,
            b'C' => Group::C,
            b'D' => Group::D,
            _ => Group::E,
        }
    }
}

/// Months of the northern summer half (April to September), by index from January.
const APRIL_TO_SEPTEMBER: std::ops::Range<usize> = 3..9;

/// The class of a climate from its monthly mean temperatures (°C) and precipitation totals
/// (mm per month), January first. Beck et al. (2018), Table 1.
pub fn classify(temperature: &[f64; 12], precipitation: &[f64; 12]) -> KoppenClass {
    use KoppenClass::*;
    let mean_temperature = temperature.iter().sum::<f64>() / 12.0;
    let annual = precipitation.iter().sum::<f64>();
    let hottest = temperature
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let coldest = temperature.iter().copied().fold(f64::INFINITY, f64::min);
    let warm_months = temperature.iter().filter(|&&t| t > 10.0).count();

    // The summer half: the warmer of April–September and October–March.
    let northern_summer = |month: usize| APRIL_TO_SEPTEMBER.contains(&month);
    let april_to_september: f64 = APRIL_TO_SEPTEMBER.map(|m| temperature[m]).sum();
    let north = april_to_september >= temperature.iter().sum::<f64>() - april_to_september;
    let in_summer = |month: usize| northern_summer(month) == north;
    let half = |summer: bool| {
        (0..12)
            .filter(move |&month| in_summer(month) == summer)
            .map(|month| precipitation[month])
    };
    let summer_total: f64 = half(true).sum();
    let winter_total: f64 = half(false).sum();
    let summer_driest = half(true).fold(f64::INFINITY, f64::min);
    let summer_wettest = half(true).fold(0.0, f64::max);
    let winter_driest = half(false).fold(f64::INFINITY, f64::min);
    let winter_wettest = half(false).fold(0.0, f64::max);
    let driest = summer_driest.min(winter_driest);

    // Polar: no month reaches 10 °C.
    if hottest < 10.0 {
        return if hottest > 0.0 { ET } else { EF };
    }
    // Arid: less precipitation than the threshold that the evaporation of the mean temperature
    // and the timing of the rain set.
    let threshold = 2.0 * mean_temperature
        + if winter_total > 0.7 * annual {
            0.0
        } else if summer_total > 0.7 * annual {
            28.0
        } else {
            14.0
        };
    if annual < 10.0 * threshold {
        let hot = mean_temperature >= 18.0;
        return match (annual < 5.0 * threshold, hot) {
            (true, true) => BWh,
            (true, false) => BWk,
            (false, true) => BSh,
            (false, false) => BSk,
        };
    }
    // Tropical: every month at least 18 °C.
    if coldest >= 18.0 {
        return if driest >= 60.0 {
            Af
        } else if driest >= 100.0 - annual / 25.0 {
            Am
        } else {
            Aw
        };
    }
    // Temperate and cold: the dry season and the warmth of the summer.
    let dry_summer = summer_driest < 40.0 && summer_driest < winter_wettest / 3.0;
    let dry_winter = winter_driest < summer_wettest / 10.0;
    let season = match (dry_summer, dry_winter) {
        (false, false) => Season::Humid,
        (true, false) => Season::DrySummer,
        (false, true) => Season::DryWinter,
        // Both (Peel et al. give no rule): the drier half of the year is the dry season, as
        // in Kottek et al. (2006).
        (true, true) => {
            if summer_total <= winter_total {
                Season::DrySummer
            } else {
                Season::DryWinter
            }
        }
    };
    let summer = if hottest >= 22.0 {
        Summer::Hot
    } else if warm_months >= 4 {
        Summer::Warm
    } else if coldest < -38.0 {
        Summer::VeryColdWinter
    } else {
        Summer::Cold
    };
    if coldest > 0.0 {
        match (season, summer) {
            (Season::DrySummer, Summer::Hot) => Csa,
            (Season::DrySummer, Summer::Warm) => Csb,
            (Season::DrySummer, _) => Csc,
            (Season::DryWinter, Summer::Hot) => Cwa,
            (Season::DryWinter, Summer::Warm) => Cwb,
            (Season::DryWinter, _) => Cwc,
            (Season::Humid, Summer::Hot) => Cfa,
            (Season::Humid, Summer::Warm) => Cfb,
            (Season::Humid, _) => Cfc,
        }
    } else {
        match (season, summer) {
            (Season::DrySummer, Summer::Hot) => Dsa,
            (Season::DrySummer, Summer::Warm) => Dsb,
            (Season::DrySummer, Summer::Cold) => Dsc,
            (Season::DrySummer, Summer::VeryColdWinter) => Dsd,
            (Season::DryWinter, Summer::Hot) => Dwa,
            (Season::DryWinter, Summer::Warm) => Dwb,
            (Season::DryWinter, Summer::Cold) => Dwc,
            (Season::DryWinter, Summer::VeryColdWinter) => Dwd,
            (Season::Humid, Summer::Hot) => Dfa,
            (Season::Humid, Summer::Warm) => Dfb,
            (Season::Humid, Summer::Cold) => Dfc,
            (Season::Humid, Summer::VeryColdWinter) => Dfd,
        }
    }
}

/// The second letter of C and D climates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Season {
    /// f: no dry season.
    Humid,
    /// s.
    DrySummer,
    /// w.
    DryWinter,
}

/// The third letter of C and D climates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Summer {
    /// a: the warmest month at least 22 °C.
    Hot,
    /// b: at least four months above 10 °C.
    Warm,
    /// c.
    Cold,
    /// d (D only): the coldest month below −38 °C.
    VeryColdWinter,
}

/// Twelve monthly values (January first) from those of the solved months, which are spread
/// evenly from January: the solved months as they are, the months between interpolated
/// linearly, wrapping around the year. The mean over the year is the mean of the solved months.
/// `None` without solved months or if their count does not divide 12.
pub fn monthly_precipitation(solved: &[f64]) -> Option<[f64; 12]> {
    let count = solved.len();
    if count == 0 || !12usize.is_multiple_of(count) {
        return None;
    }
    let step = 12 / count;
    Some(std::array::from_fn(|month| {
        let slot = month / step;
        let t = (month % step) as f64 / step as f64;
        solved[slot] * (1.0 - t) + solved[(slot + 1) % count] * t
    }))
}

/// Land at least this share under permanent ice is an ice cap (EF), as the moisture mode shows
/// ice sheets.
pub const ICE_SHEET: f64 = 0.5;

/// The class of every cell of the planet (ocean, lakes and cells without data: `None`), from the
/// twelve monthly temperatures and the solved months of the water cycle.
pub fn classes(planet: &Planet) -> Vec<Option<KoppenClass>> {
    let simulation = planet.tectonics();
    let temperature = planet.temperature();
    let solved = &planet.water_cycle().monthly_precipitation;
    let mut solved_values = vec![0.0; solved.len()];
    (0..simulation.mesh().len())
        .map(|cell| {
            // Land without lakes: dry closed depressions below sea level included, lakes (at least
            // 1 m deep) are water like the ocean (#128).
            if crate::render::Water::of_cell(planet, cell as u32) != crate::render::Water::Land {
                return None;
            }
            if temperature
                .ice
                .get(cell)
                .is_some_and(|&ice| ice >= ICE_SHEET)
            {
                return Some(KoppenClass::EF);
            }
            if temperature.monthly.len() != 12 {
                return None;
            }
            let monthly_temperature: [f64; 12] = std::array::from_fn(|month| {
                temperature.monthly[month]
                    .get(cell)
                    .copied()
                    .unwrap_or(f64::NAN)
            });
            for (value, month) in solved_values.iter_mut().zip(solved) {
                // m/yr as a rate to mm in the month.
                *value = month.get(cell).copied().unwrap_or(f64::NAN) * 1000.0 / 12.0;
            }
            let monthly = monthly_precipitation(&solved_values)?;
            (monthly_temperature
                .iter()
                .chain(&monthly)
                .all(|v| v.is_finite()))
            .then(|| classify(&monthly_temperature, &monthly))
        })
        .collect()
}

/// Shares of land (by cell count; the cells have about equal areas) in each main group and of
/// each class, among the classified cells; for the snapshot log.
pub fn land_shares(classes: &[Option<KoppenClass>]) -> ([f64; 5], [f64; 30]) {
    let mut groups = [0.0; 5];
    let mut by_class = [0.0; 30];
    let land = classes.iter().flatten().count().max(1) as f64;
    for class in classes.iter().flatten() {
        groups[class.group() as usize] += 1.0 / land;
        by_class[usize::from(class.id()) - 1] += 1.0 / land;
    }
    (groups, by_class)
}

#[cfg(test)]
mod tests {
    use super::*;
    use KoppenClass::*;

    /// Station climates (monthly means, °C and mm, January first; rounded normals of about
    /// 1961–1990/1991–2020) with their Köppen–Geiger class.
    const STATIONS: &[(&str, [f64; 12], [f64; 12], KoppenClass)] = &[
        (
            "Singapore",
            [
                26.5, 27.1, 27.5, 28.0, 28.3, 28.3, 27.9, 27.9, 27.6, 27.6, 27.0, 26.5,
            ],
            [
                242.0, 162.0, 185.0, 179.0, 172.0, 162.0, 158.0, 176.0, 169.0, 194.0, 256.0, 288.0,
            ],
            Af,
        ),
        (
            "Miami",
            [
                20.1, 21.0, 22.4, 24.3, 26.4, 27.8, 28.6, 28.7, 28.0, 26.4, 23.6, 21.4,
            ],
            [
                47.0, 57.0, 76.0, 79.0, 159.0, 239.0, 163.0, 224.0, 236.0, 155.0, 86.0, 51.0,
            ],
            Am,
        ),
        (
            "Darwin",
            [
                28.3, 28.1, 28.2, 28.0, 27.0, 25.4, 25.0, 25.9, 27.6, 28.9, 29.1, 28.9,
            ],
            [
                427.0, 374.0, 317.0, 102.0, 21.0, 1.0, 1.0, 5.0, 15.0, 70.0, 142.0, 248.0,
            ],
            Aw,
        ),
        (
            "Cairo",
            [
                14.0, 15.3, 17.6, 21.4, 24.9, 27.6, 28.3, 28.3, 26.5, 23.9, 19.7, 15.7,
            ],
            [5.0, 4.0, 3.0, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 6.0],
            BWh,
        ),
        (
            "Kashgar",
            [
                -5.3, -0.6, 8.4, 16.2, 20.8, 24.6, 26.4, 25.1, 20.2, 12.6, 3.9, -3.0,
            ],
            [2.4, 3.5, 7.1, 6.2, 13.0, 8.5, 9.9, 6.9, 6.5, 2.0, 1.4, 1.5],
            BWk,
        ),
        (
            "Niamey",
            [
                24.4, 27.4, 31.3, 33.9, 33.6, 30.8, 28.3, 27.2, 28.6, 30.5, 28.4, 25.1,
            ],
            [
                0.0, 0.0, 3.0, 6.0, 32.0, 74.0, 148.0, 186.0, 90.0, 15.0, 1.0, 0.0,
            ],
            BSh,
        ),
        (
            "Denver",
            [
                -0.6, 0.4, 4.4, 8.9, 14.1, 19.6, 23.1, 22.1, 17.3, 10.6, 3.9, -0.8,
            ],
            [
                10.0, 11.0, 25.0, 43.0, 58.0, 46.0, 54.0, 46.0, 28.0, 24.0, 15.0, 11.0,
            ],
            BSk,
        ),
        (
            "Rome",
            [
                8.0, 8.8, 11.1, 13.6, 17.9, 21.8, 24.6, 24.7, 21.3, 17.1, 12.4, 9.2,
            ],
            [
                67.0, 73.0, 58.0, 81.0, 53.0, 34.0, 19.0, 37.0, 73.0, 113.0, 115.0, 81.0,
            ],
            Csa,
        ),
        (
            "Perth (southern hemisphere)",
            [
                24.5, 24.8, 23.0, 19.8, 16.4, 14.0, 13.1, 13.6, 15.0, 17.2, 20.1, 22.5,
            ],
            [
                17.0, 17.0, 20.0, 35.0, 95.0, 135.0, 145.0, 122.0, 72.0, 52.0, 28.0, 11.0,
            ],
            Csa,
        ),
        (
            "San Francisco",
            [
                10.7, 12.2, 13.1, 13.9, 15.1, 16.3, 16.6, 17.2, 17.7, 16.3, 13.4, 10.8,
            ],
            [
                114.0, 113.0, 80.0, 37.0, 18.0, 4.0, 0.0, 1.0, 4.0, 27.0, 80.0, 116.0,
            ],
            Csb,
        ),
        (
            "Hong Kong",
            [
                16.3, 16.8, 19.1, 22.6, 25.9, 27.9, 28.8, 28.6, 27.7, 25.5, 21.8, 17.9,
            ],
            [
                33.0, 48.0, 84.0, 174.0, 305.0, 457.0, 376.0, 432.0, 328.0, 101.0, 38.0, 27.0,
            ],
            Cwa,
        ),
        (
            "Buenos Aires (southern hemisphere)",
            [
                24.9, 23.6, 21.8, 18.0, 14.6, 11.7, 11.0, 12.8, 14.6, 17.6, 20.5, 23.2,
            ],
            [
                138.0, 127.0, 140.0, 119.0, 92.0, 58.0, 64.0, 64.0, 72.0, 123.0, 111.0, 120.0,
            ],
            Cfa,
        ),
        (
            "London",
            [
                5.2, 5.3, 7.6, 9.9, 13.3, 16.5, 18.7, 18.5, 15.7, 12.0, 8.0, 5.5,
            ],
            [
                55.0, 41.0, 42.0, 44.0, 49.0, 45.0, 45.0, 50.0, 49.0, 69.0, 59.0, 55.0,
            ],
            Cfb,
        ),
        (
            "Tórshavn",
            [3.9, 3.6, 3.9, 5.1, 7.1, 9.0, 10.6, 10.8, 9.2, 7.3, 5.3, 4.2],
            [
                158.0, 115.0, 132.0, 89.0, 70.0, 59.0, 72.0, 84.0, 118.0, 156.0, 142.0, 153.0,
            ],
            Cfc,
        ),
        (
            "Seoul",
            [
                -2.4, 0.4, 5.7, 12.5, 17.8, 22.2, 24.9, 25.7, 21.2, 14.8, 7.2, 0.4,
            ],
            [
                21.0, 25.0, 47.0, 65.0, 106.0, 133.0, 395.0, 364.0, 169.0, 52.0, 53.0, 22.0,
            ],
            Dwa,
        ),
        (
            "Moscow",
            [
                -6.5, -6.7, -1.0, 6.7, 13.2, 17.0, 19.2, 17.0, 11.3, 5.6, -1.2, -5.2,
            ],
            [
                52.0, 41.0, 35.0, 37.0, 49.0, 80.0, 85.0, 82.0, 68.0, 71.0, 55.0, 52.0,
            ],
            Dfb,
        ),
        (
            "Yakutsk",
            [
                -38.6, -33.8, -20.1, -4.8, 7.5, 16.4, 19.5, 15.2, 6.1, -7.8, -27.0, -37.6,
            ],
            [
                7.0, 6.0, 5.0, 7.0, 16.0, 31.0, 43.0, 38.0, 22.0, 16.0, 13.0, 9.0,
            ],
            Dfd,
        ),
        (
            "Utqiaġvik",
            [
                -25.6, -27.4, -25.9, -17.6, -5.6, 2.3, 5.3, 3.6, -1.1, -9.4, -18.2, -23.6,
            ],
            [
                4.0, 4.0, 3.0, 4.0, 4.0, 8.0, 25.0, 26.0, 16.0, 10.0, 5.0, 4.0,
            ],
            ET,
        ),
        (
            "Vostok",
            [
                -32.0, -44.3, -57.9, -64.7, -65.6, -65.2, -66.9, -67.6, -66.1, -57.4, -43.4, -32.0,
            ],
            [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            EF,
        ),
    ];

    #[test]
    fn stations_get_their_classes() {
        for (name, temperature, precipitation, class) in STATIONS {
            assert_eq!(
                classify(temperature, precipitation),
                *class,
                "{name} should be {}",
                class.code()
            );
        }
    }

    #[test]
    fn classes_have_ids_codes_names_and_groups() {
        for (index, class) in KoppenClass::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(class.id()), index + 1);
            assert_eq!(KoppenClass::from_id(class.id()), Some(class));
            assert!(!class.name().is_empty());
            assert_eq!(class.group().letter(), &class.code()[..1]);
        }
        assert_eq!(KoppenClass::from_id(0), None);
        assert_eq!(KoppenClass::from_id(31), None);
        assert_eq!(Cfb.name(), "temperate oceanic");
    }

    #[test]
    fn the_hemisphere_does_not_matter() {
        // A station shifted by half a year (a southern copy) keeps its class.
        for (name, temperature, precipitation, class) in STATIONS {
            let shift = |values: &[f64; 12]| std::array::from_fn(|m| values[(m + 6) % 12]);
            assert_eq!(
                classify(&shift(temperature), &shift(precipitation)),
                *class,
                "{name}"
            );
        }
    }

    #[test]
    fn interpolated_months_keep_the_year() {
        let solved = [120.0, 30.0, 0.0, 60.0];
        let months = monthly_precipitation(&solved).unwrap();
        // The solved months as they are (January, April, July, October).
        assert_eq!([months[0], months[3], months[6], months[9]], solved);
        // Linear between them, wrapping around from October to January.
        assert!((months[1] - 90.0).abs() < 1e-12);
        assert!((months[11] - 100.0).abs() < 1e-12);
        let mean = months.iter().sum::<f64>() / 12.0;
        assert!((mean - solved.iter().sum::<f64>() / 4.0).abs() < 1e-12);
        // The driest and wettest month are solved months.
        assert_eq!(months.iter().copied().fold(f64::INFINITY, f64::min), 0.0);
        assert_eq!(months.iter().copied().fold(0.0, f64::max), 120.0);
        // One solved month: a uniform year; 12: as they are; others that do not divide 12: none.
        assert_eq!(monthly_precipitation(&[5.0]), Some([5.0; 12]));
        let twelve: [f64; 12] = std::array::from_fn(|m| m as f64);
        assert_eq!(monthly_precipitation(&twelve), Some(twelve));
        assert_eq!(monthly_precipitation(&[1.0; 5]), None);
        assert_eq!(monthly_precipitation(&[]), None);
    }

    #[test]
    fn a_small_planet_is_classified() {
        use clap::Parser;
        // Today's living Earth, the default before #190: the barren default at epoch 0 has no
        // tropical climate on this coarse mesh at the start.
        let cli = crate::Cli::parse_from([
            "world-simulator",
            "--subdivisions",
            "3",
            "--biosphere",
            "modern",
            "--epoch",
            "4",
        ]);
        let planet = crate::build_planet(&cli).unwrap();
        let classes = classes(&planet);
        for (cell, class) in classes.iter().enumerate() {
            if crate::render::Water::of_cell(&planet, cell as u32) != crate::render::Water::Land {
                assert_eq!(*class, None);
            } else {
                assert!(class.is_some());
            }
        }
        let (groups, by_class) = land_shares(&classes);
        assert!((groups.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!((by_class.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        // A planet with tropics, deserts and poles.
        assert!(groups[Group::A as usize] > 0.0);
        assert!(groups[Group::B as usize] > 0.0);
    }
}
