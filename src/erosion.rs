//! Fluvial erosion, sediment transport and deposition.
//!
//! 1. **Drainage.** A priority flood from the ocean assigns every land cell a receiver, the cell
//!    its water drains into. Water in closed basins (lakes) spills over the lowest pass, so all
//!    rivers reach the ocean.
//! 2. **Discharge.** Precipitation is accumulated downstream along the receivers.
//! 3. **Incision.** Rivers cut into the land following the stream-power law
//!    `E = K · Q^m · S`, solved implicitly (Braun & Willett, 2013), which is stable for any time
//!    step. Rivers cannot cut below sea level, their base level. Loose sediment is removed before
//!    bedrock. Removing bedrock thins the crust, which then rises isostatically, so the surface
//!    only drops by a fraction of the removed thickness.
//! 4. **Deposition.** Sediment is carried downstream. It fills lakes, and from river mouths it
//!    is routed onto continental margins, where Earth keeps most of its sediment: the shelf on
//!    continental and transitional crust fills up to [`ErosionParams::shelf_depth`], the oceanic
//!    crust next to it (by distance from the margin crust of the same plate) only up to the
//!    depth of the continental slope and rise, each with a cap on the sediment thickness. The
//!    river's own plate fills first, other plates' margins only when it is full. What does not
//!    fit forms deep-sea fans that thin with distance from the mouth. When everything within
//!    reach is full, deltas build up to [`ErosionParams::delta_top`] slightly above sea level on
//!    the margin crust. Sediment loads the crust, so the sea floor rises by only ~0.44 of the
//!    deposited thickness (see [`crate::isostasy`]).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::isostasy::{self, SEDIMENT_ROCK_DENSITY};
use crate::sphere::SphereMesh;

/// Parameters of erosion. Lengths in metres, times in years unless noted otherwise.
#[derive(Clone, Debug)]
pub struct ErosionParams {
    /// Planet radius in metres.
    pub planet_radius: f64,
    /// Erodibility `K` of the stream-power law, in m^(1-3m) yr^(m-1).
    pub erodibility: f64,
    /// Discharge exponent `m` of the stream-power law.
    pub discharge_exponent: f64,
    /// Sediment fills flooded continental and transitional crust up to this elevation (negative,
    /// below sea level): the shelf.
    pub shelf_depth: f64,
    /// Oceanic crust within [`Self::slope_width`] of the margin crust of its plate (and always
    /// its first ring of cells) fills up to this elevation: the continental slope.
    pub slope_depth: f64,
    /// Oceanic crust within [`Self::rise_width`] of the margin crust of its plate fills up to
    /// this elevation: the continental rise.
    pub rise_depth: f64,
    pub slope_width: f64,
    pub rise_width: f64,
    /// Maximum sediment thickness on the shelf, slope and rise, and at the head of a deep-sea
    /// fan, which thins with distance from the river mouth over [`Self::fan_length`].
    pub max_shelf_sediment: f64,
    pub max_slope_sediment: f64,
    pub max_rise_sediment: f64,
    pub max_fan_sediment: f64,
    pub fan_length: f64,
    /// Erodibility of sediment relative to bedrock: sedimentary rock and loose sediment erode
    /// ~2–4× faster than crystalline rock (cosmogenic rates ~20 vs 9–11 mm/kyr; stream-power K
    /// ~4× in Neogene sediments vs metamorphic rock), so exposed sediment is recycled before new
    /// crust is eroded, as on Earth (half-mass age of sedimentary rocks ~600 Myr).
    pub sediment_erodibility_factor: f64,
    /// Diffusivity (m²/yr) of transport-limited sediment spreading on land (deltas ~4–6 × 10³
    /// m²/yr; alluvial basins 10³–10⁴).
    pub sediment_diffusivity: f64,
    /// Diffusivity of sediment below sea level relative to [`Self::sediment_diffusivity`]
    /// (marine diffusion is 10–100× slower than on land; Kaufman et al. 1991).
    pub marine_diffusivity_ratio: f64,
    /// Elevation of delta plains above sea level, built when the margin within reach is full.
    pub delta_top: f64,
    /// Whether sediment that does not fit anywhere progrades the shelf over the slope and rise
    /// within reach (filling them to the shelf depth, beyond their caps) before it piles up
    /// on the shelf.
    pub prograde: bool,
    /// Maximum number of shelf, slope and rise cells the sediment of one river mouth spreads
    /// over.
    pub max_margin_cells: usize,
    /// Maximum number of ocean cells a deep-sea fan of one river mouth spreads over.
    pub max_fan_cells: usize,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            planet_radius: 6.371e6,
            erodibility: 1.5e-7,
            discharge_exponent: 0.5,
            shelf_depth: -120.0,
            slope_depth: -2000.0,
            rise_depth: -3500.0,
            slope_width: 150e3,
            rise_width: 500e3,
            max_shelf_sediment: 10e3,
            max_slope_sediment: 8e3,
            max_rise_sediment: 5e3,
            max_fan_sediment: 1.5e3,
            fan_length: 500e3,
            sediment_erodibility_factor: 2.0,
            sediment_diffusivity: 5e3,
            marine_diffusivity_ratio: 0.02,
            delta_top: 2.0,
            prograde: false,
            max_margin_cells: 256,
            max_fan_cells: 256,
        }
    }
}

