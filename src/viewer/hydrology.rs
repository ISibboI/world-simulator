//! Drainage basins, endorheic areas and lakes of a displayed state, rebuilt from its cells
//! (#102): every frame keeps, per cell, the neighbour its water flows to, whether it is ocean,
//! in a closed depression or under a lake ([`CellInfo::drainage`]), so live and past frames
//! give the same basins.
//!
//! The basin of a land cell is the cell where its water's path ends: a river mouth (the last
//! land cell before the ocean) or the exit of an endorheic lake, whose water does not leave
//! (the erosion step cuts its receiver), or, on a planet without ocean, the lowest cell. Ocean
//! cells belong to no basin.

use super::CellInfo;
use super::cell_info::{WaterBody, format_position, latitude_longitude};
use crate::sphere::SphereMesh;

/// Marks cells without a basin (the ocean).
pub const NO_BASIN: u32 = u32::MAX;

/// The basins and lakes of a state.
#[derive(Debug, Clone, Default)]
pub struct Drainage {
    /// The basin of every cell: the cell where its water's path ends, or [`NO_BASIN`].
    pub basin: Vec<u32>,
    /// Land cells per basin, by the basin's end cell (zero for other cells).
    pub size: Vec<u32>,
    /// Whether the basin ending at a cell drains to the ocean (by the end cell).
    pub to_ocean: Vec<bool>,
    /// Whether a cell lies in a lake (or dry depression) whose water does not leave it: the
    /// receivers inside the depression lead to an exit without outflow.
    pub closed: Vec<bool>,
    /// Land cells.
    pub land: u32,
}

/// The cell a cell's water flows to, if any.
pub fn receiver(mesh: &SphereMesh, cells: &[CellInfo], cell: u32) -> Option<u32> {
    let slot = cells.get(cell as usize)?.receiver_slot()?;
    mesh.neighbors(cell).get(slot).copied()
}

/// How the walk along the receivers from a cell ends (see [`resolve`]).
enum Step {
    /// The walk continues at this cell.
    Next(u32),
    /// The walk ends here with this value.
    End(u32),
}

/// A value for every cell found by following the receivers: `step` says whether a cell's
/// value is its own ([`Step::End`]) or that of the next cell. Each cell is resolved once
/// (iteratively, memoised); a cycle, which the drainage never makes, ends at the cell where it
/// closes.
fn resolve(count: usize, step: impl Fn(u32) -> Step) -> Vec<u32> {
    const UNKNOWN: u32 = u32::MAX;
    const ON_PATH: u32 = u32::MAX - 1;
    let mut value = vec![UNKNOWN; count];
    let mut path = Vec::new();
    for start in 0..count as u32 {
        if value[start as usize] != UNKNOWN {
            continue;
        }
        let mut cell = start;
        let result = loop {
            match value[cell as usize] {
                UNKNOWN => {}
                ON_PATH => break cell,
                known => break known,
            }
            match step(cell) {
                Step::End(end) => {
                    value[cell as usize] = end;
                    break end;
                }
                Step::Next(next) => {
                    value[cell as usize] = ON_PATH;
                    path.push(cell);
                    cell = next;
                }
            }
        };
        for cell in path.drain(..) {
            value[cell as usize] = result;
        }
    }
    value
}

