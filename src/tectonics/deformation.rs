//! Deformable plate margins (#69): where shortening and stretching at continental plate
//! boundaries go.
//!
//! The plates are rigid, so the relative motion at a boundary is fixed by the plates. On Earth,
//! convergence between continents is taken up over zones hundreds to more than a thousand
//! kilometres wide (Tibet, Zagros, Alps), and continental lithosphere deforms like a thin
//! viscous sheet (England & McKenzie 1982; Houseman & England 1986): the strain rate grows with
//! the stress to the power n ≈ 3 divided by the strength, so weak lithosphere (young, stretched,
//! previously deformed) takes up most of the shortening, and a column's resistance to further
//! thickening grows with its excess gravitational potential energy, ½ g ρc (1 − ρc/ρm)
//! (h² − h₀²) (England & Molnar 1997; Molnar & Lyon-Caen 1988), so thickened crust stops
//! thickening and the zone widens instead (plateau growth). Here, the area the rigid plates lose
//! at the boundary (the overridden samples) is redistributed as thickening of a band of the
//! overriding plate weighted that way ([`band_weight`]); thickened crust in a rift band stretches
//! first ([`stretching_weight`]). Both conserve mass: they only decide where crust goes.
//!
//! The strength comes from [`band_strength`], the single place where this module reads the
//! state of the lithosphere (see [`super::weak_zones::lithosphere_strength`]).

use super::weak_zones::lithosphere_strength;
use super::*;

/// Stress exponent n of the thin viscous sheet (power-law creep of the lithosphere): at the same
/// stress, lithosphere of relative strength s strains ∝ s⁻ⁿ.
pub(super) const DEFORMATION_EXPONENT: i32 = 3;

/// Thickness (m) of unthickened continental crust, whose excess potential energy is zero.
pub(super) const REFERENCE_THICKNESS: f64 = 35e3;

/// Crust thickness (m) at which the excess potential energy of a column balances the largest
/// compressive force continental collisions transmit, ~7.6 × 10¹² N/m (Tibet: Molnar &
/// Lyon-Caen 1988; England & Molnar 1997): the band weight of thicker crust is zero, so the
/// deforming zone widens instead (plateau growth). It is a weight, not a cap: one share can
/// still take a cell above it; the hard cap is `TectonicParams::max_crust_thickness`.
pub(super) const PLATEAU_THICKNESS: f64 = 70e3;

/// Lowest strength used for distributed deformation (≤ 8× softness contrast).
/// [`lithosphere_strength`] goes down to 0.25 in fresh orogenic belts and 0.05 at ridge axes
/// and fresh rifts: a proxy built for choosing where plates break, not an integrated viscous
/// strength of continental lithosphere. With a lower floor, the s⁻³ weights make the band
/// localise in the belt its own collision has just marked (37× at 0.3 with fresh belts at
/// 0.25), and the belt gets narrower than with no band at all (test
/// `a_collision_band_spreads_the_belt_and_conserves_mass`).
const MIN_BAND_STRENGTH: f64 = 0.5;

/// Gravitational acceleration (m/s²).
const GRAVITY: f64 = 9.81;

/// Relative strength (0.5–1) of the lithosphere under `crust` for distributed deformation.
/// The single place where this module reads the state of the lithosphere.
pub(super) fn band_strength(crust: &Crust, time: f64) -> f64 {
    lithosphere_strength(crust, time).max(MIN_BAND_STRENGTH)
}

/// Excess gravitational potential energy (N/m) of continental crust `thickness` (m) thick over
/// the reference column, under Airy isostasy: ½ g ρc (1 − ρc/ρm) (h² − h₀²) (England & Molnar
/// 1997; the thickening of the lithospheric mantle, which lowers it, is neglected). Zero for
/// crust not thicker than the reference.
pub(super) fn excess_potential_energy(thickness: f64) -> f64 {
    let (crust, mantle) = (
        crate::isostasy::CONTINENTAL_DENSITY,
        crate::isostasy::MANTLE_DENSITY,
    );
    let reference = REFERENCE_THICKNESS;
    0.5 * GRAVITY
        * crust
        * (1.0 - crust / mantle)
        * (thickness.powi(2) - reference.powi(2)).max(0.0)
}

/// How easily a column takes up more thickening: 1 − its excess potential energy over that at
/// `limit` (m), 1 up to the reference thickness and 0 from `limit` on. With the limit
/// [`PLATEAU_THICKNESS`] this is the force balance of a plateau.
pub(super) fn thickening_room(thickness: f64, limit: f64) -> f64 {
    let at_limit = excess_potential_energy(limit);
    if at_limit <= 0.0 {
        return if thickness < limit { 1.0 } else { 0.0 };
    }
    (1.0 - excess_potential_energy(thickness) / at_limit).clamp(0.0, 1.0)
}