impl ErosionParams {
    /// Sets the tunable parameter `name` (the field name) to `value`.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        let field = match name {
            "erodibility" => &mut self.erodibility,
            "shelf_depth" => &mut self.shelf_depth,
            "slope_depth" => &mut self.slope_depth,
            "rise_depth" => &mut self.rise_depth,
            "slope_width" => &mut self.slope_width,
            "rise_width" => &mut self.rise_width,
            "max_shelf_sediment" => &mut self.max_shelf_sediment,
            "max_slope_sediment" => &mut self.max_slope_sediment,
            "max_rise_sediment" => &mut self.max_rise_sediment,
            "max_fan_sediment" => &mut self.max_fan_sediment,
            "fan_length" => &mut self.fan_length,
            "sediment_erodibility_factor" => &mut self.sediment_erodibility_factor,
            "sediment_diffusivity" => &mut self.sediment_diffusivity,
            "marine_diffusivity_ratio" => &mut self.marine_diffusivity_ratio,
            "prograde" => {
                self.prograde = value != 0.0;
                return Ok(());
            }
            "max_margin_cells" | "max_fan_cells" => {
                anyhow::ensure!(value >= 1.0, "{name} must be at least 1");
                let cells = value as usize;
                if name == "max_margin_cells" {
                    self.max_margin_cells = cells;
                } else {
                    self.max_fan_cells = cells;
                }
                return Ok(());
            }
            _ => anyhow::bail!("unknown erosion parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// The kind of crust below a cell, which decides how much sediment it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrustClass {
    /// Crust of continental origin, including stretched margins.
    Continental = 0,
    /// Neither continental nor normal oceanic crust: thinned margins, arcs, plateaus.
    Transitional = 1,
    /// Normal oceanic crust.
    Oceanic = 2,
}

/// Where sediment settles in the ocean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Flooded continental or transitional crust.
    Shelf = 0,
    /// Oceanic crust next to the margin crust of its plate.
    Slope = 1,
    /// Oceanic crust further out.
    Rise = 2,
    /// Oceanic crust far from any margin of its plate.
    Fan = 3,
}

/// Per-cell input of an erosion step.
pub struct Surface<'a> {
    /// Surface elevation relative to sea level (m).
    pub elevation: &'a [f64],
    /// Thickness of loose sediment (m).
    pub sediment: &'a [f64],
    /// Density of the crust below the sediment, for mass (kg/m³).
    pub bedrock_density: &'a [f64],
    /// Density giving the isostatic response of removing crust from the top (see
    /// [`crate::tectonics::Crust::removal_isostatic_density`]).
    pub bedrock_isostatic_density: &'a [f64],
    /// Precipitation (m/yr).
    pub precipitation: &'a [f64],
    /// The plate the cell belongs to.
    pub plate: &'a [u32],
    /// The kind of crust.
    pub class: &'a [CrustClass],
}

/// Changes caused by one erosion step, and diagnostic fields.
pub struct ErosionResult {
    /// Change of the crust (bedrock) thickness of each cell in metres.
    pub bedrock_delta: Vec<f64>,
    /// Change of the sediment thickness of each cell in metres.
    pub sediment_delta: Vec<f64>,
    /// Water discharge through each cell in m³/yr.
    pub discharge: Vec<f64>,
    /// Total sediment delivered to the ocean, in m³.
    pub sediment_to_ocean: f64,
    /// Sediment piled above the delta top because the ocean within reach was full (thickness
    /// over one cell, summed); it becomes land and is eroded again.
    pub sediment_overfill: f64,
    /// Sediment deposited in the ocean per [`Zone`] (thickness over one cell, summed; deltas
    /// count as shelf, overfill not at all).
    pub deposited: [f64; 4],
}

