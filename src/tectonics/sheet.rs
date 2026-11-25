//! Non-rigid plates (#70): the lithosphere of each plate deforms as a thin viscous sheet.
//!
//! The velocity of the lithosphere at a crust sample of a plate is the plate's rotation plus a
//! deformation velocity, `v = ω × r + u`. The rotation `ω` comes from the torque balance of the
//! plates ([`super::dynamics`]); `u` is solved every step from the same forces acting on each
//! cell (drag against the mantle, slab pull and resistances at the plate's boundaries), with
//! ridge push spread over the young sea floor as the gradient of its potential energy and the
//! potential energy of thickened continental crust pushing outwards, for a thin viscous sheet
//! (England & McKenzie 1982; Houseman & England 1986) whose depth-integrated viscosity follows
//! the strength of the lithosphere ([`Simulation::sheet_viscosity`]). The forces on a plate
//! have no net torque (that is the torque balance; what is left is removed), so `u` has no
//! drag-weighted rigid part: a stiff plate does not deform, and rigid plates are the stiff limit
//! of this model.
//!
//! **Discretisation.** The sheet's viscous forces are central springs along the mesh edges
//! between samples of a plate (in the plate's frame): on a triangular lattice they make an
//! isotropic sheet, and they do not resist rigid rotations of the sphere. Samples so stiff that
//! they would hardly strain ([`SheetParams::rigid_viscosity`]) are grouped into rigid blocks
//! (connected components) with one rotation each; only the weak samples and the blocks are
//! unknowns of the linear system, solved by conjugate gradients with a block-Jacobi
//! preconditioner, warm-started from the previous step. Each plate's frame then follows its
//! largest block (its rotation absorbs that block's rigid motion), so that the bulk of a plate
//! does not move through its frame.
//!
//! **Transport.** Every substep, crust moves between the samples of a plate by `u`
//! (finite-volume upwind fluxes over the faces of the dual mesh, mass-conserving): converging
//! flow thickens the crust, diverging flow thins it. Stretched continental lithosphere is
//! heated (McKenzie 1978), shortened continental crust is marked orogenic. The edges of a plate
//! move with its material ([`PlateSheet::front`]): a retreating margin gives up the cell, and
//! the neighbouring plate moves in; an advancing margin takes a new cell.
//!
//! **Splits.** Where the blocks of a plate move apart, together or past each other faster than
//! [`SheetParams::split_speed`] for [`SheetParams::split_time`], the deforming zone between them
//! has become a plate boundary: the plate splits there.
//!
//! Grid: deforming zones of a few hundred kilometres and more are resolved at ~110 km cells;
//! single faults and narrow shear zones are smeared over a cell or two, and the upwind transport
//! smears gradients inside deforming zones.

use glam::{DMat2, DMat3, DVec2, DVec3};

use super::crust::{age_for_thermal_fraction, thermal_fraction};
use super::deformation::excess_potential_energy;
use super::dynamics::{BoundaryKind, SECONDS_PER_MYR};
use super::ledger::Term;
use super::*;

/// Parameters of the deforming plates (`--set sheet.NAME=VALUE`).
#[derive(Clone, Debug)]
pub struct SheetParams {
    /// Whether plates deform at all (0 restores rigid plates).
    pub enabled: bool,
    /// Depth-integrated viscosity (Pa·s·m) at [`Self::reference_strain_rate`] of lithosphere of
    /// relative strength 1.
    pub strong_viscosity: f64,
    /// The viscosity grows with the relative strength to this power: the integrated strength is
    /// dominated by the coldest part of the lithosphere.
    pub strength_exponent: f64,
    /// Strength of continental lithosphere relative to oceanic lithosphere of the same thermal
    /// state: quartz-rich crust creeps at lower stress than olivine (Kohlstedt, Evans &
    /// Mackwell 1995; Burov & Watts 2006).
    pub continental_strength: f64,
    /// Stress exponent n of power-law creep: above the reference strain rate the effective
    /// viscosity falls as `ε̇^(1/n − 1)` (deforming zones soften and localise).
    pub stress_exponent: f64,
    /// Strain rate (1/s) at which [`Self::strong_viscosity`] applies.
    pub reference_strain_rate: f64,
    /// Samples at least this viscous (Pa·s·m) are rigid: with basal drag of ~10¹⁵ Pa·s/m they
    /// would deform over more than ~3000 km, i.e. not at all within a plate.
    pub rigid_viscosity: f64,
    /// Lowest viscosity (Pa·s·m).
    pub min_viscosity: f64,
    /// Largest softening by power-law creep (factor on the viscosity).
    pub max_softening: f64,
    /// Lowest relative strength of lithosphere that is not of continental origin: oceanic
    /// lithosphere is olivine-dominated and strong once it has left the ridge axis, where the
    /// plate boundary takes up the motion; plate-scale stresses rarely deform young sea floor.
    pub oceanic_min_strength: f64,
    /// Age (Myr) of sea floor up to which ridge push builds up (its potential energy falls
    /// with age while the plate cools).
    pub ridge_push_age: f64,
    /// Factor on the force of the potential energy of thickened continental crust (Airy
    /// crust only, see `deformation::excess_potential_energy`). Calibrated to 2: with the
    /// crust-only estimate, continents thickened and emerged (flooded continental crust 18%
    /// instead of 25%, crust > 50 km 8% of the continental crust, 4 seeds); the factor stands
    /// in for the parts of the column's potential energy that are not modelled (thermal
    /// expansion of thickened, hot lithosphere) and for the discretisation's bulk stiffness
    /// (see the module documentation).
    pub gpe_factor: f64,
    /// Relative speed (cm/yr) of two blocks of a plate above which the zone between them
    /// becomes a plate boundary.
    pub split_speed: f64,
    /// Time (Myr) for which the speed must be exceeded.
    pub split_time: f64,
    /// How plate edges move with the material (see [`Simulation::move_fronts`]): 0 not at
    /// all, 1 only retreating, 2 retreating and advancing.
    pub fronts: u8,
    /// Relative residual at which the solve stops.
    pub tolerance: f64,
    /// Most conjugate-gradient iterations per step.
    pub max_iterations: usize,
}

impl Default for SheetParams {
    fn default() -> Self {
        Self {
            enabled: true,
            strong_viscosity: 2e31,
            strength_exponent: 6.0,
            continental_strength: 0.5,
            stress_exponent: 3.0,
            reference_strain_rate: 1e-15,
            rigid_viscosity: 1e28,
            min_viscosity: 1e27,
            max_softening: 5.0,
            oceanic_min_strength: 0.35,
            ridge_push_age: 80.0,
            gpe_factor: 2.0,
            split_speed: 1.5,
            split_time: 5.0,
            fronts: 2,
            tolerance: 1e-3,
            max_iterations: 200,
        }
    }
}

