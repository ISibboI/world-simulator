//! A simple Earth-like water cycle: evaporation over oceans, moisture transport by the prevailing
//! winds, and precipitation over land.
//!
//! The prevailing winds follow Earth's three-cell circulation: easterly trade winds below 30°
//! latitude, westerlies between 30° and 60°, and polar easterlies above 60°. Moisture evaporates
//! over the ocean (more in warm, low latitudes) and is carried downwind. Over land, a fraction of
//! the moisture rains out in every cell. That fraction is high where air rises: in the
//! intertropical convergence zone at the equator, along the polar front around 55°, and where
//! winds are forced up mountain slopes (orographic precipitation). Downwind of mountains, the air
//! is dry (rain shadow), and in the subsiding air around 25° there is little rain (deserts).

use glam::DVec3;

use crate::sphere::SphereMesh;

/// Parameters of the water cycle. Precipitation is in m/yr.
#[derive(Clone, Debug)]
pub struct ClimateParams {
    /// Moisture evaporated over the ocean at the equator (relative units).
    pub equatorial_evaporation: f64,
    /// Moisture evaporated over the ocean at the poles (relative units). Only used when no
    /// sea-surface temperature is given.
    pub polar_evaporation: f64,
    /// Sea-surface temperature in °C at which [`Self::equatorial_evaporation`] is reached.
    pub reference_temperature: f64,
    /// Relative increase of evaporation per kelvin of sea-surface temperature
    /// (Clausius-Clapeyron scaling of saturation vapour pressure, damped).
    pub evaporation_per_kelvin: f64,
    /// Minimum evaporation (relative units), e.g. over sea ice.
    pub min_evaporation: f64,
    /// Fraction of moisture raining out per cell where air neither rises nor sinks.
    pub background_rain_fraction: f64,
    /// Additional rain fraction in the intertropical convergence zone.
    pub equatorial_rain_fraction: f64,
    /// Additional rain fraction at the polar front.
    pub polar_front_rain_fraction: f64,
    /// Latitude of the polar front in degrees.
    pub polar_front_latitude: f64,
    /// Additional rain fraction per unit of upwind slope (orographic lift).
    pub orographic_factor: f64,
    /// Converts rained-out moisture into precipitation in m/yr.
    pub precipitation_scale: f64,
    /// Iterations of moisture transport per update; should exceed the width of the largest
    /// continent in cells.
    pub transport_iterations: usize,
}

impl Default for ClimateParams {
    fn default() -> Self {
        Self {
            equatorial_evaporation: 1.0,
            polar_evaporation: 0.2,
            reference_temperature: 27.0,
            evaporation_per_kelvin: 0.05,
            min_evaporation: 0.05,
            background_rain_fraction: 0.02,
            equatorial_rain_fraction: 0.06,
            polar_front_rain_fraction: 0.04,
            polar_front_latitude: 55.0,
            orographic_factor: 20.0,
            precipitation_scale: 25.0,
            transport_iterations: 80,
        }
    }
}

pub struct Climate {
    params: ClimateParams,
    /// Latitude of every cell in radians.
    latitude: Vec<f64>,
    /// The neighbour each cell receives its air from.
    upwind: Vec<u32>,
}

impl Climate {
    pub fn new(mesh: &SphereMesh, params: ClimateParams) -> Self {
        let cell_count = mesh.len() as u32;
        let latitude = (0..cell_count)
            .map(|cell| mesh.vertex(cell).z.clamp(-1.0, 1.0).asin())
            .collect();
        let upwind = (0..cell_count)
            .map(|cell| {
                let position = mesh.vertex(cell);
                let wind = prevailing_wind(position);
                *mesh
                    .neighbors(cell)
                    .iter()
                    .max_by(|&&a, &&b| {
                        let a = (mesh.vertex(a) - position).normalize().dot(-wind);
                        let b = (mesh.vertex(b) - position).normalize().dot(-wind);
                        a.total_cmp(&b)
                    })
                    .expect("every cell has neighbours")
            })
            .collect();
        Self {
            params,
            latitude,
            upwind,
        }
    }