/// Performs one erosion step of `dt_years` years.
pub fn erode(
    mesh: &SphereMesh,
    surface: &Surface,
    params: &ErosionParams,
    dt_years: f64,
) -> ErosionResult {
    let Surface {
        elevation,
        sediment,
        bedrock_density,
        bedrock_isostatic_density,
        precipitation,
        plate,
        class,
    } = *surface;
    let cell_count = mesh.len();
    let cell_area = 4.0 * std::f64::consts::PI * params.planet_radius.powi(2) / cell_count as f64;
    let is_ocean = |cell: usize| elevation[cell] < 0.0;

    let Drainage {
        receiver,
        filled,
        order,
    } = drainage(mesh, elevation);

    // Discharge, accumulated from the sources downstream.
    let mut discharge = vec![0.0; cell_count];
    for cell in 0..cell_count {
        if !is_ocean(cell) {
            discharge[cell] = precipitation[cell].max(0.0) * cell_area;
        }
    }
    for &cell in order.iter().rev() {
        if let Some(receiver) = receiver[cell as usize] {
            discharge[receiver as usize] += discharge[cell as usize];
        }
    }

    // Implicit stream-power incision, from the ocean upstream so receivers are updated first.
    let mut new_elevation = elevation.to_vec();
    let mut bedrock_delta = vec![0.0; cell_count];
    let mut sediment_delta = vec![0.0; cell_count];
    let mut produced = vec![0.0; cell_count];
    for &cell in &order {
        let cell = cell as usize;
        let Some(receiver) = receiver[cell] else {
            continue;
        };
        // Rivers flowing into the sea cannot cut below sea level.
        let receiver_elevation = new_elevation[receiver as usize].max(0.0);
        if elevation[cell] <= receiver_elevation {
            continue; // Lake or flat: no incision.
        }
        let distance = mesh
            .vertex(cell as u32)
            .angle_between(mesh.vertex(receiver))
            * params.planet_radius;
        let factor = |erodibility: f64, years: f64| {
            erodibility * years * discharge[cell].powf(params.discharge_exponent) / distance
        };
        // Sediment erodes faster than bedrock. Cut through the sediment with its erodibility;
        // if the cut goes through all of it, the rest of the step cuts the bedrock below.
        let bulk = isostasy::sediment_bulk_thickness(sediment[cell]);
        let factor_sediment = factor(
            params.erodibility * params.sediment_erodibility_factor,
            dt_years,
        );
        let cut = elevation[cell]
            - (elevation[cell] + factor_sediment * receiver_elevation) / (1.0 + factor_sediment);
        let thickness = if cut <= bulk {
            cut
        } else {
            let remaining_years = dt_years * (1.0 - bulk / cut);
            let factor_bedrock = factor(params.erodibility, remaining_years);
            let exposed = elevation[cell] - bulk;
            let bedrock_cut =
                exposed - (exposed + factor_bedrock * receiver_elevation) / (1.0 + factor_bedrock);
            bulk + bedrock_cut.max(0.0)
        };

        // The top `thickness` metres are removed: sediment (bulk, compacted) first.
        let from_bulk = thickness.min(bulk);
        let left = isostasy::sediment_for_bulk_thickness(bulk - from_bulk);
        let from_sediment = sediment[cell] - left;
        let from_bedrock = thickness - from_bulk;
        sediment_delta[cell] -= from_sediment;
        bedrock_delta[cell] -= from_bedrock;
        // Eroded bedrock becomes sediment of the same rock mass.
        produced[cell] =
            from_sediment + from_bedrock * bedrock_density[cell] / SEDIMENT_ROCK_DENSITY;
        // The column rebounds isostatically, so the surface drops less than the removed
        // thickness.
        let without_sediment = isostasy::change_sediment(elevation[cell], sediment[cell], left);
        new_elevation[cell] = isostasy::add_layer(
            without_sediment,
            -from_bedrock,
            bedrock_isostatic_density[cell],
        );
    }

    let mut seabed = Seabed {
        sediment: sediment
            .iter()
            .zip(&sediment_delta)
            .map(|(s, d)| s + d)
            .collect(),
        elevation: new_elevation,
        delta: sediment_delta,
    };

    // Route sediment downstream, filling lakes on the way.
    let mut flux = produced;
    let mut delivered = vec![0.0; cell_count];
    // The last land cell of a river reaching each mouth (u32::MAX for none).
    let mut source = vec![u32::MAX; cell_count];
    for &cell in order.iter().rev() {
        let cell = cell as usize;
        if is_ocean(cell) {
            continue;
        }
        let lake_room = if filled[cell] > elevation[cell] {
            seabed.room(cell, filled[cell], f64::INFINITY)
        } else {
            0.0
        };
        let deposit = flux[cell].min(lake_room);
        if deposit > 0.0 {
            seabed.deposit(cell, deposit);
        }
        let remaining = flux[cell] - deposit;
        match receiver[cell] {
            Some(receiver) if is_ocean(receiver as usize) => {
                delivered[receiver as usize] += remaining;
                source[receiver as usize] = cell as u32;
            }
            Some(receiver) => flux[receiver as usize] += remaining,
            None => {
                // A land cell without receiver only exists on planets without ocean.
                seabed.deposit(cell, remaining);
            }
        }
    }

    let zones = zones(mesh, plate, class, params);
    let spacing = mesh.mean_edge_angle() * params.planet_radius;

    let mut sediment_to_ocean = 0.0;
    let mut sediment_overfill = 0.0;
    let mut deposited = [0.0; 4];
    let mut margin_mark = vec![usize::MAX; cell_count];
    let mut fan_mark = vec![usize::MAX; cell_count];
    let mut queue = VecDeque::new();
    let mut fan_queue = VecDeque::new();
    let mut margin = Vec::new();
    let mut fan = Vec::new();
    let mut water = Vec::new();
    let mut shelf = Vec::new();
    for (mouth, &delivered) in delivered.iter().enumerate() {
        let mut remaining = delivered;
        if remaining <= 0.0 {
            continue;
        }
        sediment_to_ocean += remaining * cell_area;
        let source_cell = source[mouth] as usize;
        let own_plate = plate[source_cell];

        // The margin within reach: shelf, slope and rise cells connected to the mouth.
        queue.clear();
        margin.clear();
        queue.push_back(mouth);
        margin_mark[mouth] = mouth;
        while let Some(cell) = queue.pop_front() {
            margin.push(cell);
            if margin.len() >= params.max_margin_cells {
                break;
            }
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                if is_ocean(neighbor)
                    && zones[neighbor] != Zone::Fan
                    && margin_mark[neighbor] != mouth
                {
                    margin_mark[neighbor] = mouth;
                    queue.push_back(neighbor);
                }
            }
        }
        // Fill the margin up to the depth of each zone, nearest cells first, so that shelves
        // prograde: the river's own plate first, other plates' margins once it is full.
        for own in [true, false] {
            for &cell in &margin {
                if remaining <= 0.0 {
                    break;
                }
                if (plate[cell] == own_plate) != own {
                    continue;
                }
                let (target, cap) = match zones[cell] {
                    // Deeply buried sediment on continental crust metamorphoses (see
                    // `Simulation::metamorphose_sediment`), so it needs no cap.
                    Zone::Shelf if class[cell] == CrustClass::Continental => {
                        (params.shelf_depth, f64::INFINITY)
                    }
                    Zone::Shelf => (params.shelf_depth, params.max_shelf_sediment),
                    Zone::Slope => (params.slope_depth, params.max_slope_sediment),
                    Zone::Rise => (params.rise_depth, params.max_rise_sediment),
                    Zone::Fan => continue,
                };
                let thickness = remaining.min(seabed.room(cell, target, cap));
                if thickness > 0.0 {
                    seabed.deposit(cell, thickness);
                    deposited[zones[cell] as usize] += thickness;
                    remaining -= thickness;
                }
            }
        }

        // What does not fit flows down the slope into a deep-sea fan: spread over the ocean
        // far from margins within reach in proportion to the room below the slope depth, with
        // a thickness cap that falls with distance from the mouth.
        if remaining > 0.0 {
            fan_queue.clear();
            fan.clear();
            fan_queue.push_back((mouth, 0u32));
            fan_mark[mouth] = mouth;
            let mut rooms = Vec::new();
            let mut reached = 1;
            // Whether the reach limit cut the search short, i.e. the mouth is not on a small
            // enclosed water body.
            let mut open = false;
            water.clear();
            while let Some((cell, hops)) = fan_queue.pop_front() {
                water.push(cell);
                if zones[cell] == Zone::Fan {
                    let cap = params.max_fan_sediment
                        * (-f64::from(hops) * spacing / params.fan_length).exp();
                    let room = seabed.room(cell, params.slope_depth, cap);
                    if room > 0.0 {
                        fan.push(cell);
                        rooms.push(room);
                    }
                }
                for &neighbor in mesh.neighbors(cell as u32) {
                    let neighbor = neighbor as usize;
                    if is_ocean(neighbor) && fan_mark[neighbor] != mouth {
                        if reached >= params.max_fan_cells {
                            open = true;
                            continue;
                        }
                        reached += 1;
                        fan_mark[neighbor] = mouth;
                        fan_queue.push_back((neighbor, hops + 1));
                    }
                }
            }
            let total_room: f64 = rooms.iter().sum();
            if total_room > 0.0 {
                let fraction = (remaining / total_room).min(1.0);
                for (&cell, &room) in fan.iter().zip(&rooms) {
                    seabed.deposit(cell, room * fraction);
                }
                deposited[Zone::Fan as usize] += total_room * fraction;
                remaining -= total_room * fraction;
            }
            // A small enclosed sea or depression fills up completely, nearest cells first.
            if !open {
                for &cell in &water {
                    if remaining <= 0.0 {
                        break;
                    }
                    let thickness =
                        remaining.min(seabed.room(cell, params.delta_top, f64::INFINITY));
                    if thickness > 0.0 {
                        seabed.deposit(cell, thickness);
                        deposited[zones[cell] as usize] += thickness;
                        remaining -= thickness;
                    }
                }
            }
        }

        // When the fan is full too, deltas build up to a delta plain slightly above sea level
        // on the margin crust. Anything beyond that piles up evenly on the shelf within reach,
        // or at the river's last land cell if there is no shelf (it becomes land, and the next
        // step's drainage carries it on); never on the slope or rise.
        for &cell in &margin {
            if remaining <= 0.0 {
                break;
            }
            if zones[cell] != Zone::Shelf {
                continue;
            }
            let thickness = remaining.min(seabed.room(cell, params.delta_top, f64::INFINITY));
            if thickness > 0.0 {
                seabed.deposit(cell, thickness);
                deposited[Zone::Shelf as usize] += thickness;
                remaining -= thickness;
            }
        }
        if params.prograde {
            for own in [true, false] {
                for &cell in &margin {
                    if remaining <= 0.0 {
                        break;
                    }
                    if (plate[cell] == own_plate) != own
                        || !matches!(zones[cell], Zone::Slope | Zone::Rise)
                    {
                        continue;
                    }
                    let thickness =
                        remaining.min(seabed.room(cell, params.shelf_depth, f64::INFINITY));
                    if thickness > 0.0 {
                        seabed.deposit(cell, thickness);
                        deposited[zones[cell] as usize] += thickness;
                        remaining -= thickness;
                    }
                }
            }
        }
        if remaining > 0.0 {
            shelf.clear();
            shelf.extend(margin.iter().copied().filter(|&c| zones[c] == Zone::Shelf));
            if shelf.is_empty() {
                shelf.push(source_cell);
            }
            let share = remaining / shelf.len() as f64;
            for &cell in &shelf {
                seabed.deposit(cell, share);
            }
            sediment_overfill += remaining;
        }
    }

    diffuse_sediment(mesh, &mut seabed, params, dt_years);

    ErosionResult {
        bedrock_delta,
        sediment_delta: seabed.delta,
        discharge,
        sediment_to_ocean,
        sediment_overfill,
        deposited,
    }
}