impl Drainage {
    /// The basins and closed lakes of the `cells` on `mesh` (empty if they do not fit).
    pub fn of(mesh: &SphereMesh, cells: &[CellInfo]) -> Self {
        let count = cells.len();
        if count != mesh.len() {
            return Self::default();
        }
        let next = |cell: u32| receiver(mesh, cells, cell);
        let is_ocean = |cell: u32| cells[cell as usize].is_ocean();
        let basin = resolve(count, |cell| {
            if is_ocean(cell) {
                return Step::End(NO_BASIN);
            }
            match next(cell) {
                Some(receiver) if !is_ocean(receiver) => Step::Next(receiver),
                // A river mouth, or the end of an endorheic basin.
                _ => Step::End(cell),
            }
        });
        let mut size = vec![0; count];
        let mut to_ocean = vec![false; count];
        let mut land = 0;
        for (cell, &end) in basin.iter().enumerate() {
            if end != NO_BASIN {
                size[end as usize] += 1;
                land += 1;
            }
            if end == cell as u32 {
                to_ocean[cell] = next(end).is_some_and(is_ocean);
            }
        }
        // The exit of every depression: follow the receivers while they stay in it.
        let in_depression = |cell: u32| cells[cell as usize].in_depression();
        let exit = resolve(count, |cell| match next(cell) {
            Some(receiver) if in_depression(cell) && in_depression(receiver) => {
                Step::Next(receiver)
            }
            _ => Step::End(cell),
        });
        let closed = (0..count)
            .map(|cell| in_depression(cell as u32) && next(exit[cell]).is_none())
            .collect();
        Self {
            basin,
            size,
            to_ocean,
            closed,
            land,
        }
    }

    /// The basin of a cell: its end cell, if the cell is land.
    pub fn basin_of(&self, cell: u32) -> Option<u32> {
        self.basin
            .get(cell as usize)
            .copied()
            .filter(|&basin| basin != NO_BASIN)
    }

    /// Whether the basin ending at `end` is endorheic.
    pub fn is_endorheic(&self, end: u32) -> bool {
        !self.to_ocean.get(end as usize).copied().unwrap_or(false)
    }

    /// Land cells of the basin ending at `end`.
    pub fn size_of(&self, end: u32) -> u32 {
        self.size.get(end as usize).copied().unwrap_or(0)
    }

    /// Whether a cell is in a closed lake or dry depression (no outflow).
    pub fn is_closed(&self, cell: u32) -> bool {
        self.closed.get(cell as usize).copied().unwrap_or(false)
    }

    /// Statistics of the basins and lakes.
    pub fn summary(&self, cells: &[CellInfo]) -> Summary {
        let mut summary = Summary {
            land: self.land,
            ..Summary::default()
        };
        for (end, &size) in self.size.iter().enumerate() {
            if size == 0 {
                continue;
            }
            if self.to_ocean[end] {
                summary.ocean_basins += 1;
                if size >= min_basin(cells.len()) {
                    summary.large_ocean_basins += 1;
                }
            } else {
                summary.endorheic_basins += 1;
                summary.endorheic += size;
            }
            if size > summary.largest {
                summary.largest = size;
                summary.largest_endorheic = !self.to_ocean[end];
            }
        }
        for (cell, info) in cells.iter().enumerate() {
            match info.water_body() {
                WaterBody::Lake(_) => {
                    summary.lake += 1;
                    if self.closed.get(cell).copied().unwrap_or(false) {
                        summary.closed_lake += 1;
                    }
                }
                WaterBody::DryDepression => summary.dry_depression += 1,
                WaterBody::Ocean | WaterBody::Land => {}
            }
        }
        summary
    }

    /// The values of the cell widget's rows "Drainage basin" (to the ocean or endorheic, its
    /// share of the land, and for a lake whether it is closed) and "Basin outlet" (where its
    /// water leaves it: the river mouth or the exit of its endorheic lake), or `None` for the
    /// ocean.
    pub fn describe(
        &self,
        mesh: &SphereMesh,
        cells: &[CellInfo],
        cell: u32,
    ) -> Option<(String, String)> {
        let end = self.basin_of(cell)?;
        let share = 100.0 * f64::from(self.size_of(end)) / f64::from(self.land.max(1));
        let mut basin = format!(
            "{}, {}",
            if self.is_endorheic(end) {
                "endorheic"
            } else {
                "to the ocean"
            },
            format_share(share)
        );
        let lake = cells
            .get(cell as usize)
            .is_some_and(|info| matches!(info.water_body(), WaterBody::Lake(_)));
        if lake {
            basin += if self.is_closed(cell) {
                "; closed lake"
            } else {
                "; lake spills"
            };
        }
        let (latitude, longitude) = latitude_longitude(mesh.vertex(end));
        let outlet = format!("{}, cell {end}", format_position(latitude, longitude));
        Some((basin, outlet))
    }
}