    /// Computes the precipitation in m/yr for every cell, given the surface elevation in metres
    /// (sea level is 0) and the distance between neighbouring cells in metres.
    ///
    /// Evaporation over the ocean depends on the annual mean sea-surface temperature in °C, if
    /// given; otherwise on latitude.
    pub fn precipitation(
        &self,
        elevation: &[f64],
        cell_spacing: f64,
        sea_surface_temperature: Option<&[f64]>,
    ) -> Vec<f64> {
        let params = &self.params;
        let cell_count = elevation.len();
        let is_ocean = |cell: usize| elevation[cell] < 0.0;

        let evaporation: Vec<f64> = match sea_surface_temperature {
            Some(temperature) => temperature
                .iter()
                .map(|&t| {
                    let scaling =
                        (params.evaporation_per_kelvin * (t - params.reference_temperature)).exp();
                    (params.equatorial_evaporation * scaling).max(params.min_evaporation)
                })
                .collect(),
            None => self
                .latitude
                .iter()
                .map(|&latitude| {
                    let warmth = latitude.cos().powi(2);
                    params.polar_evaporation
                        + (params.equatorial_evaporation - params.polar_evaporation) * warmth
                })
                .collect(),
        };

        let rain_fraction: Vec<f64> = (0..cell_count)
            .map(|cell| {
                let latitude = self.latitude[cell].to_degrees().abs();
                let convergence = params.equatorial_rain_fraction
                    * (-(latitude / 12.0).powi(2)).exp()
                    + params.polar_front_rain_fraction
                        * (-((latitude - params.polar_front_latitude) / 12.0).powi(2)).exp();
                let upwind = self.upwind[cell] as usize;
                let lift =
                    (elevation[cell].max(0.0) - elevation[upwind].max(0.0)).max(0.0) / cell_spacing;
                (params.background_rain_fraction + convergence + params.orographic_factor * lift)
                    .clamp(0.0, 0.95)
            })
            .collect();

        // Iterate moisture transport towards a steady state. Ocean cells are moisture sources.
        let mut moisture = evaporation.clone();
        let mut next = vec![0.0; cell_count];
        for _ in 0..params.transport_iterations {
            for cell in 0..cell_count {
                next[cell] = if is_ocean(cell) {
                    evaporation[cell]
                } else {
                    moisture[self.upwind[cell] as usize] * (1.0 - rain_fraction[cell])
                };
            }
            std::mem::swap(&mut moisture, &mut next);
        }

        (0..cell_count)
            .map(|cell| {
                let incoming = if is_ocean(cell) {
                    evaporation[cell]
                } else {
                    moisture[self.upwind[cell] as usize]
                };
                incoming * rain_fraction[cell] * params.precipitation_scale
            })
            .collect()
    }
}

/// The direction of the prevailing surface wind (the direction the air moves to) at a position
/// on the unit sphere.
pub fn prevailing_wind(position: DVec3) -> DVec3 {
    let east = DVec3::Z.cross(position);
    let east = if east.length_squared() < 1e-12 {
        DVec3::X
    } else {
        east.normalize()
    };
    let north = position.cross(east);
    let latitude = position.z.clamp(-1.0, 1.0).asin().to_degrees();
    let hemisphere = latitude.signum();

    // Zonal (eastward) and meridional (poleward) components of the three-cell circulation.
    let (eastward, poleward) = match latitude.abs() {
        l if l < 30.0 => (-1.0, -0.3), // trade winds blow west and towards the equator
        l if l < 60.0 => (1.0, 0.3),   // westerlies blow east and towards the pole
        _ => (-1.0, -0.3),             // polar easterlies blow west and towards the equator
    };
    (east * eastward + north * poleward * hemisphere).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winds_follow_three_cell_circulation() {
        let at = |latitude: f64| {
            let latitude = latitude.to_radians();
            DVec3::new(latitude.cos(), 0.0, latitude.sin())
        };
        // At longitude 0, east is +y.
        assert!(prevailing_wind(at(15.0)).y < 0.0);
        assert!(prevailing_wind(at(45.0)).y > 0.0);
        assert!(prevailing_wind(at(-45.0)).y > 0.0);
        assert!(prevailing_wind(at(75.0)).y < 0.0);
        // Trade winds blow towards the equator in both hemispheres.
        assert!(prevailing_wind(at(15.0)).z < 0.0);
        assert!(prevailing_wind(at(-15.0)).z > 0.0);
    }

    #[test]
    fn rain_shadow_and_latitude_bands() {
        let mesh = SphereMesh::icosphere(5);
        let climate = Climate::new(&mesh, ClimateParams::default());
        let spacing = mesh.mean_edge_angle() * 6.371e6;

        // An all-ocean planet: wet equator, dry subtropics.
        let ocean = vec![-4000.0; mesh.len()];
        let precipitation = climate.precipitation(&ocean, spacing, None);
        let mean_at = |from: f64, to: f64| {
            let cells: Vec<f64> = (0..mesh.len())
                .filter(|&c| (from..to).contains(&climate.latitude[c].to_degrees().abs()))
                .map(|c| precipitation[c])
                .collect();
            cells.iter().sum::<f64>() / cells.len() as f64
        };
        assert!(mean_at(0.0, 5.0) > 2.0 * mean_at(20.0, 30.0));
        assert!(mean_at(50.0, 60.0) > mean_at(20.0, 30.0));

        // A north-south mountain range in the westerlies: the east side is drier than the west.
        let mut elevation = vec![-4000.0; mesh.len()];
        let longitude = |c: u32| mesh.vertex(c).y.atan2(mesh.vertex(c).x).to_degrees();
        let latitude = |c: u32| climate.latitude[c as usize].to_degrees();
        for cell in 0..mesh.len() as u32 {
            if (35.0..55.0).contains(&latitude(cell)) && (-40.0..40.0).contains(&longitude(cell)) {
                elevation[cell as usize] = if longitude(cell).abs() < 3.0 {
                    4000.0
                } else {
                    200.0
                };
            }
        }
        let precipitation = climate.precipitation(&elevation, spacing, None);
        let mean_land = |from: f64, to: f64| {
            let cells: Vec<f64> = (0..mesh.len() as u32)
                .filter(|&c| elevation[c as usize] > 0.0 && (from..to).contains(&longitude(c)))
                .map(|c| precipitation[c as usize])
                .collect();
            cells.iter().sum::<f64>() / cells.len() as f64
        };
        assert!(mean_land(-15.0, -3.0) > 1.5 * mean_land(3.0, 15.0));
    }
}
