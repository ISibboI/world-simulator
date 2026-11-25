//! Fill, spill and merge inside one closed depression (#168; Barnes et al. 2020, "Computing water
//! flow through complex landscapes", parts 2 and 3).
//!
//! A closed depression can hold several sub-basins separated by inner sills below its spill
//! level. Water crosses a sill only when its surface is above it: the water of a sub-basin stays
//! in it until it is full to its inner sill, then the surplus spills over the sill into the
//! neighbouring sub-basin, and when both are full they merge into one lake that rises above the
//! sill. The sub-basins form a tree (the depression hierarchy): its leaves are the local minima,
//! an inner node is made by the inner sill where its children meet, and the root is the whole
//! depression with its spill level.
//!
//! Lakes are in equilibrium in every step (filling and drying take 10²–10⁴ years): a lake rises
//! until its evaporation balances its inflow (see [`super::lake_balance`]). Whether a node is
//! full depends only on its total inflow: the surplus of full sub-basins passes to partial ones,
//! so if one stays partial, all the water of the node evaporates inside it, less than when all
//! are full. A node is therefore one lake when its inflow covers the evaporation of all its
//! sub-basins full; otherwise its full children spill their surplus into their partial siblings.

use super::{LAKE_MIN_DEPTH, LakeBalance, lake_balance, lake_water_depth};
use crate::sphere::SphereMesh;

const NONE: u32 = u32::MAX;

/// A node of the depression hierarchy.
struct Node {
    /// The node it merges into ([`NONE`] for the root).
    parent: u32,
    children: Vec<u32>,
    /// The members (indices into the depression's cells, ascending) that joined while this
    /// node was the top of its part: the minimum of a leaf and the cells above it, the inner
    /// sill of an inner node and the cells above it.
    own: Vec<u32>,
    /// Water surface at which it is full: the inner sill where it merges into its parent, the
    /// depression's spill level for the root.
    sill: f64,
    /// For inner nodes, the member that is the inner sill where the children meet.
    merge: u32,
    /// Range of the leaf positions of its subtree (see [`Basins::leaf_position`]).
    leaves: (u32, u32),
    /// The lowest member of its subtree.
    lowest: u32,
    /// For a sub-basin absorbed into a neighbour because it is shallower than
    /// [`LAKE_MIN_DEPTH`] below its inner sill: the node its water goes to ([`NONE`] otherwise).
    absorbed: u32,
}

/// A lake in a depression: the water body of one node of the hierarchy.
#[derive(Debug)]
pub(super) struct Lake {
    /// Its cells (global indices), from the lowest floor up.
    pub cells: Vec<u32>,
    /// The water surface at which it is full.
    pub sill: f64,
    /// The water balance (level, covered cells, evaporation of its own inflow, whether it does
    /// not spill).
    pub balance: LakeBalance,
    /// Sea water poured into it over the depression's spill level (m³ per cell area).
    pub sea_water: f64,
}

/// The depression hierarchy of one closed depression and its lakes.
pub(super) struct Basins {
    nodes: Vec<Node>,
    root: u32,
    /// The leaf the water of every member drains to (steepest descent inside the depression).
    leaf_of: Vec<u32>,
    /// Position of every leaf in a depth-first order of the tree.
    leaf_position: Vec<u32>,
    /// The members' lower neighbours in the depression (members joined earlier).
    lower: Vec<[u32; 6]>,
    /// The members (global cells, ascending floor).
    members: Vec<u32>,
    /// The lake of every node that is one water body (none for partial inner nodes, and for
    /// nodes inside a lake).
    lakes: Vec<Option<Lake>>,
}