/// "12% of land", "0.3% of land", "< 0.1% of land".
fn format_share(percent: f64) -> String {
    if percent < 0.1 {
        "< 0.1% of land".to_owned()
    } else if percent < 10.0 {
        format!("{percent:.1}% of land")
    } else {
        format!("{percent:.0}% of land")
    }
}

/// Basins draining to the ocean smaller than this share of the surface (~2.5 × 10⁵ km², 20 cells of
/// the default grid) are "small coastal basins": one colour and no outlines, so that the
/// coasts do not turn into confetti.
pub const MIN_BASIN_SHARE: f64 = 5e-4;

/// The smallest basin (cells) that is not a small coastal basin on a mesh of `cells` cells
/// (see [`MIN_BASIN_SHARE`]), at least 2.
pub fn min_basin(cells: usize) -> u32 {
    ((MIN_BASIN_SHARE * cells as f64).round() as u32).max(2)
}

/// Statistics of the basins and lakes of a state (cells).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub land: u32,
    /// Basins draining to the ocean, and those of them with at least [`min_basin`] cells.
    pub ocean_basins: u32,
    pub large_ocean_basins: u32,
    /// Endorheic basins and their land.
    pub endorheic_basins: u32,
    pub endorheic: u32,
    /// The largest basin, and whether it is endorheic.
    pub largest: u32,
    pub largest_endorheic: bool,
    /// Cells under lakes (at least 1 m of water), of them in closed lakes, and dry depressions.
    pub lake: u32,
    pub closed_lake: u32,
    pub dry_depression: u32,
}