/// Surface elevation and sediment of every cell while sediment is deposited.
struct Seabed {
    elevation: Vec<f64>,
    /// Sediment thickness now.
    sediment: Vec<f64>,
    /// Change of the sediment thickness in this erosion step.
    delta: Vec<f64>,
}

impl Seabed {
    /// Room (m) to fill a cell up to `target` without exceeding `cap` m of sediment.
    fn room(&self, cell: usize, target: f64, cap: f64) -> f64 {
        if self.elevation[cell] < target {
            isostasy::sediment_for_rise(self.elevation[cell], target, self.sediment[cell])
                .min(cap - self.sediment[cell])
                .max(0.0)
        } else {
            0.0
        }
    }

    fn remove(&mut self, cell: usize, thickness: f64) {
        let before = self.sediment[cell];
        let after = (before - thickness).max(0.0);
        self.delta[cell] -= before - after;
        self.sediment[cell] = after;
        self.elevation[cell] = isostasy::change_sediment(self.elevation[cell], before, after);
    }

    fn deposit(&mut self, cell: usize, thickness: f64) {
        let before = self.sediment[cell];
        self.delta[cell] += thickness;
        self.sediment[cell] += thickness;
        self.elevation[cell] =
            isostasy::change_sediment(self.elevation[cell], before, self.sediment[cell]);
    }
}