impl SheetParams {
    /// Sets the parameter `name` (without the `sheet.` prefix); `enabled` is off for 0.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        let field = match name {
            "enabled" => {
                self.enabled = value != 0.0;
                return Ok(());
            }
            "fronts" => {
                self.fronts = value.clamp(0.0, 2.0) as u8;
                return Ok(());
            }
            "max_iterations" => {
                self.max_iterations = value.max(0.0) as usize;
                return Ok(());
            }
            "strong_viscosity" => &mut self.strong_viscosity,
            "strength_exponent" => &mut self.strength_exponent,
            "continental_strength" => &mut self.continental_strength,
            "stress_exponent" => &mut self.stress_exponent,
            "reference_strain_rate" => &mut self.reference_strain_rate,
            "rigid_viscosity" => &mut self.rigid_viscosity,
            "min_viscosity" => &mut self.min_viscosity,
            "max_softening" => &mut self.max_softening,
            "oceanic_min_strength" => &mut self.oceanic_min_strength,
            "ridge_push_age" => &mut self.ridge_push_age,
            "gpe_factor" => &mut self.gpe_factor,
            "split_speed" => &mut self.split_speed,
            "split_time" => &mut self.split_time,
            "tolerance" => &mut self.tolerance,
            _ => anyhow::bail!("unknown sheet parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// The deformation state of a plate, per local vertex (as [`Plate::crust`]).
#[derive(Clone, Debug)]
pub(super) struct PlateSheet {
    /// Deformation velocity `u` in the plate frame (tangent, rad/Myr like `ω × r`).
    pub(super) velocity: Vec<DVec3>,
    /// Position of the material front of an edge sample: the fraction of its cell the material
    /// covers, 1 for a full cell (see [`Simulation::move_fronts`]).
    pub(super) front: Vec<f32>,
    /// Second invariant of the strain rate (1/s).
    pub(super) strain_rate: Vec<f32>,
    /// Rate of area change (1/s; positive where the sheet stretches).
    pub(super) dilatation: Vec<f32>,
    /// Effective viscosity (Pa·s·m) of the last solve (NaN if unknown).
    pub(super) viscosity: Vec<f32>,
    /// The local vertices with a strain rate (non-zero entries of `strain_rate`).
    pub(super) strained: Vec<u32>,
    /// Whether any sample has a deformation velocity.
    pub(super) moving: bool,
    /// The local vertices with a deformation velocity, in order.
    pub(super) active: Vec<u32>,
    /// The rotation (plate frame, m/s per unit radius) of the largest block that the frame
    /// took over at the last solve, for the warm start of the next one.
    pub(super) frame: DVec3,
    /// How long (Myr) two blocks of the plate have moved relative to each other faster than
    /// [`SheetParams::split_speed`].
    pub(super) split_timer: f64,
    /// The local vertices of the part to split off, once due.
    pub(super) pending_split: Option<Vec<u32>>,
}

impl PlateSheet {
    pub(super) fn new(cell_count: usize) -> Self {
        Self {
            velocity: vec![DVec3::ZERO; cell_count],
            front: vec![1.0; cell_count],
            strain_rate: vec![0.0; cell_count],
            dilatation: vec![0.0; cell_count],
            viscosity: vec![f32::NAN; cell_count],
            strained: Vec::new(),
            moving: false,
            active: Vec::new(),
            frame: DVec3::ZERO,
            split_timer: 0.0,
            pending_split: None,
        }
    }

    /// Forgets the state of a local vertex (crust newly placed there).
    pub(super) fn reset(&mut self, local: usize) {
        self.velocity[local] = DVec3::ZERO;
        self.front[local] = 1.0;
        self.strain_rate[local] = 0.0;
        self.dilatation[local] = 0.0;
        self.viscosity[local] = f32::NAN;
    }
}

/// Mesh geometry for the sheet: per vertex, its neighbours (in the order of
/// [`SphereMesh::neighbors`]) with the unit chord towards them, the chord length and the length
/// of the dual (Voronoi) face between them, all on the unit sphere, and a tangent basis.
#[derive(Clone, Debug, Default)]
pub(super) struct SheetGeometry {
    offsets: Vec<u32>,
    neighbor: Vec<u32>,
    direction: Vec<DVec3>,
    face: Vec<f64>,
    basis: Vec<(DVec3, DVec3)>,
    /// Area of a cell on the unit sphere (uniform, as everywhere in the model).
    area: f64,
}

impl SheetGeometry {
    pub(super) fn new(mesh: &SphereMesh) -> Self {
        let count = mesh.len();
        let mut offsets = Vec::with_capacity(count + 1);
        let mut neighbor = Vec::new();
        let mut direction = Vec::new();
        offsets.push(0);
        for vertex in 0..count as u32 {
            let r = mesh.vertex(vertex);
            for &other in mesh.neighbors(vertex) {
                let chord = mesh.vertex(other) - r;
                neighbor.push(other);
                direction.push(chord.normalize());
            }
            offsets.push(neighbor.len() as u32);
        }
        // Dual face lengths: the distance between the circumcentres of the two triangles that
        // share an edge.
        let mut centres: Vec<[DVec3; 2]> = vec![[DVec3::ZERO; 2]; neighbor.len()];
        let mut seen = vec![0u8; neighbor.len()];
        let slot = |a: u32, b: u32| -> usize {
            let start = offsets[a as usize] as usize;
            let end = offsets[a as usize + 1] as usize;
            start
                + neighbor[start..end]
                    .iter()
                    .position(|&n| n == b)
                    .expect("triangle edges are mesh edges")
        };
        for &[a, b, c] in mesh.faces() {
            let (ra, rb, rc) = (mesh.vertex(a), mesh.vertex(b), mesh.vertex(c));
            let mut centre = (rb - ra).cross(rc - ra).normalize();
            if centre.dot(ra) < 0.0 {
                centre = -centre;
            }
            for (x, y) in [(a, b), (b, c), (c, a), (b, a), (c, b), (a, c)] {
                let index = slot(x, y);
                let k = seen[index].min(1) as usize;
                centres[index][k] = centre;
                seen[index] += 1;
            }
        }
        let face = centres
            .iter()
            .zip(&seen)
            .map(|(pair, &n)| {
                if n >= 2 {
                    (pair[0] - pair[1]).length()
                } else {
                    0.0
                }
            })
            .collect();
        let basis = (0..count as u32)
            .map(|vertex| {
                let r = mesh.vertex(vertex);
                let helper = if r.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
                let e1 = (helper - r * helper.dot(r)).normalize();
                (e1, r.cross(e1))
            })
            .collect();
        Self {
            offsets,
            neighbor,
            direction,
            face,
            basis,
            area: 4.0 * std::f64::consts::PI / count as f64,
        }
    }

    /// The edge slots of `vertex` (indices into the per-edge arrays).
    fn edges(&self, vertex: u32) -> std::ops::Range<usize> {
        self.offsets[vertex as usize] as usize..self.offsets[vertex as usize + 1] as usize
    }
}

/// Statistics of the deformation at the last step.
#[derive(Clone, Debug, Default)]
pub struct SheetStatistics {
    /// Fractions of the surface straining faster than 10⁻¹⁶ and 10⁻¹⁵ s⁻¹ …
    pub area_1e16: f64,
    pub area_1e15: f64,
    /// … of it on crust of continental origin (fraction of the surface).
    pub continental_area_1e16: f64,
    /// Fraction of the surface straining faster than 10⁻¹⁶ s⁻¹ more than two cells from a plate
    /// boundary (plate interiors).
    pub interior_area_1e16: f64,
    /// Fraction of the continental crust straining faster than 10⁻¹⁶ s⁻¹.
    pub continental_share_1e16: f64,
    /// Samples that are weak (unknowns of the solve) and rigid blocks.
    pub weak_samples: usize,
    pub blocks: usize,
    /// RMS and largest deformation speed (cm/yr) over weak samples.
    pub rms_speed: f64,
    pub max_speed: f64,
    /// Conjugate-gradient iterations of the last solve and their relative residual.
    pub iterations: usize,
    pub residual: f64,
    /// Totals so far: iterations, solves, unconverged solves, fronts vacated and grown, splits.
    pub total_iterations: u64,
    pub solves: u64,
    pub unconverged: u64,
    pub vacated: u64,
    pub grown: u64,
    pub splits: u64,
    /// Wall time (s) spent in the solve and in the transport so far.
    pub solve_seconds: f64,
    pub transport_seconds: f64,
    /// At converging collision cells, the mean convergence (cm/yr) of the plates' rotations
    /// and of the material at the boundary (rotations plus deformation): the difference is
    /// taken up by the deforming margins instead of the boundary.
    pub collision_convergence: (f64, f64),
    /// Mass moved by the transport so far (areal mass summed over samples, kg/m²).
    pub moved: f64,
}

/// Accumulates columns of crust mixed together (by mass, as the other mixing code does).
#[derive(Clone, Copy, Default)]
struct Mixture {
    thickness: f64,
    mass: f64,
    felsic: f64,
    metasediment: f64,
    sediment: f64,
    cohort: f64,
    orogenic: f64,
    orogeny: f64,
    reheated: f64,
    heat: f64,
    oldest: f64,
}

impl Mixture {
    fn new() -> Self {
        Self {
            orogeny: NO_OROGENY,
            reheated: NO_REHEATING,
            oldest: f64::INFINITY,
            ..Self::default()
        }
    }

    /// Adds the fraction `share` of `crust` at time `time`.
    fn add(&mut self, crust: &Crust, share: f64, time: f64) {
        if share <= 0.0 {
            return;
        }
        let thickness = crust.thickness * share;
        let mass = thickness * crust.density;
        self.thickness += thickness;
        self.mass += mass;
        self.felsic += mass * crust.felsic;
        self.metasediment += mass * crust.metasediment;
        let sediment = crust.sediment * share;
        self.sediment += sediment;
        self.cohort += sediment * crust.cohort;
        self.orogenic += mass * crust.orogenic;
        self.orogeny = self.orogeny.max(crust.orogeny);
        self.reheated = self.reheated.max(crust.reheated);
        self.heat += mass.max(1e-9 * share) * thermal_fraction(crust.age(time));
        self.oldest = self.oldest.min(crust.birth);
    }

    fn crust(&self, time: f64) -> Crust {
        let mass = self.mass.max(1e-9);
        let weight = self.mass.max(1e-9);
        let fraction = self.heat / weight;
        let birth = age_for_thermal_fraction(fraction).map_or(self.oldest, |age| {
            // Never older than the oldest contributor.
            (time - age).max(self.oldest)
        });
        let mut crust = Crust {
            thickness: self.thickness,
            density: if self.thickness > 0.0 {
                self.mass / self.thickness
            } else {
                crate::isostasy::OCEANIC_DENSITY
            },
            birth,
            sediment: self.sediment,
            felsic: (self.felsic / mass).clamp(0.0, 1.0),
            metasediment: (self.metasediment / mass).clamp(0.0, 1.0),
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: self.reheated,
            cohort: if self.sediment > 0.0 {
                self.cohort / self.sediment
            } else {
                0.0
            },
        };
        crust.metasediment = crust.metasediment.min(crust.felsic);
        crust.mix_orogeny(self.orogeny, self.orogenic / mass);
        crust
    }
}

/// Scales a column to the share `share` of itself (thickness and sediment; the rest is
/// unchanged).
fn scaled(crust: &Crust, share: f64) -> Crust {
    Crust {
        thickness: crust.thickness * share,
        sediment: crust.sediment * share,
        ..*crust
    }
}

/// Least-squares gradient (in the tangent basis `(e1, e2)` at `r`) of a quantity known at `r`
/// (`value`) and at neighbours (`(position, value)`); `None` with fewer than three neighbours or
/// neighbours nearly on a line.
fn gradient(
    r: DVec3,
    (e1, e2): (DVec3, DVec3),
    value: f64,
    neighbours: &[(DVec3, f64)],
) -> Option<DVec3> {
    if neighbours.len() < 3 {
        return None;
    }
    let mut normal = DMat2::ZERO;
    let mut rhs = DVec2::ZERO;
    for &(position, other) in neighbours {
        let d = position - r;
        let x = DVec2::new(d.dot(e1), d.dot(e2));
        normal += DMat2::from_cols(x * x.x, x * x.y);
        rhs += x * (other - value);
    }
    // Neighbours nearly on a line give no gradient across it.
    let trace = normal.x_axis.x + normal.y_axis.y;
    if normal.determinant() < 0.1 * 0.25 * trace * trace {
        return None;
    }
    let g = normal.inverse() * rhs;
    Some(e1 * g.x + e2 * g.y)
}

/// One node of the linear system: a crust sample that is weak (two unknowns) or a rigid sample
/// at the edge of its block (its velocity follows the block's rotation).
#[derive(Clone, Copy)]
enum Node {
    /// Weak sample: index of its first unknown.
    Weak(usize),
    /// Rigid sample: index of its block's first unknown.
    Rigid(usize),
}

impl Simulation {
    /// The relative strength (0–1) of the lithosphere under `crust` for the sheet: the
    /// thermal state (thermal age since formation, stretching or plume reheating; see
    /// [`Crust::strength_age`]) and the remaining orogenic weakening of the strength proxy
    /// ([`super::weak_zones::lithosphere_strength`]), times the weaker creep of continental lithosphere and the
    /// weakening by thickened crust, whose hot lower crust replaces the strong mantle lid
    /// (`(h₀/h)²` above the reference thickness). Unlike the proxy for where plates break, thin
    /// (stretched) continental crust is not weaker in itself: once its lithosphere has cooled
    /// it carries less weak crust over the same mantle lid (passive margins are strong).
    pub(super) fn sheet_strength(&self, crust: &Crust) -> f64 {
        let mut strength = super::weak_zones::combine_strength(
            super::weak_zones::thermal_strength(crust.strength_age(self.time)),
            1.0,
            crust.orogenic,
        );
        if crust.is_continental_origin() {
            strength *= self.params.sheet.continental_strength;
            let thickness = crust.thickness.max(1.0);
            let reference = deformation::REFERENCE_THICKNESS;
            if thickness > reference {
                strength *= (reference / thickness).powi(2);
            }
        } else {
            strength = strength.max(self.params.sheet.oceanic_min_strength);
        }
        strength
    }

    /// Effective depth-integrated viscosity (Pa·s·m) of the lithosphere under `crust` straining
    /// at `strain_rate` (1/s): `B_strong · s^p`, softened above the reference strain rate by
    /// power-law creep.
    pub(super) fn sheet_viscosity(&self, crust: &Crust, strain_rate: f64) -> f64 {
        let params = &self.params.sheet;
        let strength = self.sheet_strength(crust).max(1e-6);
        let mut viscosity =
            params.strong_viscosity * math::powf(strength, params.strength_exponent);
        if strain_rate > params.reference_strain_rate && params.stress_exponent > 1.0 {
            viscosity *= math::powf(
                strain_rate / params.reference_strain_rate,
                1.0 / params.stress_exponent - 1.0,
            )
            .max(1.0 / params.max_softening);
        }
        viscosity.max(params.min_viscosity)
    }

    /// The force (N, world frame, tangent) acting on the lithosphere of every world cell of the
    /// `deforming` plates (zero elsewhere), from
    /// the last torque balance (see [`super::dynamics::ForceBalance`]): drag against the mantle,
    /// slab pull, slab resistance, shear coupling and the collision and yielding resistances at
    /// the solved plate velocities; ridge push spread over the young sea floor instead of the
    /// ridge cells; and the push of thickened continental crust.
    fn cell_forces(&self, deforming: &[bool]) -> Vec<DVec3> {
        let params = &self.params;
        let radius = params.planet_radius;
        let cell_count = self.mesh.len();
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / cell_count as f64;
        let mut force = vec![DVec3::ZERO; cell_count];
        let Some(balance) = self.force_balance.as_ref() else {
            return force;
        };
        let velocity = |plate: u32, r: DVec3| {
            self.plates[plate as usize].angular_velocity.cross(r) * (radius / SECONDS_PER_MYR)
        };
        let crusts: Vec<Crust> = (0..cell_count as u32)
            .map(|cell| self.surface_crust(cell))
            .collect();
        // Basal drag at the rigid velocity.
        for cell in 0..cell_count as u32 {
            let r = self.mesh.vertex(cell);
            let plate = self.owners[cell as usize].plate;
            if !deforming[plate as usize] {
                continue;
            }
            let coefficient = params.basal_drag
                * if crusts[cell as usize].is_continental() {
                    params.continental_drag_factor
                } else {
                    1.0
                }
                * cell_area;
            let mantle = balance.mantle_velocity[cell as usize];
            let mantle = mantle - r * mantle.dot(r);
            force[cell as usize] += (mantle - velocity(plate, r)) * coefficient;
        }
        // The relative deformation velocity (m/s) across a boundary, as in the torque balance.
        let deformation = |boundary: &dynamics::BoundaryCell| {
            (self.deformation_velocity_absolute(boundary.cell)
                - self.deformation_velocity_absolute(boundary.neighbor))
                * (radius / SECONDS_PER_MYR)
        };
        let length_per_cell = self.boundary_length_per_cell();
        for (index, boundary) in balance.boundaries.iter().enumerate() {
            let r = self.mesh.vertex(boundary.cell);
            let cell = boundary.cell as usize;
            if boundary.kind != BoundaryKind::Ridge {
                force[cell] += self.boundary_force(boundary);
            }
            let resistance = self.slab_resistance(boundary);
            if resistance > 0.0 {
                let mantle = balance.mantle_velocity[cell];
                let speed = (velocity(boundary.plate, r) - mantle).dot(boundary.normal);
                force[cell] -= boundary.normal * (resistance * speed);
            }
            let k = 0.5 * params.basal_drag * params.boundary_coupling_width * length_per_cell;
            if k > 0.0 {
                let t = r.cross(balance.normals[index]);
                // The rotations only: the deformation's part is in the sheet's solve.
                let relative = velocity(boundary.plate, r) - velocity(boundary.other, r);
                let shear = t * (k * relative.dot(t));
                force[cell] -= shear;
                force[boundary.neighbor as usize] += shear;
            }
        }
        for &index in &balance.active_collisions {
            let boundary = &balance.boundaries[index];
            let r = self.mesh.vertex(boundary.cell);
            let n = balance.normals[index];
            let relative =
                velocity(boundary.plate, r) - velocity(boundary.other, r) + deformation(boundary);
            force[boundary.cell as usize] -= n * (balance.collision_coupling * relative.dot(n));
        }
        for &(index, coupling) in &balance.nascent {
            let boundary = &balance.boundaries[index];
            let r = self.mesh.vertex(boundary.cell);
            let n = balance.normals[index];
            let relative = velocity(boundary.plate, r) - velocity(boundary.other, r);
            let resisting = n * (coupling * relative.dot(n));
            force[boundary.cell as usize] -= resisting;
            force[boundary.neighbor as usize] += resisting;
        }
        // Ridge push as the gradient of the potential energy of cooling sea floor, which falls
        // linearly with age until `ridge_push_age` (Turcotte & Schubert): the force per unit
        // length transmitted at age a is `ridge_push · min(a / A, 1)`. And the potential energy
        // of thickened continental crust (`deformation::excess_potential_energy`).
        let sheet = &params.sheet;
        let mut neighbours: Vec<(DVec3, f64)> = Vec::new();
        for cell in 0..cell_count as u32 {
            let crust = &crusts[cell as usize];
            let plate = self.owners[cell as usize].plate;
            if !deforming[plate as usize] {
                continue;
            }
            let r = self.mesh.vertex(cell);
            let basis = self.sheet_geometry.basis[cell as usize];
            let push = |crust: &Crust| {
                if crust.is_oceanic() {
                    params.ridge_push * (crust.age(self.time) / sheet.ridge_push_age).min(1.0)
                } else {
                    params.ridge_push
                }
            };
            if crust.is_oceanic() && crust.age(self.time) < sheet.ridge_push_age {
                neighbours.clear();
                for &n in self.mesh.neighbors(cell) {
                    if self.owners[n as usize].plate == plate {
                        neighbours.push((self.mesh.vertex(n), push(&crusts[n as usize])));
                    }
                }
                if let Some(g) = gradient(r, basis, push(crust), &neighbours) {
                    // Per metre on the planet: the gradient is per unit of the unit sphere.
                    force[cell as usize] += g * (cell_area / radius);
                }
            }
            if sheet.gpe_factor > 0.0 {
                let energy = |crust: &Crust| {
                    if crust.is_continental_origin_column() {
                        excess_potential_energy(crust.thickness)
                    } else {
                        0.0
                    }
                };
                let own = energy(crust);
                neighbours.clear();
                let mut any = own > 0.0;
                // All neighbours, also those of other plates: a belt straddling a boundary
                // does not push across it (its other half pushes back).
                for &n in self.mesh.neighbors(cell) {
                    let e = energy(&crusts[n as usize]);
                    any |= e > 0.0;
                    neighbours.push((self.mesh.vertex(n), e));
                }
                if any && let Some(g) = gradient(r, basis, own, &neighbours) {
                    force[cell as usize] -= g * (sheet.gpe_factor * cell_area / radius);
                }
            }
        }
        force
    }

    /// Solves the deformation velocity of every plate (see the module documentation), updates
    /// the strain rates and lets each plate's frame follow its largest rigid block. Runs once
    /// per step, after the torque balance.
    pub(super) fn deform_plates(&mut self, dt: f64) {
        if !self.params.sheet.enabled {
            return;
        }
        let start = web_time::Instant::now();
        let params = self.params.clone();
        let params = &params;
        let sheet = &params.sheet;
        let radius = params.planet_radius;
        let cell_count = self.mesh.len();
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / cell_count as f64;
        let geometry = &self.sheet_geometry;
        let mesh = &self.mesh;
        let spring = 4.0 / 3f64.sqrt();
        let to_rad_per_myr = SECONDS_PER_MYR / radius;
        let plate_count = self.plates.len();

        // Per plate: its samples, and per sample (in the order of `samples`) the force (plate
        // frame), drag coefficient, viscosity, whether it is weak, its block and its node.
        struct Work {
            samples: Vec<u32>,
            /// Position of a local vertex in `samples` (u32::MAX: no sample).
            index: Vec<u32>,
            force: Vec<DVec3>,
            drag: Vec<f64>,
            viscosity: Vec<f64>,
            weak: Vec<bool>,
            block: Vec<u32>,
            node: Vec<u32>,
        }
        let mut work: Vec<Work> = self
            .plates
            .iter()
            .map(|plate| {
                let samples: Vec<u32> = (0..cell_count as u32)
                    .filter(|&local| plate.crust[local as usize].is_some())
                    .collect();
                let mut index = vec![u32::MAX; cell_count];
                for (i, &local) in samples.iter().enumerate() {
                    index[local as usize] = i as u32;
                }
                let n = samples.len();
                Work {
                    samples,
                    index,
                    force: vec![DVec3::ZERO; n],
                    drag: vec![0.0; n],
                    viscosity: vec![0.0; n],
                    weak: vec![false; n],
                    block: vec![u32::MAX; n],
                    node: vec![u32::MAX; n],
                }
            })
            .collect();
        for (plate, w) in self.plates.iter().zip(work.iter_mut()) {
            for (i, &local) in w.samples.iter().enumerate() {
                let l = local as usize;
                let crust = plate.crust[l].expect("samples have crust");
                let target = self.sheet_viscosity(&crust, plate.sheet.strain_rate[l] as f64);
                let old = plate.sheet.viscosity[l] as f64;
                // Under-relaxed (geometric mean): the strain rate is the last step's.
                w.viscosity[i] = if old.is_finite() && old > 0.0 {
                    (old * target).sqrt()
                } else {
                    target
                };
                w.drag[i] = params.basal_drag
                    * if crust.is_continental() {
                        params.continental_drag_factor
                    } else {
                        1.0
                    }
                    * cell_area;
                // Samples with fewer than three neighbours in the plate (slivers, necks one
                // cell wide) have no meaningful strain: they move with their surroundings.
                let neighbours = mesh
                    .neighbors(local)
                    .iter()
                    .filter(|&&n| w.index[n as usize] != u32::MAX)
                    .count();
                w.weak[i] = w.viscosity[i] < sheet.rigid_viscosity && neighbours >= 3;
            }
        }
        // Forces only for plates that can deform (a plate without weak samples is rigid).
        let deforming: Vec<bool> = work
            .iter()
            .map(|w| w.weak.iter().any(|&weak| weak))
            .collect();
        let forces = self.cell_forces(&deforming);
        for (cell, owner) in self.owners.iter().enumerate() {
            if deforming[owner.plate as usize] {
                let w = &mut work[owner.plate as usize];
                debug_assert!(
                    w.index[owner.local as usize] != u32::MAX,
                    "cell {cell} shows local vertex {} of plate {}, which has no crust",
                    owner.local,
                    owner.plate
                );
                let i = w.index[owner.local as usize] as usize;
                w.force[i] += self.plates[owner.plate as usize].rotation.inverse() * forces[cell];
            }
        }

        // The couplings across plate boundaries (shear coupling and the yielding resistances
        // of the torque balance): (boundary index, coefficient N·s/m, direction, world frame).
        let mut entries: Vec<(usize, f64, DVec3)> = Vec::new();
        // Per plate (its frame): the torque of the couplings on the last step's deformation,
        // which the torque balance included (lagged, see `update_plate_velocities`). The
        // forces of the rotations here are balanced against it, so it is not removed below.
        let mut lagged_torque = vec![DVec3::ZERO; plate_count];
        if let Some(balance) = self.force_balance.as_ref() {
            let k_shear = 0.5
                * params.basal_drag
                * params.boundary_coupling_width
                * self.boundary_length_per_cell();
            for (index, boundary) in balance.boundaries.iter().enumerate() {
                if k_shear > 0.0 {
                    let r = mesh.vertex(boundary.cell);
                    entries.push((index, k_shear, r.cross(balance.normals[index])));
                }
            }
            for &(index, coupling) in &balance.nascent {
                entries.push((index, coupling, balance.normals[index]));
            }
            for &(index, k, direction) in &entries {
                let boundary = &balance.boundaries[index];
                let relative = (self.deformation_velocity_absolute(boundary.cell)
                    - self.deformation_velocity_absolute(boundary.neighbor))
                    * (radius / SECONDS_PER_MYR);
                let force = direction * (-k * relative.dot(direction));
                for (cell, sign) in [(boundary.cell, 1.0), (boundary.neighbor, -1.0)] {
                    let plate = self.owners[cell as usize].plate as usize;
                    lagged_torque[plate] += self.plates[plate].rotation.inverse()
                        * (mesh.vertex(cell).cross(force) * sign);
                }
            }
        }

        let mut statistics = std::mem::take(&mut self.sheet_statistics);
        statistics.weak_samples = 0;
        let mut unknowns = 0usize;
        let mut nodes: Vec<(u32, u32, Node)> = Vec::new();
        // Per block: plate, first unknown, drag matrix, generalised force, sample count.
        let mut blocks: Vec<(u32, usize, DMat3, DVec3, usize)> = Vec::new();
        // Per weak node: drag coefficient and force (tangent basis).
        let mut drag: Vec<f64> = Vec::new();
        let mut rhs: Vec<f64> = Vec::new();
        let mut stack = Vec::new();

        for (p, w) in work.iter_mut().enumerate() {
            if !deforming[p] {
                continue;
            }
            // Remove the net torque of the forces (drag-weighted rigid rotation).
            let (mut matrix, mut torque) = (DMat3::ZERO, DVec3::ZERO);
            for (i, &local) in w.samples.iter().enumerate() {
                let r = mesh.vertex(local);
                let normal = r * w.force[i].dot(r);
                w.force[i] -= normal;
                matrix += (DMat3::IDENTITY - dynamics::outer(r, r)) * w.drag[i];
                torque += r.cross(w.force[i]);
            }
            if matrix.determinant().abs() > 0.0 {
                let rotation = matrix.inverse() * (torque + lagged_torque[p]);
                for (i, &local) in w.samples.iter().enumerate() {
                    w.force[i] -= rotation.cross(mesh.vertex(local)) * w.drag[i];
                }
            }
            // Rigid blocks: connected components of rigid samples.
            for first in 0..w.samples.len() {
                if w.weak[first] || w.block[first] != u32::MAX {
                    continue;
                }
                let id = blocks.len() as u32;
                w.block[first] = id;
                stack.push(first as u32);
                let (mut m, mut f, mut count) = (DMat3::ZERO, DVec3::ZERO, 0);
                while let Some(i) = stack.pop() {
                    let i = i as usize;
                    let r = mesh.vertex(w.samples[i]);
                    m += (DMat3::IDENTITY - dynamics::outer(r, r)) * w.drag[i];
                    f += r.cross(w.force[i]);
                    count += 1;
                    for &n in mesh.neighbors(w.samples[i]) {
                        let j = w.index[n as usize];
                        if j != u32::MAX && !w.weak[j as usize] && w.block[j as usize] == u32::MAX {
                            w.block[j as usize] = id;
                            stack.push(j);
                        }
                    }
                }
                // A small drag against spinning about the block's own axis keeps blocks of a
                // single sample (and nearly so) positive definite.
                let m = m + DMat3::IDENTITY * (1e-3 * params.basal_drag * cell_area);
                blocks.push((p as u32, unknowns, m, f, count));
                unknowns += 3;
            }
            // Nodes: weak samples and rigid samples next to a weak one.
            for (i, &local) in w.samples.iter().enumerate() {
                let kind = if w.weak[i] {
                    let first = unknowns;
                    unknowns += 2;
                    let (e1, e2) = geometry.basis[local as usize];
                    drag.push(w.drag[i]);
                    rhs.push(w.force[i].dot(e1));
                    rhs.push(w.force[i].dot(e2));
                    statistics.weak_samples += 1;
                    Node::Weak(first)
                } else if mesh.neighbors(local).iter().any(|&n| {
                    let j = w.index[n as usize];
                    j != u32::MAX && w.weak[j as usize]
                }) {
                    Node::Rigid(blocks[w.block[i] as usize].1)
                } else {
                    continue;
                };
                w.node[i] = nodes.len() as u32;
                nodes.push((p as u32, local, kind));
            }
        }
        statistics.blocks = blocks.len();

        // Couplings across plate boundaries (shear coupling and the yielding resistances of the
        // torque balance), acting on the deformation velocities of both sides: the part of the
        // resisting force that comes from the rotations is in the forces already. (node or
        // u32::MAX for a side that does not deform, coefficient, direction in each side's
        // frame)
        let mut couplings: Vec<(u32, u32, f64, DVec3, DVec3)> = Vec::new();
        if let Some(balance) = self.force_balance.as_ref() {
            for &(index, k, direction) in &entries {
                let boundary = &balance.boundaries[index];
                let mut sides = [u32::MAX; 2];
                let mut frames = [DVec3::ZERO; 2];
                for (side, cell) in [boundary.cell, boundary.neighbor].into_iter().enumerate() {
                    let owner = self.owners[cell as usize];
                    let w = &mut work[owner.plate as usize];
                    frames[side] = self.plates[owner.plate as usize].rotation.inverse() * direction;
                    debug_assert!(
                        w.index[owner.local as usize] != u32::MAX,
                        "boundary cell {cell} shows a sample without crust"
                    );
                    let i = w.index[owner.local as usize] as usize;
                    if w.node[i] == u32::MAX && w.block[i] != u32::MAX {
                        w.node[i] = nodes.len() as u32;
                        nodes.push((
                            owner.plate,
                            owner.local,
                            Node::Rigid(blocks[w.block[i] as usize].1),
                        ));
                    }
                    sides[side] = w.node[i];
                }
                if sides != [u32::MAX; 2] {
                    couplings.push((sides[0], sides[1], k, frames[0], frames[1]));
                }
            }
        }

        // Edges between nodes of the same plate (each once), with their spring constants.
        // (node a, node b, k, unit chord in the plate frame)
        let mut edges: Vec<(u32, u32, f64, DVec3)> = Vec::new();
        for (index, &(p, local, kind)) in nodes.iter().enumerate() {
            let w = &work[p as usize];
            let i = w.index[local as usize] as usize;
            for slot in geometry.edges(local) {
                let j = w.index[geometry.neighbor[slot] as usize];
                if j == u32::MAX {
                    continue;
                }
                let other = w.node[j as usize];
                if other == u32::MAX || (other as usize) < index {
                    continue;
                }
                if let (Node::Rigid(a), Node::Rigid(b)) = (kind, nodes[other as usize].2)
                    && a == b
                {
                    continue;
                }
                let (va, vb) = (w.viscosity[i], w.viscosity[j as usize]);
                let k = spring * 2.0 * va * vb / (va + vb);
                edges.push((index as u32, other, k, geometry.direction[slot]));
            }
        }

        // Right-hand side, and the block-Jacobi preconditioner.
        let mut b_vector = vec![0.0; unknowns];
        let mut weak_diag: Vec<DMat2> = Vec::with_capacity(drag.len());
        let mut node_weak: Vec<usize> = vec![usize::MAX; nodes.len()];
        for (index, &(_, _, kind)) in nodes.iter().enumerate() {
            if let Node::Weak(first) = kind {
                let k = weak_diag.len();
                b_vector[first] = rhs[2 * k];
                b_vector[first + 1] = rhs[2 * k + 1];
                weak_diag.push(DMat2::from_diagonal(DVec2::splat(drag[k])));
                node_weak[index] = k;
            }
        }
        let mut block_diag: Vec<DMat3> = blocks.iter().map(|block| block.2).collect();
        let mut block_index_of_first = vec![usize::MAX; unknowns];
        for (index, block) in blocks.iter().enumerate() {
            b_vector[block.1] = block.3.x;
            b_vector[block.1 + 1] = block.3.y;
            b_vector[block.1 + 2] = block.3.z;
            block_index_of_first[block.1] = index;
        }
        for &(a, b, k, e) in &edges {
            for node in [a, b] {
                let (_, local, kind) = nodes[node as usize];
                match kind {
                    Node::Weak(_) => {
                        let (e1, e2) = geometry.basis[local as usize];
                        let t = DVec2::new(e.dot(e1), e.dot(e2));
                        weak_diag[node_weak[node as usize]] +=
                            DMat2::from_cols(t * t.x, t * t.y) * k;
                    }
                    Node::Rigid(first) => {
                        let m = mesh.vertex(local).cross(e);
                        block_diag[block_index_of_first[first]] += dynamics::outer(m, m) * k;
                    }
                }
            }
        }
        for &(a, b, k, da, db) in &couplings {
            for (node, d) in [(a, da), (b, db)] {
                if node == u32::MAX {
                    continue;
                }
                let (_, local, kind) = nodes[node as usize];
                match kind {
                    Node::Weak(_) => {
                        let (e1, e2) = geometry.basis[local as usize];
                        let t = DVec2::new(d.dot(e1), d.dot(e2));
                        weak_diag[node_weak[node as usize]] +=
                            DMat2::from_cols(t * t.x, t * t.y) * k;
                    }
                    Node::Rigid(first) => {
                        let m = mesh.vertex(local).cross(d);
                        block_diag[block_index_of_first[first]] += dynamics::outer(m, m) * k;
                    }
                }
            }
        }
        let weak_inverse: Vec<DMat2> = weak_diag.iter().map(|m| m.inverse()).collect();
        let block_inverse: Vec<DMat3> = block_diag
            .iter()
            .map(|m| {
                if m.determinant().abs() > 0.0 {
                    m.inverse()
                } else {
                    DMat3::ZERO
                }
            })
            .collect();

        // Node velocity (m/s, plate frame) from the unknowns.
        let node_velocity = |x: &[f64], node: usize| -> DVec3 {
            let (_, local, kind) = nodes[node];
            match kind {
                Node::Weak(first) => {
                    let (e1, e2) = geometry.basis[local as usize];
                    e1 * x[first] + e2 * x[first + 1]
                }
                Node::Rigid(first) => {
                    DVec3::new(x[first], x[first + 1], x[first + 2]).cross(mesh.vertex(local))
                }
            }
        };
        let mut velocities = vec![DVec3::ZERO; nodes.len()];
        let mut apply = |x: &[f64], y: &mut [f64]| {
            for (node, velocity) in velocities.iter_mut().enumerate() {
                *velocity = node_velocity(x, node);
            }
            y.fill(0.0);
            for (index, &(_, _, kind)) in nodes.iter().enumerate() {
                if let Node::Weak(first) = kind {
                    let c = drag[node_weak[index]];
                    y[first] += c * x[first];
                    y[first + 1] += c * x[first + 1];
                }
            }
            for block in &blocks {
                let w = DVec3::new(x[block.1], x[block.1 + 1], x[block.1 + 2]);
                let f = block.2 * w;
                y[block.1] += f.x;
                y[block.1 + 1] += f.y;
                y[block.1 + 2] += f.z;
            }
            let scatter = |y: &mut [f64], node: u32, f: DVec3| {
                let (_, local, kind) = nodes[node as usize];
                match kind {
                    Node::Weak(first) => {
                        let (e1, e2) = geometry.basis[local as usize];
                        y[first] += f.dot(e1);
                        y[first + 1] += f.dot(e2);
                    }
                    Node::Rigid(first) => {
                        let t = mesh.vertex(local).cross(f);
                        y[first] += t.x;
                        y[first + 1] += t.y;
                        y[first + 2] += t.z;
                    }
                }
            };
            for &(a, b, k, e) in &edges {
                let delta = velocities[b as usize] - velocities[a as usize];
                let s = k * delta.dot(e);
                // (K u)_a = −k ((u_b − u_a) · e) e, (K u)_b = +k (…) e.
                scatter(y, a, e * -s);
                scatter(y, b, e * s);
            }
            for &(a, b, k, da, db) in &couplings {
                let along = |node: u32, d: DVec3| {
                    if node == u32::MAX {
                        0.0
                    } else {
                        velocities[node as usize].dot(d)
                    }
                };
                let s = k * (along(b, db) - along(a, da));
                if a != u32::MAX {
                    scatter(y, a, da * -s);
                }
                if b != u32::MAX {
                    scatter(y, b, db * s);
                }
            }
        };
        let precondition = |r: &[f64], z: &mut [f64]| {
            for (index, &(_, _, kind)) in nodes.iter().enumerate() {
                if let Node::Weak(first) = kind {
                    let v = weak_inverse[node_weak[index]] * DVec2::new(r[first], r[first + 1]);
                    z[first] = v.x;
                    z[first + 1] = v.y;
                }
            }
            for (index, block) in blocks.iter().enumerate() {
                let v =
                    block_inverse[index] * DVec3::new(r[block.1], r[block.1 + 1], r[block.1 + 2]);
                z[block.1] = v.x;
                z[block.1 + 1] = v.y;
                z[block.1 + 2] = v.z;
            }
        };

        // The operator must be symmetric (and positive definite) for conjugate gradients.
        #[cfg(debug_assertions)]
        if unknowns > 0 {
            let a: Vec<f64> = (0..unknowns)
                .map(|i| ((i * 7919 % 101) as f64) - 50.0)
                .collect();
            let b: Vec<f64> = (0..unknowns)
                .map(|i| ((i * 104729 % 97) as f64) - 48.0)
                .collect();
            let (mut aa, mut ab) = (vec![0.0; unknowns], vec![0.0; unknowns]);
            apply(&a, &mut aa);
            apply(&b, &mut ab);
            let x: f64 = aa.iter().zip(&b).map(|(p, q)| p * q).sum();
            let y: f64 = ab.iter().zip(&a).map(|(p, q)| p * q).sum();
            let scale: f64 = aa.iter().map(|v| v.abs()).sum::<f64>() * 100.0;
            debug_assert!(
                (x - y).abs() <= 1e-9 * scale,
                "asymmetric sheet operator: {x} vs {y}"
            );
            let positive: f64 = aa.iter().zip(&a).map(|(p, q)| p * q).sum();
            debug_assert!(positive > 0.0, "sheet operator not positive: {positive}");
        }

        // Warm start from the last step's velocities (relative to the plate's mean rotation).
        let mut x = vec![0.0; unknowns];
        let previous = |p: u32, local: u32| {
            let sheet = &self.plates[p as usize].sheet;
            sheet.velocity[local as usize] / to_rad_per_myr + sheet.frame.cross(mesh.vertex(local))
        };
        for &(p, local, kind) in &nodes {
            if let Node::Weak(first) = kind {
                let u = previous(p, local);
                let (e1, e2) = geometry.basis[local as usize];
                x[first] = u.dot(e1);
                x[first + 1] = u.dot(e2);
            }
        }
        let mut block_torque = vec![DVec3::ZERO; blocks.len()];
        for (p, w) in work.iter().enumerate() {
            for (i, &local) in w.samples.iter().enumerate() {
                if w.block[i] != u32::MAX {
                    let r = mesh.vertex(local);
                    block_torque[w.block[i] as usize] +=
                        r.cross(previous(p as u32, local)) * w.drag[i];
                }
            }
        }
        for (block, torque) in blocks.iter().zip(&block_torque) {
            if block.2.determinant().abs() > 0.0 {
                let rotation = block.2.inverse() * *torque;
                x[block.1] = rotation.x;
                x[block.1 + 1] = rotation.y;
                x[block.1 + 2] = rotation.z;
            }
        }

        // Preconditioned conjugate gradients.
        let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
        let norm_b = dot(&b_vector, &b_vector).sqrt();
        let mut r = vec![0.0; unknowns];
        apply(&x, &mut r);
        for (r, b) in r.iter_mut().zip(&b_vector) {
            *r = b - *r;
        }
        if dot(&r, &r).sqrt() > norm_b {
            // The warm start is worse than none (the plates changed).
            x.fill(0.0);
            r.copy_from_slice(&b_vector);
        }
        let mut z = vec![0.0; unknowns];
        precondition(&r, &mut z);
        let mut d = z.clone();
        let mut rz = dot(&r, &z);
        let mut q = vec![0.0; unknowns];
        let mut iterations = 0;
        let mut residual = if norm_b > 0.0 {
            dot(&r, &r).sqrt() / norm_b
        } else {
            0.0
        };
        while residual > sheet.tolerance && iterations < sheet.max_iterations {
            apply(&d, &mut q);
            let dq = dot(&d, &q);
            if dq <= 0.0 {
                break;
            }
            let alpha = rz / dq;
            for i in 0..unknowns {
                x[i] += alpha * d[i];
                r[i] -= alpha * q[i];
            }
            precondition(&r, &mut z);
            let rz_new = dot(&r, &z);
            let beta = rz_new / rz;
            rz = rz_new;
            for i in 0..unknowns {
                d[i] = z[i] + beta * d[i];
            }
            iterations += 1;
            residual = dot(&r, &r).sqrt() / norm_b;
        }
        statistics.iterations = iterations;
        statistics.residual = residual;
        statistics.total_iterations += iterations as u64;
        statistics.solves += 1;
        if residual > sheet.tolerance {
            statistics.unconverged += 1;
        }

        // Velocities of all samples (m/s, plate frame); the frame follows the largest block.
        let split_speed = sheet.split_speed / 100.0 / (SECONDS_PER_MYR / 1e6);
        let min_block = (params.events.min_plate_area * cell_count as f64).ceil() as usize;
        let (mut speed_sum, mut speed_max, mut speed_count) = (0.0f64, 0.0f64, 0usize);
        // Blocks per plate, in order.
        let mut plate_blocks: Vec<Vec<usize>> = vec![Vec::new(); plate_count];
        for (index, block) in blocks.iter().enumerate() {
            plate_blocks[block.0 as usize].push(index);
        }
        // Per block, its relative speed (m/s) to the frame at its edge samples (nodes).
        let mut block_edge: Vec<Vec<u32>> = vec![Vec::new(); blocks.len()];
        for &(_, local, kind) in &nodes {
            if let Node::Rigid(first) = kind {
                block_edge[block_index_of_first[first]].push(local);
            }
        }
        let block_rotation = |index: usize| {
            let first = blocks[index].1;
            DVec3::new(x[first], x[first + 1], x[first + 2])
        };
        for (p, w) in work.iter().enumerate() {
            let main = plate_blocks[p]
                .iter()
                .copied()
                .max_by(|&a, &b| blocks[a].4.cmp(&blocks[b].4).then(b.cmp(&a)));
            // Only a block that is a substantial part of the plate carries the frame (a small
            // one's rotation is poorly constrained, e.g. its spin about its own axis).
            let main = main.filter(|&index| {
                blocks[index].4 >= min_block.max(1) && 5 * blocks[index].4 >= w.samples.len()
            });
            let frame = main.map_or(DVec3::ZERO, block_rotation);
            let mut velocity = vec![DVec3::ZERO; w.samples.len()];
            let mut moving = false;
            for (i, &local) in w.samples.iter().enumerate() {
                let r = mesh.vertex(local);
                let u = if w.block[i] != u32::MAX {
                    block_rotation(w.block[i] as usize).cross(r) - frame.cross(r)
                } else if w.node[i] != u32::MAX {
                    node_velocity(&x, w.node[i] as usize) - frame.cross(r)
                } else {
                    // A rigid plate.
                    DVec3::ZERO
                };
                moving |= u != DVec3::ZERO;
                if w.weak[i] {
                    let speed = u.length();
                    speed_sum += speed * speed;
                    speed_max = speed_max.max(speed);
                    speed_count += 1;
                }
                velocity[i] = u;
            }
            // Strain rates (from u; rigid rotations give none), for moving samples and their
            // neighbours.
            let mut strain: Vec<(u32, f32, f32)> = Vec::new();
            if moving {
                for (i, &local) in w.samples.iter().enumerate() {
                    let near_moving = velocity[i] != DVec3::ZERO
                        || mesh.neighbors(local).iter().any(|&n| {
                            let j = w.index[n as usize];
                            j != u32::MAX && velocity[j as usize] != DVec3::ZERO
                        });
                    if !near_moving {
                        continue;
                    }
                    let r = mesh.vertex(local);
                    let (e1, e2) = geometry.basis[local as usize];
                    let (mut xx, mut vx) = (DMat2::ZERO, DMat2::ZERO);
                    let mut count = 0;
                    for &n in mesh.neighbors(local) {
                        let j = w.index[n as usize];
                        if j == u32::MAX {
                            continue;
                        }
                        let d = (mesh.vertex(n) - r) * radius;
                        let dx = DVec2::new(d.dot(e1), d.dot(e2));
                        let dv = velocity[j as usize] - velocity[i];
                        let dv = DVec2::new(dv.dot(e1), dv.dot(e2));
                        xx += DMat2::from_cols(dx * dx.x, dx * dx.y);
                        vx += DMat2::from_cols(dv * dx.x, dv * dx.y);
                        count += 1;
                    }
                    if count < 2 || xx.determinant().abs() < 1e-6 {
                        continue;
                    }
                    let g = vx * xx.inverse();
                    let (e11, e22) = (g.x_axis.x, g.y_axis.y);
                    let e12 = 0.5 * (g.y_axis.x + g.x_axis.y);
                    let e33 = -(e11 + e22);
                    let second = (0.5 * (e11 * e11 + e22 * e22 + e33 * e33) + e12 * e12).sqrt();
                    strain.push((local, second as f32, (e11 + e22) as f32));
                }
            }
            // Splits: blocks moving relative to the main block.
            let mut candidate: Option<(usize, f64)> = None;
            if let Some(main) = main
                && blocks[main].4 >= min_block
            {
                for &index in &plate_blocks[p] {
                    if index == main || blocks[index].4 < min_block {
                        continue;
                    }
                    let relative = block_rotation(index) - frame;
                    let fastest = block_edge[index]
                        .iter()
                        .map(|&local| relative.cross(mesh.vertex(local)).length())
                        .fold(0.0, f64::max);
                    if fastest > split_speed && candidate.is_none_or(|(_, s)| fastest > s) {
                        candidate = Some((index, fastest));
                    }
                }
            }
            let rotation = self.plates[p].rotation;
            let plate = &mut self.plates[p];
            plate.angular_velocity += rotation * frame * (SECONDS_PER_MYR / radius);
            // Velocities are non-zero only at the active samples, strain rates only at the
            // strained ones: clear the last step's, then set the new ones.
            // Every sample's velocity is written (also zeros: a sample grown at a front after
            // the last solve is not in `active`).
            for &local in &plate.sheet.active {
                plate.sheet.velocity[local as usize] = DVec3::ZERO;
            }
            for &local in &w.samples {
                plate.sheet.velocity[local as usize] = DVec3::ZERO;
            }
            for &local in &plate.sheet.strained {
                plate.sheet.strain_rate[local as usize] = 0.0;
                plate.sheet.dilatation[local as usize] = 0.0;
            }
            plate.sheet.active.clear();
            for (i, &local) in w.samples.iter().enumerate() {
                if velocity[i] != DVec3::ZERO {
                    plate.sheet.velocity[local as usize] = velocity[i] * to_rad_per_myr;
                    plate.sheet.active.push(local);
                }
                plate.sheet.viscosity[local as usize] = w.viscosity[i] as f32;
            }
            plate.sheet.strained.clear();
            for &(local, rate, dilatation) in &strain {
                if rate != 0.0 || dilatation != 0.0 {
                    plate.sheet.strain_rate[local as usize] = rate;
                    plate.sheet.dilatation[local as usize] = dilatation;
                    plate.sheet.strained.push(local);
                }
            }
            plate.sheet.moving = moving;
            plate.sheet.frame = frame;
            plate.sheet.pending_split = None;
            match candidate {
                Some((index, _)) => {
                    plate.sheet.split_timer += dt;
                    if plate.sheet.split_timer >= sheet.split_time {
                        // Each sample goes to the block whose motion is closer to its own.
                        let other = block_rotation(index) - frame;
                        let side: Vec<u32> = w
                            .samples
                            .iter()
                            .zip(&velocity)
                            .filter(|&(&local, u)| {
                                (*u - other.cross(mesh.vertex(local))).length_squared()
                                    < u.length_squared()
                            })
                            .map(|(&local, _)| local)
                            .collect();
                        plate.sheet.pending_split = Some(side);
                    }
                }
                None => plate.sheet.split_timer = 0.0,
            }
        }
        statistics.rms_speed = if speed_count > 0 {
            (speed_sum / speed_count as f64).sqrt() * 100.0 * (SECONDS_PER_MYR / 1e6)
        } else {
            0.0
        };
        statistics.max_speed = speed_max * 100.0 * (SECONDS_PER_MYR / 1e6);
        statistics.solve_seconds += start.elapsed().as_secs_f64();
        self.sheet_statistics = statistics;
        self.update_sheet_statistics();
    }

    /// Area statistics of the strain rate (see [`SheetStatistics`]).
    fn update_sheet_statistics(&mut self) {
        let cell_count = self.mesh.len();
        let (mut a16, mut a15, mut c16, mut i16, mut continental) = (0, 0, 0, 0, 0);
        // Cells within two cells of a plate boundary.
        let mut near = vec![false; cell_count];
        for cell in 0..cell_count as u32 {
            if self.at_plate_boundary(cell) {
                near[cell as usize] = true;
                for &n in self.mesh.neighbors(cell) {
                    near[n as usize] = true;
                }
            }
        }
        for cell in 0..cell_count as u32 {
            let owner = self.owners[cell as usize];
            let rate = self.plates[owner.plate as usize].sheet.strain_rate[owner.local as usize];
            let crust = self.surface_crust(cell);
            let is_continental = crust.is_continental_origin();
            continental += usize::from(is_continental);
            if rate >= 1e-16 {
                a16 += 1;
                c16 += usize::from(is_continental);
                i16 += usize::from(!near[cell as usize]);
            }
            a15 += usize::from(rate >= 1e-15);
        }
        let s = &mut self.sheet_statistics;
        let n = cell_count as f64;
        s.area_1e16 = a16 as f64 / n;
        s.area_1e15 = a15 as f64 / n;
        s.continental_area_1e16 = c16 as f64 / n;
        s.interior_area_1e16 = i16 as f64 / n;
        s.continental_share_1e16 = c16 as f64 / continental.max(1) as f64;
    }

    /// Moves crust between the samples of each plate by its deformation velocity over `dt` Myr
    /// (see the module documentation), then moves the plates' edges. Conserves mass.
    pub(super) fn transport_crust(&mut self, dt: f64) {
        if !self.params.sheet.enabled {
            return;
        }
        let start = web_time::Instant::now();
        let geometry = &self.sheet_geometry;
        let cell_count = self.mesh.len();
        let time = self.time;
        let max_thickness = self.params.max_crust_thickness;
        let (mut moved, mut change) = (0.0, 0.0);
        // Scratch per local vertex, reset after each use.
        let mut outflow = vec![0.0f64; cell_count];
        let mut incoming = vec![0.0f64; cell_count];
        let mut mixtures: Vec<Option<Mixture>> = vec![None; cell_count];
        let mut transfers: Vec<(u32, u32, f64)> = Vec::new();
        let mut parts: Vec<(u32, u32, f64)> = Vec::new();
        let mut touched: Vec<u32> = Vec::new();
        for plate in self.plates.iter_mut() {
            if !plate.sheet.moving {
                continue;
            }
            // Faces with a deformation velocity: from the moving samples (each face once).
            transfers.clear();
            for &local in &plate.sheet.active {
                if plate.crust[local as usize].is_none() {
                    continue;
                }
                let u = plate.sheet.velocity[local as usize];
                for slot in geometry.edges(local) {
                    let n = geometry.neighbor[slot];
                    let other = plate.sheet.velocity[n as usize];
                    if plate.crust[n as usize].is_none() || (other != DVec3::ZERO && n < local) {
                        continue;
                    }
                    let w = 0.5 * (u + other).dot(geometry.direction[slot]);
                    if w == 0.0 {
                        continue;
                    }
                    let share = w.abs() * geometry.face[slot] * dt / geometry.area;
                    let (from, to) = if w > 0.0 { (local, n) } else { (n, local) };
                    outflow[from as usize] += share;
                    transfers.push((from, to, share));
                }
            }
            // Sub-cycles keep each sample's outflow per cycle below half its column.
            let mut largest = 0.0f64;
            for &(from, _, _) in &transfers {
                largest = largest.max(outflow[from as usize]);
            }
            for &(from, _, _) in &transfers {
                outflow[from as usize] = 0.0;
            }
            let cycles = (largest / 0.5).ceil().max(1.0) as usize;
            let scale = 1.0 / cycles as f64;
            for _ in 0..cycles {
                // Room below the cap: receivers take at most what fits.
                for &(from, to, share) in &transfers {
                    if let Some(crust) = plate.crust[from as usize] {
                        incoming[to as usize] += crust.thickness * share * scale;
                    }
                }
                parts.clear();
                for &(from, to, share) in &transfers {
                    let thickness = plate.crust[to as usize].map_or(0.0, |c| c.thickness);
                    let room = (max_thickness - thickness).max(0.0);
                    let wanted = incoming[to as usize];
                    let factor = if wanted > room { room / wanted } else { 1.0 };
                    let s = share * scale * factor;
                    outflow[from as usize] += s;
                    parts.push((from, to, s));
                }
                touched.clear();
                for &(from, to, s) in &parts {
                    for v in [from, to] {
                        if mixtures[v as usize].is_none() {
                            let crust = plate.crust[v as usize].expect("transfers between samples");
                            let mut mixture = Mixture::new();
                            mixture.add(&crust, 1.0 - outflow[v as usize], time);
                            mixtures[v as usize] = Some(mixture);
                            touched.push(v);
                        }
                    }
                    let crust = plate.crust[from as usize].expect("transfers between samples");
                    moved += crust.mass() * s;
                    mixtures[to as usize]
                        .as_mut()
                        .expect("set above")
                        .add(&crust, s, time);
                }
                touched.sort_unstable();
                for &v in &touched {
                    let v = v as usize;
                    let old = plate.crust[v].expect("touched samples have crust");
                    let mut new = mixtures[v].take().expect("set above").crust(time);
                    // Strain from the divergence of the velocity (not from the thickness
                    // change, which advection of a thickness gradient also causes).
                    let strain = plate.sheet.dilatation[v] as f64 * dt * scale * SECONDS_PER_MYR;
                    if old.is_continental_origin() && new.thickness > 0.0 && strain != 0.0 {
                        if strain > 0.0 {
                            // Stretching heats the lithosphere (McKenzie's β).
                            let beta = 1.0 + strain;
                            let fraction = 1.0 - (1.0 - thermal_fraction(new.age(time))) / beta;
                            if let Some(age) = age_for_thermal_fraction(fraction) {
                                new.birth = new.birth.max(time - age);
                            }
                        } else if new.is_continental_origin() {
                            // Shortening deforms it: orogenic in proportion to the strain.
                            let shortening = -strain;
                            new.orogenic = (new.orogenic + shortening).min(1.0);
                            if shortening > 1e-3 {
                                new.orogeny = time;
                            }
                        }
                    }
                    change += new.mass() - old.mass();
                    plate.crust[v] = Some(new);
                    outflow[v] = 0.0;
                    incoming[v] = 0.0;
                }
            }
        }
        if self.params.sheet.fronts > 0 {
            change += self.move_fronts(dt);
        }
        self.sheet_statistics.moved += moved;
        self.ledger.record(Term::Deformation, change);
        self.sheet_statistics.transport_seconds += start.elapsed().as_secs_f64();
    }

    /// Moves the material fronts of the plates' edge samples by the normal deformation velocity
    /// (see [`PlateSheet::front`]): a front that retreated past the middle of its cell gives the
    /// cell up (the column moves to its neighbours in the plate), one that advanced past the
    /// middle of the next cell takes it (the column is shared with the new sample).
    /// Returns the change of mass (zero up to rounding).
    fn move_fronts(&mut self, dt: f64) -> f64 {
        let fronts = self.params.sheet.fronts;
        let geometry = &self.sheet_geometry;
        let max_thickness = self.params.max_crust_thickness;
        let time = self.time;
        let mut change = 0.0;
        let mut vacated: Vec<(u32, u32, u32)> = Vec::new();
        let mut grown: Vec<(u32, u32)> = Vec::new();
        for (p, plate) in self.plates.iter_mut().enumerate() {
            if !plate.sheet.moving {
                continue;
            }
            for index in 0..plate.sheet.active.len() {
                let local = plate.sheet.active[index];
                let l = local as usize;
                if plate.crust[l].is_none() {
                    continue;
                }
                let u = plate.sheet.velocity[l];
                let mut external = false;
                let mut advance = 0.0;
                let mut outward: Option<(u32, f64)> = None;
                for slot in geometry.edges(local) {
                    let n = geometry.neighbor[slot];
                    if plate.crust[n as usize].is_some() {
                        continue;
                    }
                    external = true;
                    let w = u.dot(geometry.direction[slot]);
                    advance += w * geometry.face[slot] * dt / geometry.area;
                    if w > 0.0 && outward.is_none_or(|(_, best)| w > best) {
                        outward = Some((n, w));
                    }
                }
                if !external {
                    plate.sheet.front[l] = 1.0;
                    continue;
                }
                let front = plate.sheet.front[l] as f64 + advance;
                if front < 0.5 {
                    // Give the cell up: the column goes to the neighbours it moves towards.
                    let mut weights: Vec<(u32, f64)> = Vec::new();
                    for slot in geometry.edges(local) {
                        let n = geometry.neighbor[slot];
                        if plate.crust[n as usize].is_some() {
                            weights.push((n, u.dot(geometry.direction[slot]).max(0.0)));
                        }
                    }
                    if weights.is_empty() {
                        plate.sheet.front[l] = front.max(0.0) as f32;
                        continue;
                    }
                    let total: f64 = weights.iter().map(|w| w.1).sum();
                    let count = weights.len() as f64;
                    // Only into room below the maximum thickness: a front that cannot hand its
                    // column on stays (shortening against crust at the cap stops).
                    let column = plate.crust[l].expect("checked").thickness;
                    let fits = weights.iter().all(|&(n, w)| {
                        let share = if total > 0.0 { w / total } else { 1.0 / count };
                        plate.crust[n as usize].expect("checked").thickness + share * column
                            <= max_thickness
                    });
                    if !fits {
                        plate.sheet.front[l] = 0.5;
                        continue;
                    }
                    let crust = plate.crust[l].take().expect("checked");
                    change -= crust.mass();
                    let mut best = (weights[0].0, -1.0);
                    for &(n, w) in &weights {
                        let share = if total > 0.0 { w / total } else { 1.0 / count };
                        if share > best.1 {
                            best = (n, share);
                        }
                        let receiver = plate.crust[n as usize].expect("checked");
                        let mut mixture = Mixture::new();
                        mixture.add(&receiver, 1.0, time);
                        mixture.add(&crust, share, time);
                        let mixed = mixture.crust(time);
                        change += mixed.mass() - receiver.mass();
                        plate.crust[n as usize] = Some(mixed);
                    }
                    plate.sheet.reset(l);
                    vacated.push((p as u32, local, best.0));
                } else if front > 1.5 {
                    let Some((n, _)) = outward.filter(|_| fronts > 1) else {
                        plate.sheet.front[l] = 1.5;
                        continue;
                    };
                    let crust = plate.crust[l].expect("checked");
                    let share = (front - 1.0) / front;
                    let (outer, inner) = (scaled(&crust, share), scaled(&crust, 1.0 - share));
                    change += outer.mass() + inner.mass() - crust.mass();
                    plate.crust[n as usize] = Some(outer);
                    plate.crust[l] = Some(inner);
                    plate.consumed[n as usize] = false;
                    plate.sheet.reset(n as usize);
                    plate.sheet.velocity[n as usize] = u;
                    plate.sheet.front[l] = 1.0;
                    grown.push((p as u32, n));
                } else {
                    plate.sheet.front[l] = front as f32;
                }
            }
        }
        // Cells that showed a vacated sample show the neighbour that took most of it until the
        // next ownership resolution.
        if !vacated.is_empty() {
            let mut target: HashMap<(u32, u32), u32> = HashMap::default();
            for &(p, local, receiver) in &vacated {
                target.insert((p, local), receiver);
            }
            for owner in &mut self.owners {
                // A receiver may itself have been vacated later in the pass.
                while let Some(&receiver) = target.get(&(owner.plate, owner.local)) {
                    owner.local = receiver;
                }
            }
        }
        for &(p, local) in &grown {
            let plate = &mut self.plates[p as usize];
            let near = self.mesh.nearest(plate.rotation * self.mesh.vertex(local));
            matching::track_sample(plate, local, near);
        }
        if !vacated.is_empty() || !grown.is_empty() {
            tracing::trace!(
                time = self.time,
                vacated = vacated.len(),
                grown = grown.len(),
                "Plate fronts moved"
            );
        }
        self.sheet_statistics.vacated += vacated.len() as u64;
        self.sheet_statistics.grown += grown.len() as u64;
        change
    }

    /// Splits the plates whose deformation has separated them into parts that move relative to
    /// each other (see [`PlateSheet::pending_split`]), if both parts are large enough.
    pub(super) fn split_deforming_plates(&mut self) {
        let min_cells = self.params.events.min_plate_area * self.mesh.len() as f64;
        for plate in 0..self.plates.len() as u32 {
            let Some(side) = self.plates[plate as usize].sheet.pending_split.take() else {
                continue;
            };
            if self.plates.len() >= self.params.events.max_plates {
                continue;
            }
            let mut moving = vec![false; self.mesh.len()];
            for &local in &side {
                moving[local as usize] = true;
            }
            let piece: Vec<u32> = (0..self.mesh.len() as u32)
                .filter(|&cell| {
                    let owner = self.owners[cell as usize];
                    owner.plate == plate && moving[owner.local as usize]
                })
                .collect();
            let total = self
                .owners
                .iter()
                .filter(|owner| owner.plate == plate)
                .count();
            if (piece.len() as f64) < min_cells || ((total - piece.len()) as f64) < min_cells {
                continue;
            }
            if let Some(new_plate) = self.split_plate_cells(plate, &piece) {
                self.plates[plate as usize].sheet.split_timer = 0.0;
                self.plates[new_plate as usize].sheet.split_timer = 0.0;
                self.sheet_statistics.splits += 1;
                tracing::debug!(
                    plate,
                    new_plate,
                    cells = piece.len(),
                    time = self.time,
                    "Deforming plate split"
                );
            }
        }
    }

    /// Velocity (rad/Myr, world frame, at the boundary cell) of the material of a boundary
    /// cell's plate relative to that of the other plate across the boundary: the plates'
    /// rotations and their deformation on either side.
    pub(super) fn material_relative_velocity(&self, boundary: &dynamics::BoundaryCell) -> DVec3 {
        let r = self.mesh.vertex(boundary.cell);
        (self.plates[boundary.plate as usize].angular_velocity
            - self.plates[boundary.other as usize].angular_velocity)
            .cross(r)
            + self.deformation_velocity(boundary.cell)
            - self.deformation_velocity(boundary.neighbor)
    }

    /// The deformation velocity (rad/Myr, world frame) of the lithosphere at a world cell.
    pub fn deformation_velocity(&self, cell: u32) -> DVec3 {
        let owner = self.owners[cell as usize];
        let plate = &self.plates[owner.plate as usize];
        plate.rotation * plate.sheet.velocity[owner.local as usize]
    }

    /// The deformation velocity (rad/Myr, world frame) at a world cell relative to the plate's
    /// drag-weighted mean rotation, as the torque balance solves it (before the frame follows
    /// the plate's largest block, see [`PlateSheet::frame`]).
    pub(super) fn deformation_velocity_absolute(&self, cell: u32) -> DVec3 {
        let owner = self.owners[cell as usize];
        let plate = &self.plates[owner.plate as usize];
        let r = self.mesh.vertex(owner.local);
        let frame = plate.sheet.frame.cross(r) * (SECONDS_PER_MYR / self.params.planet_radius);
        plate.rotation * (plate.sheet.velocity[owner.local as usize] + frame)
    }

    /// The strain rate (second invariant, 1/s) and the rate of area change (1/s, positive when
    /// stretching) of the lithosphere at a world cell.
    pub fn strain_rate(&self, cell: u32) -> (f64, f64) {
        let owner = self.owners[cell as usize];
        let sheet = &self.plates[owner.plate as usize].sheet;
        (
            sheet.strain_rate[owner.local as usize] as f64,
            sheet.dilatation[owner.local as usize] as f64,
        )
    }

    /// How the convergence at continental collisions is taken up (see
    /// [`SheetStatistics::collision_convergence`]).
    pub(super) fn record_partitioning(&mut self, boundaries: &[dynamics::BoundaryCell]) {
        let (mut rigid, mut material, mut count) = (0.0, 0.0, 0usize);
        for boundary in boundaries {
            if boundary.kind != BoundaryKind::Collision {
                continue;
            }
            let r = self.mesh.vertex(boundary.cell);
            let plates = (self.plates[boundary.plate as usize].angular_velocity
                - self.plates[boundary.other as usize].angular_velocity)
                .cross(r)
                .dot(boundary.normal);
            if plates <= 0.0 {
                continue;
            }
            rigid += plates;
            material += boundary.convergence;
            count += 1;
        }
        let to_cm = self.params.planet_radius * 100.0 / 1e6;
        let n = count.max(1) as f64;
        self.sheet_statistics.collision_convergence = (rigid / n * to_cm, material / n * to_cm);
    }

    /// Statistics of the deformation.
    pub fn sheet_statistics(&self) -> &SheetStatistics {
        &self.sheet_statistics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deforming_plates_stay_consistent() {
        // Runs the solve (with its symmetry check in debug builds) and the transport.
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), 3);
        let mass = simulation.total_mass();
        for _ in 0..15 {
            simulation.step(1.0);
        }
        let ledger = &simulation.ledger;
        assert!(
            ledger.deformation.abs() < 1e-9 * mass,
            "{}",
            ledger.deformation
        );
        assert!(simulation.sheet_statistics.solves > 0);
    }

    /// The crust, owners and rotations of a simulation, bit for bit.
    fn fingerprint(simulation: &Simulation) -> Vec<u64> {
        let mut bits = Vec::new();
        for owner in &simulation.owners {
            bits.extend([owner.plate as u64, owner.local as u64]);
        }
        for plate in &simulation.plates {
            bits.extend(plate.angular_velocity.to_array().map(f64::to_bits));
            for crust in plate.crust.iter().flatten() {
                bits.extend(
                    [
                        crust.thickness,
                        crust.density,
                        crust.birth,
                        crust.sediment,
                        crust.orogenic,
                    ]
                    .map(f64::to_bits),
                );
            }
        }
        bits
    }

    #[test]
    fn infinitely_stiff_plates_are_rigid_plates() {
        let run = |params: TectonicParams| {
            let mut simulation = Simulation::new(SphereMesh::icosphere(4), 8, params, 5);
            for _ in 0..10 {
                simulation.step(1.0);
            }
            fingerprint(&simulation)
        };
        let mut rigid = TectonicParams::default();
        rigid.sheet.enabled = false;
        let mut stiff = TectonicParams::default();
        stiff.sheet.rigid_viscosity = 0.0;
        assert!(
            run(rigid) == run(stiff),
            "stiff deforming plates differ from rigid plates"
        );
    }

    /// A simulation whose plate 0 has the deformation velocity `field(r)` (rad/Myr, plate
    /// frame) at every sample.
    fn with_velocity(field: impl Fn(DVec3) -> DVec3) -> Simulation {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 4, TectonicParams::default(), 2);
        let plate = &mut simulation.plates[0];
        plate.sheet.active.clear();
        for local in 0..plate.crust.len() as u32 {
            if plate.crust[local as usize].is_some() {
                let r = simulation.mesh.vertex(local);
                let u = field(r);
                let u = u - r * u.dot(r);
                plate.sheet.velocity[local as usize] = u;
                if u != DVec3::ZERO {
                    plate.sheet.active.push(local);
                }
            }
        }
        plate.sheet.moving = !plate.sheet.active.is_empty();
        simulation
    }