impl Basins {
    /// The hierarchy of the depression with the cells `members` (ascending floor) and spill
    /// level `spill` on the floors `elevation`; `local` is a scratch map of all cells, [`NONE`]
    /// everywhere, left so. Without `hierarchy`, the depression is a single basin (the lake
    /// covers its cells from the lowest floor up, whatever the inner sills).
    pub fn new(
        mesh: &SphereMesh,
        members: &[u32],
        elevation: &[f64],
        spill: f64,
        local: &mut [u32],
        hierarchy: bool,
    ) -> Self {
        let count = members.len();
        if !hierarchy {
            let nodes = vec![Node {
                parent: NONE,
                children: Vec::new(),
                own: (0..count as u32).collect(),
                sill: spill,
                merge: NONE,
                leaves: (0, 1),
                lowest: 0,
                absorbed: NONE,
            }];
            return Self {
                nodes,
                root: 0,
                leaf_of: vec![0; count],
                leaf_position: vec![0],
                lower: Vec::new(),
                members: members.to_vec(),
                lakes: vec![None],
            };
        }
        for (i, &cell) in members.iter().enumerate() {
            local[cell as usize] = i as u32;
        }
        // Union–find over the members in ascending order; the node at the top of each part.
        let mut part = vec![NONE; count];
        let find = |part: &mut Vec<u32>, mut i: u32| {
            while part[i as usize] != i {
                let up = part[part[i as usize] as usize];
                part[i as usize] = up;
                i = up;
            }
            i
        };
        let mut top = vec![NONE; count];
        let mut nodes: Vec<Node> = Vec::new();
        let mut leaf_of = vec![NONE; count];
        let mut lower = vec![[NONE; 6]; count];
        let mut roots = Vec::with_capacity(6);
        for i in 0..count {
            let cell = members[i];
            let mut k = 0;
            for &neighbor in mesh.neighbors(cell) {
                let j = local[neighbor as usize];
                if j != NONE && (j as usize) < i && k < 6 {
                    lower[i][k] = j;
                    k += 1;
                }
            }
            part[i] = i as u32;
            roots.clear();
            for &j in lower[i].iter().take_while(|&&j| j != NONE) {
                let root = find(&mut part, j);
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
            // The water of a member runs to its lowest lower neighbour.
            let steepest = lower[i]
                .iter()
                .take_while(|&&j| j != NONE)
                .min_by(|&&a, &&b| {
                    elevation[members[a as usize] as usize]
                        .total_cmp(&elevation[members[b as usize] as usize])
                        .then(a.cmp(&b))
                })
                .copied();
            let node = match roots.len() {
                0 => {
                    nodes.push(Node {
                        parent: NONE,
                        children: Vec::new(),
                        own: Vec::new(),
                        sill: spill,
                        merge: NONE,
                        leaves: (0, 0),
                        lowest: i as u32,
                        absorbed: NONE,
                    });
                    leaf_of[i] = (nodes.len() - 1) as u32;
                    (nodes.len() - 1) as u32
                }
                1 => top[roots[0] as usize],
                _ => {
                    let level = elevation[cell as usize];
                    let mut children: Vec<u32> = roots.iter().map(|&r| top[r as usize]).collect();
                    children.sort_unstable_by_key(|&c| nodes[c as usize].lowest);
                    // Sub-basins less than LAKE_MIN_DEPTH below the inner sill (flat floors,
                    // rounding) are no basins of their own: their water runs on to the deepest
                    // neighbour.
                    let deep = |nodes: &[Node], c: u32| {
                        let lowest = members[nodes[c as usize].lowest as usize];
                        level - elevation[lowest as usize] > LAKE_MIN_DEPTH
                    };
                    let (kept, shallow): (Vec<u32>, Vec<u32>) =
                        children.iter().partition(|&&c| deep(&nodes, c));
                    let (kept, shallow) = if kept.len() >= 2 {
                        (kept, shallow)
                    } else {
                        // At most one deep one: all join the lowest.
                        (vec![children[0]], children[1..].to_vec())
                    };
                    let target = if kept.len() == 1 {
                        kept[0]
                    } else {
                        let id = nodes.len() as u32;
                        for &child in &kept {
                            nodes[child as usize].parent = id;
                            nodes[child as usize].sill = level;
                        }
                        nodes.push(Node {
                            parent: NONE,
                            children: kept.clone(),
                            own: Vec::new(),
                            sill: spill,
                            merge: i as u32,
                            leaves: (0, 0),
                            lowest: nodes[kept[0] as usize].lowest,
                            absorbed: NONE,
                        });
                        id
                    };
                    let drain = nodes[kept[0] as usize].lowest;
                    for absorbed in shallow {
                        let mut stack = vec![absorbed];
                        while let Some(n) = stack.pop() {
                            let own = std::mem::take(&mut nodes[n as usize].own);
                            nodes[target as usize].own.extend(own);
                            nodes[n as usize].absorbed = drain;
                            stack.extend(std::mem::take(&mut nodes[n as usize].children));
                        }
                    }
                    target
                }
            };
            if let Some(j) = steepest {
                leaf_of[i] = leaf_of[j as usize];
            }
            nodes[node as usize].own.push(i as u32);
            for &root in &roots {
                part[root as usize] = i as u32;
            }
            top[i] = node;
        }
        for &cell in members {
            local[cell as usize] = NONE;
        }
        // The water of absorbed sub-basins drains to the leaf of the lowest member of the
        // basin that took them in (absorbed in turn, possibly).
        for i in 0..count {
            let mut leaf = leaf_of[i];
            while nodes[leaf as usize].absorbed != NONE {
                leaf = leaf_of[nodes[leaf as usize].absorbed as usize];
            }
            leaf_of[i] = leaf;
        }
        let root = top[find(&mut part, 0) as usize];
        // Leaf positions in depth-first order, and the leaf range of every subtree.
        let mut leaf_position = vec![NONE; nodes.len()];
        let mut next = 0;
        let mut stack = vec![(root, false)];
        while let Some((node, done)) = stack.pop() {
            if done {
                let n = &nodes[node as usize];
                let start = n
                    .children
                    .iter()
                    .map(|&c| nodes[c as usize].leaves.0)
                    .min()
                    .unwrap_or(next);
                nodes[node as usize].leaves = (start, next);
                continue;
            }
            if nodes[node as usize].children.is_empty() {
                leaf_position[node as usize] = next;
                nodes[node as usize].leaves = (next, next + 1);
                next += 1;
            } else {
                stack.push((node, true));
                for &child in nodes[node as usize].children.iter().rev() {
                    stack.push((child, false));
                }
            }
        }
        let lakes = (0..nodes.len()).map(|_| None).collect();
        Self {
            nodes,
            root,
            leaf_of,
            leaf_position,
            lower,
            members: members.to_vec(),
            lakes,
        }
    }

    /// Number of sub-basins (local minima).
    #[cfg(test)]
    pub fn leaves(&self) -> usize {
        self.nodes.iter().filter(|n| n.children.is_empty()).count()
    }

    /// Solves the lakes for the water `inflow` (m³/yr) arriving at every member (runoff and
    /// rivers, `total` in all), where open water over a cell evaporates `excess(cell)` m³/yr
    /// more than the land.
    pub fn solve(
        &mut self,
        inflow: &[f64],
        total: f64,
        excess: &dyn Fn(usize) -> f64,
        elevation: &[f64],
    ) {
        let nodes = self.nodes.len();
        // Inflow of the leaves of every subtree, and evaporation of all its cells, full.
        let mut base = vec![0.0; nodes];
        let mut full = vec![0.0; nodes];
        for (i, &water) in inflow.iter().enumerate() {
            base[self.leaf_of[i] as usize] += water;
        }
        // Children come before their parents.
        for node in 0..nodes {
            let own: f64 = self.nodes[node]
                .own
                .iter()
                .map(|&i| excess(self.members[i as usize] as usize))
                .sum();
            full[node] += own;
            let parent = self.nodes[node].parent;
            if parent != NONE {
                base[parent as usize] += base[node];
                full[parent as usize] += full[node];
            }
        }
        let mut stack = vec![(self.root, total, Vec::<(u32, f64)>::new())];
        while let Some((node, input, extra)) = stack.pop() {
            let n = &self.nodes[node as usize];
            let below: f64 = n.children.iter().map(|&c| full[c as usize]).sum();
            if n.children.is_empty() || input > below || (below > 0.0 && input >= below) {
                let cells = self.subtree_cells(node);
                let balance = lake_balance(&cells, elevation, n.sill, input, excess);
                self.lakes[node as usize] = Some(Lake {
                    cells,
                    sill: n.sill,
                    balance,
                    sea_water: 0.0,
                });
                continue;
            }
            // The children's own water and what reaches them from above.
            let mut inputs: Vec<(f64, Vec<(u32, f64)>)> = n
                .children
                .iter()
                .map(|&c| (base[c as usize], Vec::new()))
                .collect();
            for &(leaf, water) in &extra {
                let k = self.child_with_leaf(node, leaf);
                inputs[k].0 += water;
                inputs[k].1.push((leaf, water));
            }
            // Full children spill their surplus over the inner sill into partial siblings,
            // the one with the lowest cell next to the sill first.
            let mut surplus = 0.0;
            for (k, &c) in n.children.iter().enumerate() {
                if inputs[k].0 >= full[c as usize] {
                    surplus += inputs[k].0 - full[c as usize];
                }
            }
            for (k, entry) in self.entries(node) {
                if surplus <= 0.0 {
                    break;
                }
                let c = self.nodes[node as usize].children[k];
                if inputs[k].0 >= full[c as usize] {
                    continue;
                }
                inputs[k].0 += surplus;
                inputs[k].1.push((self.leaf_of[entry as usize], surplus));
                surplus = (inputs[k].0 - full[c as usize]).max(0.0);
            }
            let children = self.nodes[node as usize].children.clone();
            for (k, (input, extra)) in inputs.into_iter().enumerate() {
                stack.push((children[k], input, extra));
            }
        }
    }

    /// Pours `volume` (m³ per cell area) of sea water into the depression over its spill level
    /// (entering at `entry`, a member next to the pass) and returns what does not fit.
    pub fn pour(&mut self, entry: u32, volume: f64, elevation: &[f64]) -> f64 {
        let leaf = self.leaf_of[entry as usize];
        self.pour_into(self.root, leaf, volume, elevation)
    }

    fn pour_into(&mut self, node: u32, leaf: u32, volume: f64, elevation: &[f64]) -> f64 {
        if volume <= 0.0 {
            return 0.0;
        }
        if let Some(lake) = &mut self.lakes[node as usize] {
            let before = lake_volume(&lake.cells, elevation, lake.balance.level, lake.sill);
            let room = lake_volume(&lake.cells, elevation, lake.sill, lake.sill) - before;
            if volume >= room {
                set_level(lake, elevation, lake.sill);
                lake.sea_water += room;
                return volume - room;
            }
            hold(lake, elevation, before + volume);
            lake.sea_water += volume;
            return 0.0;
        }
        // A partial inner node: the water fills the child it enters, then spills over the
        // inner sill into the partial siblings; when all are full, they merge.
        let k = self.child_with_leaf(node, leaf);
        let child = self.nodes[node as usize].children[k];
        let mut left = self.pour_into(child, leaf, volume, elevation);
        while left > 0.0 {
            let next = self.entries(node).into_iter().find(|&(k, _)| {
                let c = self.nodes[node as usize].children[k];
                !self.is_full(c)
            });
            let Some((k, entry)) = next else {
                // All sub-basins full: one lake from the inner sill up, holding their water
                // (cells of absorbed shallow sub-basins, dry so far, take their share).
                let cells = self.subtree_cells(node);
                let (mut sea_water, mut evaporation, mut volume) = (0.0, 0.0, 0.0);
                let mut stack = self.nodes[node as usize].children.clone();
                while let Some(n) = stack.pop() {
                    if let Some(lake) = self.lakes[n as usize].take() {
                        sea_water += lake.sea_water;
                        evaporation += lake.balance.evaporation;
                        volume +=
                            lake_volume(&lake.cells, elevation, lake.balance.level, lake.sill);
                    } else {
                        stack.extend(self.nodes[n as usize].children.iter().copied());
                    }
                }
                let sill = self.nodes[node as usize].sill;
                let lowest = elevation[cells[0] as usize];
                let mut lake = Lake {
                    cells,
                    sill,
                    balance: LakeBalance {
                        level: lowest,
                        covered: 0.0,
                        evaporation,
                        endorheic: true,
                    },
                    sea_water,
                };
                hold(&mut lake, elevation, volume);
                self.lakes[node as usize] = Some(lake);
                return self.pour_into(node, leaf, left, elevation);
            };
            let c = self.nodes[node as usize].children[k];
            left = self.pour_into(c, self.leaf_of[entry as usize], left, elevation);
        }
        0.0
    }

    /// Whether the water of `node` stands at its sill.
    fn is_full(&self, node: u32) -> bool {
        match &self.lakes[node as usize] {
            Some(lake) => lake.balance.level >= lake.sill,
            None => false,
        }
    }

    /// The lakes, each a water body of one node.
    pub fn lakes(&self) -> impl Iterator<Item = &Lake> {
        self.lakes.iter().flatten()
    }

    /// Whether the depression spills over its spill level.
    pub fn spills(&self) -> bool {
        self.lakes[self.root as usize]
            .as_ref()
            .is_some_and(|lake| !lake.balance.endorheic)
    }

    /// The index of the child of `node` whose subtree holds `leaf`.
    fn child_with_leaf(&self, node: u32, leaf: u32) -> usize {
        let position = self.leaf_position[leaf as usize];
        self.nodes[node as usize]
            .children
            .iter()
            .position(|&c| {
                let (start, end) = self.nodes[c as usize].leaves;
                (start..end).contains(&position)
            })
            .expect("the leaf lies below the node")
    }

    /// The children of an inner node (their indices) with the member through which water
    /// spilling over the inner sill enters them (the sill's lowest neighbour in them), ordered
    /// by that member's floor (water takes the steepest way first).
    fn entries(&self, node: u32) -> Vec<(usize, u32)> {
        let n = &self.nodes[node as usize];
        let merge = n.merge as usize;
        let mut entries: Vec<(usize, u32)> = Vec::with_capacity(n.children.len());
        // Members ascend by floor, so the lowest neighbour in each child is the smallest index.
        let mut neighbours: Vec<u32> = self.lower[merge]
            .iter()
            .copied()
            .take_while(|&j| j != NONE)
            .collect();
        neighbours.sort_unstable();
        for j in neighbours {
            let k = self.child_with_leaf(node, self.leaf_of[j as usize]);
            if !entries.iter().any(|&(e, _)| e == k) {
                entries.push((k, j));
            }
        }
        entries
    }

    /// The cells of the subtree of `node`, ascending.
    fn subtree_cells(&self, node: u32) -> Vec<u32> {
        let mut members = Vec::new();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            members.extend_from_slice(&self.nodes[n as usize].own);
            stack.extend(self.nodes[n as usize].children.iter().copied());
        }
        members.sort_unstable();
        members.iter().map(|&i| self.members[i as usize]).collect()
    }
}