/// Transport-limited spreading of sediment (Kenyon & Turcotte 1985; Kaufman et al. 1991; Paola
/// et al. 1992): sediment moves to lower neighbours with a flux proportional to the difference in
/// surface elevation (linear diffusion), as far as the cell has sediment. On land the diffusivity
/// is [`ErosionParams::sediment_diffusivity`] and land is lowered at most to sea level: delta and
/// coastal plains spread seaward and grade to sea level, they are not drowned. Below sea level
/// sediment spreads [`ErosionParams::marine_diffusivity_ratio`] times as fast (below wave base
/// only gravity flows move it).
fn diffuse_sediment(mesh: &SphereMesh, seabed: &mut Seabed, params: &ErosionParams, dt_years: f64) {
    if params.sediment_diffusivity <= 0.0 {
        return;
    }
    let spacing = mesh.mean_edge_angle() * params.planet_radius;
    // Explicit scheme: each cell may pass at most ~1/2 of its excess over ~6 neighbours.
    let rate = params.sediment_diffusivity * dt_years / (spacing * spacing);
    let substeps = (rate * 6.0 / 0.5).ceil().max(1.0) as usize;
    let rate = rate / substeps as f64;
    let cell_count = mesh.len();
    let mut outflow = vec![0.0; cell_count];
    let mut transfers: Vec<(usize, usize, f64)> = Vec::new();
    for _ in 0..substeps {
        transfers.clear();
        outflow.iter_mut().for_each(|x| *x = 0.0);
        for cell in 0..cell_count {
            let height = seabed.elevation[cell];
            if seabed.sediment[cell] <= 0.0 {
                continue;
            }
            let (rate, floor) = if height >= 0.0 {
                (rate, 0.0)
            } else {
                (rate * params.marine_diffusivity_ratio, f64::NEG_INFINITY)
            };
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                let drop = height - seabed.elevation[neighbor].max(floor);
                if drop > 0.0 {
                    let amount = rate * drop;
                    outflow[cell] += amount;
                    transfers.push((cell, neighbor, amount));
                }
            }
        }
        for &(from, to, amount) in &transfers {
            // No cell gives more sediment than it has.
            let scale = (seabed.sediment[from] / outflow[from]).min(1.0);
            let amount = amount * scale;
            if amount > 0.0 {
                seabed.remove(from, amount);
                seabed.deposit(to, amount);
            }
        }
    }
}