    #[test]
    fn transport_conserves_mass_and_thickens_converging_crust() {
        // Converging towards the centre of plate 0 (0.5 cm/yr at a radian).
        let centre = {
            let probe = with_velocity(|_| DVec3::ZERO);
            let plate = &probe.plates[0];
            let sum: DVec3 = (0..plate.crust.len() as u32)
                .filter(|&l| plate.crust[l as usize].is_some())
                .map(|l| probe.mesh.vertex(l))
                .sum();
            sum.normalize()
        };
        let speed = 0.005 / (6.371e6 * 100.0 / 1e6) * 100.0;
        let mut simulation = with_velocity(|r| (centre - r) * speed);
        let mass = ledger::plates_mass(&simulation.plates);
        let thickness = |s: &Simulation| {
            let local = s.mesh.nearest(centre);
            s.plates[0].crust[local as usize].map(|c| c.thickness)
        };
        let before = thickness(&simulation);
        for _ in 0..5 {
            simulation.transport_crust(1.0);
        }
        let after = ledger::plates_mass(&simulation.plates);
        assert!((after - mass).abs() < 1e-9 * mass, "{mass} → {after}");
        assert!(simulation.ledger.deformation.abs() < 1e-9 * mass);
        if let (Some(before), Some(after)) = (before, thickness(&simulation)) {
            assert!(
                after > before,
                "converging crust thickens: {before} → {after}"
            );
        }
    }