/// The covered cells (from the lowest floor, the last one in part) of a lake with the `cells`
/// (ascending floor) whose water stands at `level`, full at `sill`: as in
/// [`super::lake_balance`], the level rises linearly from one floor to the next.
fn covered_at(cells: &[u32], elevation: &[f64], level: f64, sill: f64) -> f64 {
    if level >= sill {
        return cells.len() as f64;
    }
    for (i, &cell) in cells.iter().enumerate() {
        let floor = elevation[cell as usize];
        let next = cells
            .get(i + 1)
            .map_or(sill, |&c| elevation[c as usize])
            .min(sill);
        if level < next {
            return if level > floor && next > floor {
                i as f64 + (level - floor) / (next - floor)
            } else {
                i as f64
            };
        }
    }
    cells.len() as f64
}

/// Water (m per cell area) in a lake over `cells` standing at `level` (see [`covered_at`]).
fn lake_volume(cells: &[u32], elevation: &[f64], level: f64, sill: f64) -> f64 {
    let covered = covered_at(cells, elevation, level, sill);
    cells
        .iter()
        .enumerate()
        .map(|(i, &cell)| {
            (covered - i as f64).clamp(0.0, 1.0) * lake_water_depth(level, elevation[cell as usize])
        })
        .sum()
}

/// Sets the water surface of a lake to hold `volume` (m per cell area, at most its volume when
/// full).
fn hold(lake: &mut Lake, elevation: &[f64], volume: f64) {
    let (mut low, mut high) = (elevation[lake.cells[0] as usize], lake.sill);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if lake_volume(&lake.cells, elevation, mid, lake.sill) < volume {
            low = mid;
        } else {
            high = mid;
        }
    }
    set_level(lake, elevation, high);
}