/// The [`Zone`] of each cell: oceanic crust by its distance from the nearest crust of its own
/// plate that is not oceanic (counted in rings of cells within the plate).
fn zones(
    mesh: &SphereMesh,
    plate: &[u32],
    class: &[CrustClass],
    params: &ErosionParams,
) -> Vec<Zone> {
    let cell_count = mesh.len();
    let spacing = mesh.mean_edge_angle() * params.planet_radius;
    let mut ring = vec![u32::MAX; cell_count];
    let mut queue = VecDeque::new();
    for cell in 0..cell_count {
        if class[cell] != CrustClass::Oceanic {
            ring[cell] = 0;
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        for &neighbor in mesh.neighbors(cell as u32) {
            let neighbor = neighbor as usize;
            if ring[neighbor] == u32::MAX && plate[neighbor] == plate[cell] {
                ring[neighbor] = ring[cell] + 1;
                queue.push_back(neighbor);
            }
        }
    }
    ring.iter()
        .map(|&ring| {
            let distance = f64::from(ring) * spacing;
            if ring == 0 {
                Zone::Shelf
            } else if ring == 1 || distance <= params.slope_width {
                Zone::Slope
            } else if distance <= params.rise_width {
                Zone::Rise
            } else {
                Zone::Fan
            }
        })
        .collect()
}

struct Drainage {
    /// The cell each cell drains into; `None` for ocean cells.
    receiver: Vec<Option<u32>>,
    /// Elevation of the water surface: the cell elevation, or the spill level of a lake.
    filled: Vec<f64>,
    /// Cells ordered such that every cell comes after its receiver.
    order: Vec<u32>,
}

/// Priority-flood drainage (Barnes et al., 2014), starting from all ocean cells.
fn drainage(mesh: &SphereMesh, elevation: &[f64]) -> Drainage {
    /// Minimal rise of lake surfaces per cell, so that flow directions inside lakes are defined.
    const EPSILON: f64 = 1e-3;

    let cell_count = mesh.len();
    let mut receiver = vec![None; cell_count];
    let mut filled = elevation.to_vec();
    let mut visited = vec![false; cell_count];
    let mut order = Vec::with_capacity(cell_count);
    let mut heap = BinaryHeap::new();

    for cell in 0..cell_count {
        if elevation[cell] < 0.0 {
            visited[cell] = true;
            heap.push(Reverse((OrderedFloat(elevation[cell]), cell as u32)));
        }
    }
    if heap.is_empty() {
        let lowest = (0..cell_count)
            .min_by(|&a, &b| elevation[a].total_cmp(&elevation[b]))
            .expect("mesh has cells");
        visited[lowest] = true;
        heap.push(Reverse((OrderedFloat(elevation[lowest]), lowest as u32)));
    }

    while let Some(Reverse((OrderedFloat(level), cell))) = heap.pop() {
        order.push(cell);
        for &neighbor in mesh.neighbors(cell) {
            let n = neighbor as usize;
            if visited[n] {
                continue;
            }
            visited[n] = true;
            receiver[n] = Some(cell);
            filled[n] = elevation[n].max(level + EPSILON);
            heap.push(Reverse((OrderedFloat(filled[n]), neighbor)));
        }
    }

    Drainage {
        receiver,
        filled,
        order,
    }
}

#[derive(Clone, Copy)]
struct OrderedFloat(f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cone-shaped island (radius 20°) with a shelf sloping to −1500 m at 30°, surrounded by
    /// a deep ocean.
    fn island(mesh: &SphereMesh) -> Vec<f64> {
        (0..mesh.len() as u32)
            .map(|cell| {
                let angle = mesh.vertex(cell).angle_between(glam::DVec3::Z).to_degrees();
                if angle <= 30.0 {
                    3000.0 - angle * 150.0
                } else {
                    -4500.0 - angle
                }
            })
            .collect()
    }

    /// Continental crust down to `depth` below sea level, oceanic crust below, all on one plate.
    fn classes(elevation: &[f64], depth: f64) -> Vec<CrustClass> {
        elevation
            .iter()
            .map(|&e| {
                if e >= depth {
                    CrustClass::Continental
                } else {
                    CrustClass::Oceanic
                }
            })
            .collect()
    }

    /// One erosion step of 1 Myr with uniform precipitation and bedrock density.
    /// One erosion step of 1 Myr with uniform precipitation and bedrock density, without
    /// sediment diffusion (routing tests).
    fn run(
        mesh: &SphereMesh,
        elevation: &[f64],
        precipitation: f64,
        plate: &[u32],
        class: &[CrustClass],
    ) -> ErosionResult {
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            ..ErosionParams::default()
        };
        run_with(
            mesh,
            elevation,
            &vec![0.0; mesh.len()],
            precipitation,
            plate,
            class,
            &params,
        )
    }

    fn run_with(
        mesh: &SphereMesh,
        elevation: &[f64],
        sediment: &[f64],
        precipitation: f64,
        plate: &[u32],
        class: &[CrustClass],
        params: &ErosionParams,
    ) -> ErosionResult {
        let n = mesh.len();
        erode(
            mesh,
            &Surface {
                elevation,
                sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                precipitation: &vec![precipitation; n],
                plate,
                class,
            },
            params,
            1e6,
        )
    }

    #[test]
    fn sediment_erodes_faster_than_bedrock() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -1000.0);
        let plate = vec![0; mesh.len()];
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            ..ErosionParams::default()
        };
        // The same island with a thick sediment cover on land, at the same elevations.
        let covered: Vec<f64> = elevation
            .iter()
            .map(|&e| if e > 0.0 { 5000.0 } else { 0.0 })
            .collect();
        let bare = run_with(
            &mesh,
            &elevation,
            &vec![0.0; mesh.len()],
            1.0,
            &plate,
            &class,
            &params,
        );
        let soft = run_with(&mesh, &elevation, &covered, 1.0, &plate, &class, &params);
        let removed = |r: &ErosionResult| -> f64 {
            (0..mesh.len())
                .filter(|&c| elevation[c] > 0.0)
                .map(|c| -(r.bedrock_delta[c] + r.sediment_delta[c]))
                .sum()
        };
        assert!(
            removed(&soft) > 1.5 * removed(&bare),
            "{} {}",
            removed(&soft),
            removed(&bare)
        );
        // The covered island loses sediment, not bedrock, where the cover is thick.
        assert!(soft.bedrock_delta.iter().all(|&d| d == 0.0));
    }

    #[test]
    fn sediment_spreads_from_coastal_plains_into_the_sea() {
        let mesh = SphereMesh::icosphere(4);
        // A flat coastal plain at +50 m with 2 km of sediment next to a shelf at −1000 m.
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                if mesh.vertex(cell).z > 0.3 {
                    50.0
                } else {
                    -1000.0
                }
            })
            .collect();
        let sediment: Vec<f64> = elevation
            .iter()
            .map(|&e| if e > 0.0 { 2000.0 } else { 0.0 })
            .collect();
        let class = classes(&elevation, -2000.0);
        let plate = vec![0; mesh.len()];
        let params = ErosionParams {
            sediment_erodibility_factor: 1.0,
            ..ErosionParams::default()
        };
        let result = run_with(&mesh, &elevation, &sediment, 0.0, &plate, &class, &params);
        let to_sea: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] < 0.0)
            .map(|c| result.sediment_delta[c])
            .sum();
        let from_land: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] > 0.0)
            .map(|c| -result.sediment_delta[c])
            .sum();
        assert!(to_sea > 0.0);
        // Conserved, and no cell gives more than it has.
        assert!((to_sea - from_land).abs() < 1e-6 * from_land);
        assert!((0..mesh.len()).all(|c| sediment[c] + result.sediment_delta[c] >= -1e-9));
        // The plain spreads seaward but is not drowned: diffusion lowers land at most to sea
        // level.
        for c in 0..mesh.len() {
            if elevation[c] > 0.0 {
                let after = isostasy::change_sediment(
                    elevation[c],
                    sediment[c],
                    sediment[c] + result.sediment_delta[c],
                );
                assert!(after >= -1e-6, "cell {c}: {after}");
            }
        }
        // Only cells next to the coast change much: diffusion over ~1 cell per Myr.
        let deep_inland = (0..mesh.len() as u32)
            .filter(|&c| mesh.vertex(c).z > 0.8)
            .map(|c| result.sediment_delta[c as usize].abs())
            .fold(0.0, f64::max);
        assert!(deep_inland < 1.0, "{deep_inland}");
    }

    #[test]
    fn all_water_reaches_the_ocean() {
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        // Dig a pit on the flank.
        let pit = mesh.nearest(glam::DVec3::new(0.05, 0.0, 1.0).normalize()) as usize;
        elevation[pit] -= 1500.0;
        assert!(
            mesh.neighbors(pit as u32)
                .iter()
                .all(|&n| elevation[n as usize] > elevation[pit])
        );
        let class = classes(&elevation, -1000.0);
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);

        let cell_area = 4.0 * std::f64::consts::PI * 6.371e6f64.powi(2) / mesh.len() as f64;
        let land_cells = elevation.iter().filter(|&&e| e >= 0.0).count() as f64;
        let into_ocean: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] < 0.0)
            .map(|c| result.discharge[c])
            .sum();
        assert!((into_ocean - land_cells * cell_area).abs() / into_ocean < 1e-9);

        // The pit collects sediment.
        assert!(result.sediment_delta[pit] > 0.0);
    }

    #[test]
    fn erosion_lowers_land_and_builds_shelves() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -1000.0);
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);

        assert!(result.bedrock_delta.iter().all(|&d| d <= 0.0));
        assert!(result.bedrock_delta.iter().any(|&d| d < 0.0));
        assert!(result.sediment_to_ocean > 0.0);
        // Sediment ends up in the ocean, building the shelf around the island.
        assert!((0..mesh.len()).any(|c| elevation[c] < 0.0 && result.sediment_delta[c] > 0.0));
        // Land is not raised except by lake deposits; this island has no lakes.
        for cell in 0..mesh.len() {
            if elevation[cell] >= 0.0 {
                assert!(result.bedrock_delta[cell] + result.sediment_delta[cell] <= 1e-9);
            }
        }
    }

    #[test]
    fn erosion_conserves_rock_mass() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        // No margin crust at sea and heavy rain: fans fill too.
        let class = classes(&elevation, 0.0);
        let result = run(&mesh, &elevation, 20.0, &vec![0; mesh.len()], &class);
        assert!(result.deposited[Zone::Fan as usize] > 0.0);
        let change: f64 = (0..mesh.len())
            .map(|c| {
                result.bedrock_delta[c] * 2800.0 + result.sediment_delta[c] * SEDIMENT_ROCK_DENSITY
            })
            .sum();
        let eroded: f64 = result.bedrock_delta.iter().map(|d| -d * 2800.0).sum();
        assert!(eroded > 0.0);
        // Nothing is lost: eroded bedrock becomes sediment at equal mass.
        assert!(
            change.abs() < 1e-9 * eroded,
            "change {change}, eroded {eroded}"
        );
    }

    #[test]
    fn margins_fill_before_oceanic_crust() {
        let mesh = SphereMesh::icosphere(4);
        // Continental crust reaches far out to sea: the shelf has plenty of room.
        let elevation = island(&mesh);
        let class = classes(&elevation, -3000.0);
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);
        assert!(result.sediment_to_ocean > 0.0);
        for cell in 0..mesh.len() {
            if class[cell] == CrustClass::Oceanic {
                assert_eq!(result.sediment_delta[cell], 0.0, "cell {cell}");
            }
        }
        assert!(result.deposited[Zone::Shelf as usize] > 0.0);
    }

    #[test]
    fn oceanic_crust_is_never_filled_to_shelf_depth() {
        let mesh = SphereMesh::icosphere(4);
        // A steep coast: deep ocean right off the island.
        let elevation: Vec<f64> = island(&mesh)
            .iter()
            .map(|&e| if e < 0.0 { -4500.0 } else { e })
            .collect();
        let class = classes(&elevation, 0.0);
        let params = ErosionParams::default();
        let result = run(&mesh, &elevation, 5.0, &vec![0; mesh.len()], &class);
        assert_eq!(result.sediment_overfill, 0.0);
        assert!(
            result.deposited[Zone::Slope as usize] + result.deposited[Zone::Rise as usize] > 0.0,
            "{:?}",
            result.deposited
        );
        for cell in 0..mesh.len() {
            if class[cell] == CrustClass::Oceanic && result.sediment_delta[cell] > 0.0 {
                let after =
                    isostasy::change_sediment(elevation[cell], 0.0, result.sediment_delta[cell]);
                assert!(after <= params.slope_depth + 1e-6, "cell {cell}: {after}");
                assert!(result.sediment_delta[cell] <= params.max_slope_sediment + 1e-6);
            }
        }
    }

    #[test]
    fn enclosed_seas_fill_up() {
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        // A small sea below sea level on the island's flank, on oceanic crust.
        let pit = mesh.nearest(glam::DVec3::new(0.15, 0.0, 1.0).normalize()) as usize;
        elevation[pit] = -1000.0;
        let mut class = classes(&elevation, -1000.0);
        class[pit] = CrustClass::Oceanic;
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);
        // The sea fills up to the delta top (beyond the slope depth of oceanic crust); what
        // does not fit stays on land, not piled on the sea.
        let after = isostasy::change_sediment(elevation[pit], 0.0, result.sediment_delta[pit]);
        let delta_top = ErosionParams::default().delta_top;
        assert!(result.sediment_overfill > 0.0);
        assert!((after - delta_top).abs() < 1e-6, "{after}");
    }

    #[test]
    fn rivers_fill_their_own_plate_first() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -3000.0);
        // The island is on plate 0; the deep shelf on its southern side (x < 0) belongs to
        // plate 1, the rest of the shelf has room enough.
        let plate: Vec<u32> = (0..mesh.len() as u32)
            .map(|cell| u32::from(elevation[cell as usize] < 0.0 && mesh.vertex(cell).x < -0.05))
            .collect();
        let result = run(&mesh, &elevation, 0.2, &plate, &class);
        assert!(result.sediment_to_ocean > 0.0);
        let on_plate = |p: u32| -> f64 {
            (0..mesh.len())
                .filter(|&c| plate[c] == p)
                .map(|c| result.sediment_delta[c].max(0.0))
                .sum()
        };
        assert!(on_plate(0) > 0.0);
        assert_eq!(on_plate(1), 0.0);
    }

    #[test]
    fn fans_thin_with_distance() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        // No margin crust at sea: everything offshore is fan, so heavy rain feeds fans.
        let class = classes(&elevation, 0.0);
        let mut plate = vec![0; mesh.len()];
        for cell in 0..mesh.len() {
            if elevation[cell] < 0.0 {
                plate[cell] = 1;
            }
        }
        let result = run(&mesh, &elevation, 5.0, &plate, &class);
        assert!(result.deposited[Zone::Fan as usize] > 0.0);
        let params = ErosionParams::default();
        // Mean deposit by angular distance from the island's centre, in bands of 5° from the
        // foot of the shelf.
        let mut bands = [(0.0, 0); 6];
        for cell in 0..mesh.len() {
            if elevation[cell] < 0.0 {
                assert!(result.sediment_delta[cell] <= params.max_fan_sediment + 1e-6);
                let angle = mesh
                    .vertex(cell as u32)
                    .angle_between(glam::DVec3::Z)
                    .to_degrees();
                let band = ((angle - 30.0) / 5.0) as usize;
                if angle >= 30.0 && band < bands.len() {
                    bands[band].0 += result.sediment_delta[cell];
                    bands[band].1 += 1;
                }
            }
        }
        let means: Vec<f64> = bands
            .iter()
            .map(|&(sum, n)| sum / n.max(1) as f64)
            .collect();
        assert!(means[0] > 0.0, "{means:?}");
        for pair in means.windows(2) {
            assert!(pair[1] <= pair[0], "{means:?}");
        }
    }
}