    #[test]
    fn advection_without_strain_leaves_the_lithosphere_unmarked() {
        // A rigid drift within the frame has no strain: crust moves, but no orogenic marking or
        // thermal reset from thickness changes of advected gradients.
        let mut simulation = with_velocity(|r| DVec3::Z.cross(r) * 0.01);
        let marks = |s: &Simulation| {
            s.plates[0]
                .crust
                .iter()
                .flatten()
                .map(|c| (c.orogenic.to_bits(), c.birth.max(-1e9).to_bits()))
                .filter(|&(o, _)| o != 0.0f64.to_bits())
                .count()
        };
        let before = marks(&simulation);
        simulation.params.sheet.fronts = 0;
        for _ in 0..10 {
            simulation.transport_crust(1.0);
        }
        assert!(
            marks(&simulation) <= before,
            "advection marked crust as orogenic"
        );
    }

    #[test]
    fn moving_fronts_conserve_mass() {
        // A uniform drift of plate 0 within its frame: its front advances on one side and
        // retreats on the other.
        let mut simulation = with_velocity(|r| DVec3::Z.cross(r) * 0.05);
        let mass = ledger::plates_mass(&simulation.plates);
        for _ in 0..20 {
            simulation.transport_crust(1.0);
        }
        let statistics = &simulation.sheet_statistics;
        assert!(statistics.vacated > 0 && statistics.grown > 0);
        let after = ledger::plates_mass(&simulation.plates);
        assert!((after - mass).abs() < 1e-9 * mass, "{mass} → {after}");
        // Every owner points to a sample with crust.
        for cell in 0..simulation.mesh.len() as u32 {
            let owner = simulation.owners[cell as usize];
            assert!(simulation.plates[owner.plate as usize].crust[owner.local as usize].is_some());
        }
    }

    #[test]
    fn geometry_is_consistent() {
        let mesh = SphereMesh::icosphere(3);
        let geometry = SheetGeometry::new(&mesh);
        for vertex in 0..mesh.len() as u32 {
            // The dual faces of a cell close: Σ n L ≈ 0 in its tangent plane.
            let r = mesh.vertex(vertex);
            let mut sum = DVec3::ZERO;
            let mut perimeter = 0.0;
            for slot in geometry.edges(vertex) {
                assert!(geometry.face[slot] > 0.0);
                let e = geometry.direction[slot];
                sum += (e - r * e.dot(r)).normalize() * geometry.face[slot];
                perimeter += geometry.face[slot];
            }
            assert!(sum.length() < 0.02 * perimeter, "{vertex}: {sum}");
        }
    }
}