/// Sets the water surface of a lake.
fn set_level(lake: &mut Lake, elevation: &[f64], level: f64) {
    lake.balance.level = level;
    lake.balance.covered = covered_at(&lake.cells, elevation, level, lake.sill);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::WATER_LOADING;

    /// Three cells in a row: sub-basin `a` (floor 0 m), the inner sill `s` (50 m) and sub-basin
    /// `b` (10 m); the depression spills at 100 m. Members ascending: a, b, s.
    fn row() -> (SphereMesh, Vec<u32>, Vec<f64>) {
        let mesh = SphereMesh::icosphere(3);
        let a = 0u32;
        let s = mesh.neighbors(a)[0];
        let b = *mesh
            .neighbors(s)
            .iter()
            .find(|&&n| n != a && !mesh.neighbors(a).contains(&n))
            .unwrap();
        let mut elevation = vec![1000.0; mesh.len()];
        elevation[a as usize] = 0.0;
        elevation[b as usize] = 10.0;
        elevation[s as usize] = 50.0;
        (mesh, vec![a, b, s], elevation)
    }

    fn solved(hierarchy: bool, into_b: f64) -> (Basins, Vec<u32>, Vec<f64>) {
        let (mesh, members, elevation) = row();
        let mut local = vec![NONE; mesh.len()];
        let mut basins = Basins::new(&mesh, &members, &elevation, 100.0, &mut local, hierarchy);
        assert!(local.iter().all(|&l| l == NONE));
        basins.solve(&[0.0, into_b, 0.0], into_b, &|_| 1.0, &elevation);
        (basins, members, elevation)
    }

    /// Water covering each member.
    fn cover(basins: &Basins, members: &[u32]) -> Vec<f64> {
        let mut cover = vec![0.0; members.len()];
        for lake in basins.lakes() {
            for (i, cell) in lake.cells.iter().enumerate() {
                let k = members.iter().position(|m| m == cell).unwrap();
                cover[k] += (lake.balance.covered - i as f64).clamp(0.0, 1.0);
            }
        }
        cover
    }

    #[test]
    fn a_sub_basin_fills_before_it_spills_over_its_inner_sill() {
        // Water only into b: it stays in b until b is full.
        let (basins, members, _) = solved(true, 0.5);
        assert_eq!(basins.leaves(), 2);
        assert_eq!(cover(&basins, &members), [0.0, 0.5, 0.0]);
        assert!(!basins.spills());
        // Without the hierarchy the water would cover the lowest floor, a, which no water
        // reaches.
        let (basins, members, _) = solved(false, 0.5);
        assert_eq!(cover(&basins, &members), [0.5, 0.0, 0.0]);
        // b full: its surplus spills over the sill into a.
        let (basins, members, _) = solved(true, 1.5);
        assert_eq!(cover(&basins, &members), [0.5, 1.0, 0.0]);
        let levels: Vec<f64> = basins.lakes().map(|l| l.balance.level).collect();
        assert!(levels.contains(&50.0), "{levels:?}");
        // Both full: one lake above the inner sill, evaporating all its inflow.
        let (basins, members, _) = solved(true, 2.5);
        assert_eq!(basins.lakes().count(), 1);
        let lake = basins.lakes().next().unwrap();
        assert_eq!(lake.balance.level, 75.0);
        assert_eq!(lake.balance.evaporation, 2.5);
        assert_eq!(cover(&basins, &members), [1.0, 1.0, 0.5]);
        assert!(!basins.spills());
        // Full to the spill level: the depression spills.
        let (basins, _, _) = solved(true, 3.5);
        assert!(basins.spills());
        assert_eq!(basins.lakes().next().unwrap().balance.evaporation, 3.0);
    }

    #[test]
    fn sea_water_fills_the_sub_basin_it_enters_first() {
        let (mut basins, members, elevation) = solved(true, 0.0);
        // Entering at the inner sill, the water runs down to a (the lower side). Within a cell
        // the covered part grows with the level (as in `lake_balance`), so a cell from floor f
        // to the next floor n holds (l − f)² / (n − f) at level l.
        assert_eq!(basins.pour(2, 10.0 * WATER_LOADING, &elevation), 0.0);
        let a = basins.lakes().find(|l| l.cells == [members[0]]).unwrap();
        assert!((a.balance.level - 500f64.sqrt()).abs() < 1e-9, "{a:?}");
        // a holds 50 m of water up to the sill; the rest goes into b.
        assert_eq!(basins.pour(2, 50.0 * WATER_LOADING, &elevation), 0.0);
        let b = basins.lakes().find(|l| l.cells == [members[1]]).unwrap();
        assert!((b.balance.level - 30.0).abs() < 1e-9, "{b:?}");
        // Both full at the inner sill (90 m of water), then one lake up to the spill level (240 m);
        // beyond it nothing fits.
        let room = 100.0 + 90.0 + 50.0 - 60.0;
        let left = basins.pour(2, (room + 7.0) * WATER_LOADING, &elevation);
        assert!((left / WATER_LOADING - 7.0).abs() < 1e-6, "{left}");
        assert_eq!(basins.lakes().count(), 1);
        let lake = basins.lakes().next().unwrap();
        assert_eq!(lake.balance.level, 100.0);
        let poured = lake.sea_water / WATER_LOADING;
        assert!((poured - (room + 60.0)).abs() < 1e-6, "{poured}");
    }
}