impl Summary {
    /// A share of the land in percent.
    pub fn percent(&self, cells: u32) -> f64 {
        100.0 * f64::from(cells) / f64::from(self.land.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cells of a small mesh, all land without receivers.
    fn land(mesh: &SphereMesh) -> Vec<CellInfo> {
        vec![
            CellInfo {
                drainage: CellInfo::NO_RECEIVER,
                ..bytemuck::Zeroable::zeroed()
            };
            mesh.len()
        ]
    }

    /// Lets `from` drain to its neighbour `to`.
    fn flow(mesh: &SphereMesh, cells: &mut [CellInfo], from: u32, to: u32) {
        let slot = mesh
            .neighbors(from)
            .iter()
            .position(|&n| n == to)
            .expect("neighbours");
        let info = &mut cells[from as usize];
        info.drainage = (info.drainage & !CellInfo::RECEIVER_MASK) | slot as u32;
    }

    /// A path of neighbouring cells from `start`, `length` long, each cell new.
    fn path(mesh: &SphereMesh, start: u32, length: usize) -> Vec<u32> {
        let mut path = vec![start];
        while path.len() < length {
            let last = *path.last().unwrap();
            let next = mesh
                .neighbors(last)
                .iter()
                .copied()
                .find(|n| {
                    !path.contains(n)
                        && path
                            .iter()
                            .all(|&p| p == last || !mesh.neighbors(p).contains(n))
                })
                .expect("room for the path");
            path.push(next);
        }
        path
    }

    #[test]
    fn basins_end_at_mouths_and_endorheic_exits() {
        let mesh = SphereMesh::icosphere(3);
        let mut cells = land(&mesh);
        // A river: cells 0..4 of a path flow down it into an ocean cell at its end.
        let river = path(&mesh, 0, 5);
        for pair in river.windows(2) {
            flow(&mesh, &mut cells, pair[0], pair[1]);
        }
        cells[river[4] as usize].drainage = CellInfo::NO_RECEIVER | CellInfo::OCEAN;
        let drainage = Drainage::of(&mesh, &cells);
        let mouth = river[3];
        for &cell in &river[..4] {
            assert_eq!(drainage.basin_of(cell), Some(mouth));
        }
        assert_eq!(drainage.basin_of(river[4]), None);
        assert!(!drainage.is_endorheic(mouth));
        assert_eq!(drainage.size_of(mouth), 4);
        // Every other land cell has no receiver: an endorheic basin of its own.
        let other = (0..mesh.len() as u32)
            .find(|cell| !river.contains(cell))
            .unwrap();
        assert_eq!(drainage.basin_of(other), Some(other));
        assert!(drainage.is_endorheic(other));
        assert_eq!(drainage.land as usize, mesh.len() - 1);
        let summary = drainage.summary(&cells);
        assert_eq!(summary.ocean_basins, 1);
        assert_eq!(summary.endorheic as usize, mesh.len() - 5);
        assert_eq!(summary.largest, 4);
    }

    #[test]
    fn lakes_in_a_chain_spill_or_close() {
        let mesh = SphereMesh::icosphere(3);
        let mut cells = land(&mesh);
        // Lake A (2 cells) spills over a pass into lake B (2 cells), which has no outflow.
        let chain = path(&mesh, 7, 5);
        let [a0, a1, pass, b0, b1] = chain[..] else {
            unreachable!()
        };
        for (from, to) in [(a0, a1), (a1, pass), (pass, b0), (b0, b1)] {
            flow(&mesh, &mut cells, from, to);
        }
        for cell in [a0, a1, b0, b1] {
            let info = &mut cells[cell as usize];
            info.drainage |= CellInfo::DEPRESSION | CellInfo::LAKE;
            info.lake_depth = 20.0;
        }
        let drainage = Drainage::of(&mesh, &cells);
        assert!(!drainage.is_closed(a0) && !drainage.is_closed(a1));
        assert!(drainage.is_closed(b0) && drainage.is_closed(b1));
        assert!(!drainage.is_closed(pass));
        // All of it ends in lake B's exit: one endorheic basin.
        for cell in [a0, a1, pass, b0, b1] {
            assert_eq!(drainage.basin_of(cell), Some(b1));
        }
        assert!(drainage.is_endorheic(b1));
        let summary = drainage.summary(&cells);
        assert_eq!((summary.lake, summary.closed_lake), (4, 2));
        let (text, outlet) = drainage.describe(&mesh, &cells, a0).unwrap();
        assert!(text.starts_with("endorheic"), "{text}");
        assert!(text.ends_with("lake spills"), "{text}");
        assert!(outlet.ends_with(&format!("cell {b1}")), "{outlet}");
        assert!(
            drainage
                .describe(&mesh, &cells, b0)
                .unwrap()
                .0
                .ends_with("closed lake")
        );
        assert!(drainage.describe(&mesh, &cells, pass).is_some());
    }

    #[test]
    fn a_planet_without_ocean_drains_to_its_ends() {
        let mesh = SphereMesh::icosphere(2);
        let mut cells = land(&mesh);
        // Everything flows to cell 0 along the shortest way, which has no receiver.
        let centre = mesh.vertex(0);
        for cell in 1..mesh.len() as u32 {
            let to = *mesh
                .neighbors(cell)
                .iter()
                .min_by(|&&a, &&b| {
                    mesh.vertex(a)
                        .distance(centre)
                        .total_cmp(&mesh.vertex(b).distance(centre))
                })
                .unwrap();
            flow(&mesh, &mut cells, cell, to);
        }
        let drainage = Drainage::of(&mesh, &cells);
        assert!((0..mesh.len() as u32).all(|cell| drainage.basin_of(cell) == Some(0)));
        assert!(drainage.is_endorheic(0));
        assert_eq!(drainage.size_of(0) as usize, mesh.len());
        // A cycle (which the drainage never makes) ends without hanging.
        flow(&mesh, &mut cells, 0, mesh.neighbors(0)[0]);
        let cyclic = Drainage::of(&mesh, &cells);
        assert!((0..mesh.len() as u32).all(|cell| cyclic.basin_of(cell).is_some()));
        // Cells that do not fit the mesh give nothing.
        assert!(Drainage::of(&mesh, &cells[1..]).basin.is_empty());
    }

    #[test]
    fn a_simulated_planet_has_consistent_basins() {
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let mut planet = crate::build_planet(&cli).unwrap();
        planet.step(cli.dt);
        let cells = super::super::cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let drainage = Drainage::of(mesh, &cells);
        let water = planet.surface_water();
        for (cell, info) in cells.iter().enumerate() {
            let c = cell as u32;
            // The receivers survive the slot coding.
            assert_eq!(receiver(mesh, &cells, c), water.receiver[cell]);
            assert_eq!(info.is_ocean(), water.ocean[cell]);
            assert_eq!(drainage.basin_of(c).is_none(), info.is_ocean());
            if let Some(end) = drainage.basin_of(c) {
                assert_eq!(drainage.basin_of(end), Some(end));
                // A river's discharge grows downstream (but lakes evaporate some at their exit).
                if let Some(next) = receiver(mesh, &cells, c)
                    && !cells[next as usize].is_ocean()
                    && !cells[next as usize].in_depression()
                    && !info.in_depression()
                {
                    assert!(cells[next as usize].discharge >= info.discharge * 0.999);
                }
            }
        }
        let summary = drainage.summary(&cells);
        assert!(summary.ocean_basins > 0);
        assert_eq!(
            summary.land as usize,
            cells.iter().filter(|info| !info.is_ocean()).count()
        );
    }

    /// Prints the basins, endorheic land and lakes of a run, for `results.md` (`cargo test
    /// --release hydrology_statistics -- --ignored --nocapture`, with `HYDROLOGY_SEED`,
    /// `HYDROLOGY_SUBDIVISIONS`, `HYDROLOGY_STEPS` and `HYDROLOGY_EVERY`).
    #[test]
    #[ignore]
    fn hydrology_statistics() {
        use clap::Parser;
        let variable = |name: &str, default: u32| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(default)
        };
        let cli = crate::Cli::parse_from([
            "world-simulator".to_owned(),
            format!("--seed={}", variable("HYDROLOGY_SEED", 0)),
            format!("--subdivisions={}", variable("HYDROLOGY_SUBDIVISIONS", 6)),
        ]);
        let steps = variable("HYDROLOGY_STEPS", 300);
        let every = variable("HYDROLOGY_EVERY", 100);
        let mut planet = crate::build_planet(&cli).unwrap();
        for step in 1..=steps {
            planet.step(cli.dt);
            if !step.is_multiple_of(every) {
                continue;
            }
            let cells = super::super::cell_infos(&planet);
            let mesh = planet.tectonics().mesh();
            let drainage = Drainage::of(mesh, &cells);
            let summary = drainage.summary(&cells);
            let mut basins: Vec<(u32, u32)> = (0..cells.len() as u32)
                .filter(|&end| drainage.size_of(end) > 0)
                .map(|end| (drainage.size_of(end), end))
                .collect();
            basins.sort_unstable_by(|a, b| b.cmp(a));
            let largest: Vec<String> = basins
                .iter()
                .take(5)
                .map(|&(size, end)| {
                    format!(
                        "{:.1}%{}",
                        summary.percent(size),
                        if drainage.is_endorheic(end) { "e" } else { "" }
                    )
                })
                .collect();
            let hydrology = planet.statistics().hydrology;
            let land = hydrology.land_cells.max(1.0);
            eprintln!(
                "t {:.0} Myr: land {} cells; basins to the ocean {} (>= {} cells: {}), \
                 largest {}; endorheic {:.1}% in {} basins (simulation {:.1}%, pits {:.1}%); \
                 lakes >= 1 m {:.1}% (closed {:.1}%; simulation, fractional {:.1}%), \
                 dry depressions {:.1}%; lakes {} (endorheic {})",
                planet.tectonics().time(),
                summary.land,
                summary.ocean_basins,
                min_basin(cells.len()),
                summary.large_ocean_basins,
                largest.join(" "),
                summary.percent(summary.endorheic),
                summary.endorheic_basins,
                100.0 * hydrology.endorheic_cells / land,
                100.0 * hydrology.endorheic_pit_cells / land,
                summary.percent(summary.lake),
                summary.percent(summary.closed_lake),
                100.0 * hydrology.lake_cells / land,
                summary.percent(summary.dry_depression),
                hydrology.lakes,
                hydrology.endorheic_lakes,
            );
        }
    }
}