/// Share of the shortening that a sample in ring `ring` (0 = the contact cell, rings 0 to
/// `rings` inclusive) takes up, relative to the other samples of the band: a linear taper of
/// the strain rate away from the boundary for uniform strength (England, Houseman & Sclater
/// 1985 give such decay lengths for the strain rate), times the softness s⁻ⁿ of its
/// lithosphere (strain rate at the same stress), times its room to thicken below
/// [`PLATEAU_THICKNESS`]. Zero for crust that is not of continental origin (oceanic
/// lithosphere is not part of an orogen).
pub(super) fn band_weight(crust: &Crust, time: f64, ring: usize, rings: usize) -> f64 {
    if ring > rings || !crust.is_continental_origin_column() {
        return 0.0;
    }
    let taper = 1.0 - ring as f64 / (rings + 1) as f64;
    let softness = band_strength(crust, time).powi(-DEFORMATION_EXPONENT);
    taper * softness * thickening_room(crust.thickness, PLATEAU_THICKNESS)
}

/// Number of rings of a collision band of `width` (m) on a mesh with `spacing` (m) between
/// cells; 0 if there is no band.
pub(super) fn band_rings(width: f64, spacing: f64) -> usize {
    if width <= 0.0 {
        0
    } else {
        (width / spacing).round().max(1.0) as usize
    }
}

impl Simulation {
    /// Resistance (N) of a continental collision belt at a boundary cell to further convergence:
    /// the excess potential energy of the thickest crust at the cell and its neighbours (the
    /// belt on either side of the boundary), over half a cell's boundary length, since both
    /// sides of a collision have boundary cells. A plastic force like the resistance to
    /// subduction initiation (see `Simulation::update_plate_velocities`): the boundary
    /// converges only where the driving forces exceed it, so thickened belts slow their own
    /// collision down (England & Houseman 1989; England & Molnar 1997).
    pub(super) fn belt_resistance(&self, cell: u32) -> f64 {
        if self.params.belt_gpe_resistance <= 0.0 {
            return 0.0;
        }
        let thickest = self
            .mesh
            .neighbors(cell)
            .iter()
            .chain(std::iter::once(&cell))
            .map(|&c| self.surface_crust(c))
            .filter(Crust::is_continental_origin_column)
            .map(|crust| crust.thickness)
            .fold(0.0, f64::max);
        self.params.belt_gpe_resistance
            * excess_potential_energy(thickest)
            * 0.5
            * self.boundary_length_per_cell()
    }
}

/// Relative weight of a donor in a rift band beyond its distance weight: thickened crust, which
/// stores excess potential energy ∝ h² − h₀², stretches first (extensional collapse of thickened
/// crust: Basin and Range, Aegean; Buck 1991), as (h / h₀)²; 1 up to the reference thickness.
pub(super) fn stretching_weight(thickness: f64) -> f64 {
    (thickness / REFERENCE_THICKNESS).max(1.0).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn continental(thickness: f64) -> Crust {
        Crust {
            thickness,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: 0.0,
        }
    }

    #[test]
    fn thickened_crust_has_less_room() {
        assert_eq!(thickening_room(20e3, 70e3), 1.0);
        assert_eq!(thickening_room(35e3, 70e3), 1.0);
        assert_eq!(thickening_room(70e3, 70e3), 0.0);
        assert_eq!(thickening_room(80e3, 70e3), 0.0);
        let room = thickening_room(55e3, 70e3);
        let expected = (70e3f64.powi(2) - 55e3f64.powi(2)) / (70e3f64.powi(2) - 35e3f64.powi(2));
        assert!((room - expected).abs() < 1e-12);
        assert!(thickening_room(50e3, 70e3) > thickening_room(60e3, 70e3));
    }

    #[test]
    fn tibet_has_the_plateau_potential_energy() {
        // ~2.6e12 N/m at 50 km, ~7.6e12 N/m at 70 km (Molnar & Lyon-Caen 1988).
        let at = |h: f64| excess_potential_energy(h) / 1e12;
        assert!((2.4..2.9).contains(&at(50e3)), "{}", at(50e3));
        assert!((7.2..8.0).contains(&at(70e3)), "{}", at(70e3));
        assert_eq!(excess_potential_energy(30e3), 0.0);
    }

    #[test]
    fn weak_lithosphere_takes_up_more_shortening() {
        let old = continental(35e3);
        let mut belt = old;
        belt.mix_orogeny(-500.0, 1.0);
        let strong = band_weight(&old, 0.0, 0, 4);
        let weak = band_weight(&belt, 0.0, 0, 4);
        // A fresh belt (strength 0.25) is bounded at 0.5 → softness 2³.
        assert!((weak / strong - 8.0).abs() < 1e-9, "{weak} / {strong}");
        // Fresh, thin lithosphere is bounded at the band's floor.
        let mut fresh = continental(20e3);
        fresh.birth = 0.0;
        let freshest = band_weight(&fresh, 0.0, 0, 4);
        assert!((freshest / strong - MIN_BAND_STRENGTH.powi(-3)).abs() < 1e-9);
    }

    #[test]
    fn the_band_tapers_and_ends() {
        let crust = continental(35e3);
        let weights: Vec<f64> = (0..6)
            .map(|ring| band_weight(&crust, 0.0, ring, 4))
            .collect();
        assert!(
            weights.windows(2).take(4).all(|w| w[0] > w[1]),
            "{weights:?}"
        );
        assert!(weights[4] > 0.0);
        assert_eq!(weights[5], 0.0);
    }

    #[test]
    fn oceanic_crust_is_not_part_of_the_band() {
        let ocean = new_oceanic_crust(-50.0);
        assert_eq!(band_weight(&ocean, 0.0, 0, 4), 0.0);
    }

    #[test]
    fn thick_crust_stretches_first() {
        assert_eq!(stretching_weight(20e3), 1.0);
        assert_eq!(stretching_weight(35e3), 1.0);
        assert!((stretching_weight(70e3) - 4.0).abs() < 1e-12);
    }

    /// Two continental hemispheres (35 km, old) on a ~220 km mesh; the second plate turns about
    /// the x axis so that it converges with the first at `speed` (cm/yr) at y = −1 on the
    /// equator and diverges from it at y = +1. Runs `duration` Myr with crustal flow, returns
    /// the simulation.
    fn colliding_hemispheres(band_width: f64, speed: f64, duration: f64) -> Simulation {
        let params = TectonicParams {
            collision_band_width: band_width,
            ..TectonicParams::default()
        };
        let mut simulation = Simulation::new(crate::sphere::SphereMesh::icosphere(5), 1, params, 1);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = continental(35e3);
        }
        simulation.split_plate(0, glam::DVec3::Z).unwrap();
        simulation.polarities.clear();
        let omega = speed / 100.0 * 1e6 / simulation.params.planet_radius;
        simulation.plates[0].angular_velocity = glam::DVec3::ZERO;
        simulation.plates[1].angular_velocity = glam::DVec3::X * omega;
        let dt = 2.0;
        let mut time = 0.0;
        while time < duration {
            simulation.substep(dt);
            simulation.crustal_flow(dt);
            time += dt;
        }
        simulation
    }

    /// Continental crust thicker than `limit` (fraction of continental cells) and the maximum.
    fn thick(simulation: &Simulation, limit: f64) -> (f64, f64) {
        let cells: Vec<Crust> = (0..simulation.mesh.len() as u32)
            .map(|cell| simulation.surface_crust(cell))
            .filter(Crust::is_continental_origin)
            .collect();
        let above = cells.iter().filter(|c| c.thickness > limit).count() as f64;
        let max = cells.iter().map(|c| c.thickness).fold(0.0, f64::max);
        (above / cells.len() as f64, max)
    }

    #[test]
    fn a_collision_band_spreads_the_belt_and_conserves_mass() {
        // 1 cm/yr for 40 Myr: ~400 km of shortening at the fastest point. The mesh has ~220 km
        // cells, so the band is 1100 km wide (5 rings, as 500 km on the default mesh).
        let profile = colliding_hemispheres(0.0, 1.0, 40.0);
        let band = colliding_hemispheres(1100e3, 1.0, 40.0);
        let (profile_thick, profile_max) = thick(&profile, 50e3);
        let (band_thick, band_max) = thick(&band, 50e3);
        let (profile_belt, _) = thick(&profile, 37e3);
        let (band_belt, _) = thick(&band, 37e3);
        // The band makes the belt wider and lower.
        assert!(band_belt > profile_belt, "{band_belt} vs {profile_belt}");
        assert!(
            band_thick < profile_thick,
            "{band_thick} vs {profile_thick}"
        );
        assert!(band_max < profile_max, "{band_max} vs {profile_max}");
        // Nothing is lost at the cap (relative to the stacked mass).
        let stacked = band.ledger.collision_stacked;
        assert!(stacked > 0.0);
        assert!(band.ledger.additions_lost.abs() < 1e-9 * stacked);
    }
}
