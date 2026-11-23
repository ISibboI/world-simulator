//! The compact form of a frame: quantised cell attributes in byte planes ([`Planes`]), and the
//! encoding of a frame as a keyframe or as the difference to the previous one, compressed with
//! lz4 ([`encode`], [`decode`]).
//!
//! Each attribute of [`CellInfo`] becomes a channel of 1 or 2 bytes per cell (31 bytes per cell
//! instead of 68, see [`CHANNELS`]). A 2-byte channel is stored as two planes, the low bytes of
//! all cells and then the high bytes, so that the high bytes, which rarely change, form long runs.
//! A delta frame is the byte-wise difference (wrapping) of its planes to those of the previous
//! frame: zero where nothing changed. lz4 then compresses the runs.

use super::super::{CellInfo, PlateInfo};

/// The quantisation of one attribute.
#[derive(Debug, Clone, Copy)]
enum Code {
    /// Signed fixed point with this step, 2 bytes; `i16::MIN` is NaN.
    Signed(f32),
    /// Unsigned fixed point with this step, 2 bytes; `u16::MAX` is NaN.
    Unsigned(f32),
    /// A share from 0 to 1 in steps of 1/254, 1 byte; 255 is NaN.
    Share,
    /// ln(1 + value) with this step, 2 bytes, for values spanning many orders of magnitude;
    /// `u16::MAX` is NaN.
    Logarithmic(f32),
    /// An integer of up to 16 bits (larger ones are clamped).
    Integer16,
    /// An integer of up to 8 bits.
    Integer8,
}

impl Code {
    const fn bytes(self) -> usize {
        match self {
            Code::Share | Code::Integer8 => 1,
            _ => 2,
        }
    }
}

/// The channels in the order of the planes: the attributes of [`CellInfo`] in its order.
const CHANNELS: [Code; 17] = [
    // Elevation: 1 m, ±32 km.
    Code::Signed(1.0),
    // Crust thickness: 10 m, up to 655 km.
    Code::Unsigned(10.0),
    // Lithosphere age: 0.25 Myr, up to 16 Gyr.
    Code::Unsigned(0.25),
    // Sediment: 1 m, up to 65 km.
    Code::Unsigned(1.0),
    // Orogenic share.
    Code::Share,
    // Time since the last orogeny: 0.25 Myr.
    Code::Unsigned(0.25),
    // Temperatures (annual, January, July): 0.1 °C.
    Code::Signed(0.1),
    Code::Signed(0.1),
    Code::Signed(0.1),
    // Precipitation (annual, January, July) and potential evaporation: 1 mm/yr, up to 65 m/yr.
    Code::Unsigned(0.001),
    Code::Unsigned(0.001),
    Code::Unsigned(0.001),
    Code::Unsigned(0.001),
    // Ice share.
    Code::Share,
    // Discharge: 0.6 % relative (the widget shows 3 digits), up to 10^38 m³/yr.
    Code::Logarithmic(1.0 / 170.0),
    Code::Integer16,
    Code::Integer8,
];

/// Bytes per cell of the quantised attributes.
pub const BYTES_PER_CELL: usize = {
    let mut bytes = 0;
    let mut index = 0;
    while index < CHANNELS.len() {
        bytes += CHANNELS[index].bytes();
        index += 1;
    }
    bytes
};

/// The attributes of a cell as the channel values.
fn attributes(cell: &CellInfo) -> [f32; CHANNELS.len()] {
    [
        cell.elevation,
        cell.crust_thickness,
        cell.age,
        cell.sediment,
        cell.orogenic,
        cell.since_orogeny,
        cell.temperature,
        cell.temperature_january,
        cell.temperature_july,
        cell.precipitation,
        cell.precipitation_january,
        cell.precipitation_july,
        cell.potential_evaporation,
        cell.ice,
        cell.discharge,
        0.0,
        0.0,
    ]
}

fn quantise(code: Code, value: f32) -> u16 {
    match code {
        Code::Signed(step) => {
            if value.is_nan() {
                return i16::MIN as u16;
            }
            let mut steps = (value / step).round();
            // Below zero stays below zero: an ocean cell just under sea level is not land.
            if value < 0.0 && steps == 0.0 {
                steps = -1.0;
            }
            steps.clamp(-f32::from(i16::MAX), f32::from(i16::MAX)) as i16 as u16
        }
        Code::Unsigned(step) => {
            if value.is_nan() {
                return u16::MAX;
            }
            (value / step).round().clamp(0.0, f32::from(u16::MAX - 1)) as u16
        }
        Code::Share => {
            if value.is_nan() {
                return 255;
            }
            (value * 254.0).round().clamp(0.0, 254.0) as u16
        }
        Code::Logarithmic(step) => {
            if value.is_nan() {
                return u16::MAX;
            }
            // Up to e^88, about the largest f32.
            (value.max(0.0).ln_1p() / step)
                .round()
                .clamp(0.0, (88.0 / step).floor().min(f32::from(u16::MAX - 1))) as u16
        }
        Code::Integer16 | Code::Integer8 => unreachable!("integers are not quantised"),
    }
}

fn dequantise(code: Code, value: u16) -> f32 {
    match code {
        Code::Signed(step) => {
            let value = value as i16;
            if value == i16::MIN {
                f32::NAN
            } else {
                f32::from(value) * step
            }
        }
        Code::Unsigned(step) => {
            if value == u16::MAX {
                f32::NAN
            } else {
                f32::from(value) * step
            }
        }
        Code::Share => {
            if value == 255 {
                f32::NAN
            } else {
                f32::from(value) / 254.0
            }
        }
        Code::Logarithmic(step) => {
            if value == u16::MAX {
                f32::NAN
            } else {
                (f32::from(value) * step).exp_m1()
            }
        }
        Code::Integer16 | Code::Integer8 => unreachable!("integers are not quantised"),
    }
}

/// The quantised attributes of every cell of a frame, plane by plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planes {
    cells: usize,
    bytes: Vec<u8>,
}

impl Planes {
    /// Quantises the attributes of the cells.
    pub fn new(cells: &[CellInfo]) -> Self {
        let count = cells.len();
        let mut bytes = vec![0u8; count * BYTES_PER_CELL];
        for (index, cell) in cells.iter().enumerate() {
            let values = attributes(cell);
            let mut plane = 0;
            for (channel, code) in CHANNELS.iter().enumerate() {
                let value = match code {
                    Code::Integer16 => cell.plate.min(u32::from(u16::MAX)) as u16,
                    Code::Integer8 => (cell.flags & 0xFF) as u16,
                    _ => quantise(*code, values[channel]),
                };
                for byte in 0..code.bytes() {
                    bytes[(plane + byte) * count + index] = (value >> (8 * byte)) as u8;
                }
                plane += code.bytes();
            }
        }
        Self {
            cells: count,
            bytes,
        }
    }

    /// The attributes of the cells, as far as the quantisation keeps them.
    pub fn cell_infos(&self) -> Vec<CellInfo> {
        let count = self.cells;
        (0..count)
            .map(|index| {
                let mut values = [0u16; CHANNELS.len()];
                let mut plane = 0;
                for (channel, code) in CHANNELS.iter().enumerate() {
                    for byte in 0..code.bytes() {
                        values[channel] |=
                            u16::from(self.bytes[(plane + byte) * count + index]) << (8 * byte);
                    }
                    plane += code.bytes();
                }
                let value = |channel: usize| dequantise(CHANNELS[channel], values[channel]);
                CellInfo {
                    elevation: value(0),
                    crust_thickness: value(1),
                    age: value(2),
                    sediment: value(3),
                    orogenic: value(4),
                    since_orogeny: value(5),
                    temperature: value(6),
                    temperature_january: value(7),
                    temperature_july: value(8),
                    precipitation: value(9),
                    precipitation_january: value(10),
                    precipitation_july: value(11),
                    potential_evaporation: value(12),
                    ice: value(13),
                    discharge: value(14),
                    plate: u32::from(values[15]),
                    flags: u32::from(values[16]),
                }
            })
            .collect()
    }
}

/// Whether a frame stands alone or is the difference to the previous one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Key,
    Delta,
}

/// The state of a frame as the history keeps it: its time, the quantised cells and the
/// angular velocity of every plate (rad/Myr, by plate id).
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub time: f64,
    pub planes: Planes,
    pub plates: Vec<PlateInfo>,
}

/// Version of the format, the first byte of every frame.
const VERSION: u8 = 3;
/// Bytes per plate: its rotation and id.
pub const PLATE_BYTES: usize = 16;
/// Bytes before the compressed part: version, kind, time, cells, plates.
const HEADER: usize = 2 + 8 + 4 + 4;

/// Encodes a frame: as the difference to `previous` (the state of the frame before it, of the
/// same cells), or as a keyframe if there is none.
pub fn encode(state: &State, previous: Option<&State>) -> Vec<u8> {
    let previous = previous.filter(|previous| previous.planes.cells == state.planes.cells);
    let kind = if previous.is_some() {
        Kind::Delta
    } else {
        Kind::Key
    };
    let mut body = Vec::with_capacity(state.plates.len() * PLATE_BYTES + state.planes.bytes.len());
    // Plates are few and their rotations change every step: as they are.
    for plate in &state.plates {
        for component in plate.rotation {
            body.extend_from_slice(&component.to_le_bytes());
        }
        body.extend_from_slice(&plate.id.to_le_bytes());
    }
    match previous {
        Some(previous) => body.extend(
            state
                .planes
                .bytes
                .iter()
                .zip(&previous.planes.bytes)
                .map(|(byte, before)| byte.wrapping_sub(*before)),
        ),
        None => body.extend_from_slice(&state.planes.bytes),
    }
    let compressed = lz4_flex::compress(&body);
    let mut frame = Vec::with_capacity(HEADER + compressed.len());
    frame.push(VERSION);
    frame.push(match kind {
        Kind::Key => 0,
        Kind::Delta => 1,
    });
    frame.extend_from_slice(&state.time.to_le_bytes());
    frame.extend_from_slice(&(state.planes.cells as u32).to_le_bytes());
    frame.extend_from_slice(&(state.plates.len() as u32).to_le_bytes());
    frame.extend_from_slice(&compressed);
    frame
}

/// The kind of an encoded frame.
pub fn kind(frame: &[u8]) -> Result<Kind, String> {
    match frame.get(..2) {
        Some([VERSION, 0]) => Ok(Kind::Key),
        Some([VERSION, 1]) => Ok(Kind::Delta),
        _ => Err("not a frame of this version".to_owned()),
    }
}

/// Decodes a frame: a keyframe alone, a delta frame on the state of the frame before it.
pub fn decode(frame: &[u8], previous: Option<&State>) -> Result<State, String> {
    let kind = kind(frame)?;
    let header = frame.get(..HEADER).ok_or("truncated frame")?;
    let number = |range: std::ops::Range<usize>| {
        u32::from_le_bytes(header[range].try_into().expect("4 bytes")) as usize
    };
    let time = f64::from_le_bytes(header[2..10].try_into().expect("8 bytes"));
    let cells = number(10..14);
    let plates = number(14..18);
    let plate_bytes = plates * PLATE_BYTES;
    let size = plate_bytes + cells * BYTES_PER_CELL;
    let body = lz4_flex::decompress(&frame[HEADER..], size)
        .map_err(|error| format!("corrupt frame: {error}"))?;
    if body.len() != size {
        return Err("frame of the wrong size".to_owned());
    }
    let plates = body[..plate_bytes]
        .as_chunks::<PLATE_BYTES>()
        .0
        .iter()
        .map(|chunk| {
            let word = |at: usize| chunk[at..at + 4].try_into().expect("4 bytes");
            let component = |at: usize| f32::from_le_bytes(word(at));
            PlateInfo {
                rotation: [component(0), component(4), component(8)],
                id: u32::from_le_bytes(word(12)),
            }
        })
        .collect();
    let mut bytes = body[plate_bytes..].to_vec();
    if kind == Kind::Delta {
        let previous = previous
            .filter(|previous| previous.planes.cells == cells)
            .ok_or("a delta frame without the frame before it")?;
        for (byte, before) in bytes.iter_mut().zip(&previous.planes.bytes) {
            *byte = byte.wrapping_add(*before);
        }
    }
    Ok(State {
        time,
        planes: Planes { cells, bytes },
        plates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(index: usize, time: f32) -> CellInfo {
        let x = index as f32;
        CellInfo {
            elevation: (x * 0.37).sin() * 6000.0 + time,
            crust_thickness: 7000.0 + x * 3.0,
            age: x * 0.1 + time,
            sediment: x * 0.5,
            orogenic: (x * 0.01).fract(),
            since_orogeny: if index.is_multiple_of(3) {
                f32::NAN
            } else {
                x * 0.25
            },
            temperature: (x * 0.1).cos() * 30.0,
            temperature_january: f32::NAN,
            temperature_july: -12.34,
            precipitation: x * 0.001,
            precipitation_january: x * 0.0013,
            precipitation_july: if index.is_multiple_of(4) {
                f32::NAN
            } else {
                x * 0.0007
            },
            potential_evaporation: (x * 0.05).sin().abs() * 2.0,
            ice: if index.is_multiple_of(5) { 1.0 } else { 0.0 },
            discharge: if index.is_multiple_of(7) {
                0.0
            } else {
                1.0e8 * x
            },
            plate: (index % 13) as u32,
            flags: (index % 2) as u32,
        }
    }

    fn cells(count: usize, time: f32) -> Vec<CellInfo> {
        (0..count).map(|index| cell(index, time)).collect()
    }

    /// Whether `decoded` is `original` within the resolution of the quantisation.
    fn assert_close(original: &CellInfo, decoded: &CellInfo) {
        let pairs = [
            (original.elevation, decoded.elevation, 1.0),
            (original.crust_thickness, decoded.crust_thickness, 5.0),
            (original.age, decoded.age, 0.125),
            (original.sediment, decoded.sediment, 0.5),
            (original.orogenic, decoded.orogenic, 0.5 / 254.0),
            (original.since_orogeny, decoded.since_orogeny, 0.125),
            (original.temperature, decoded.temperature, 0.05),
            (
                original.temperature_january,
                decoded.temperature_january,
                0.05,
            ),
            (original.temperature_july, decoded.temperature_july, 0.05),
            (original.precipitation, decoded.precipitation, 0.0005),
            (
                original.precipitation_january,
                decoded.precipitation_january,
                0.0005,
            ),
            (
                original.precipitation_july,
                decoded.precipitation_july,
                0.0005,
            ),
            (
                original.potential_evaporation,
                decoded.potential_evaporation,
                0.0005,
            ),
            (original.ice, decoded.ice, 0.5 / 254.0),
        ];
        for (index, (a, b, tolerance)) in pairs.into_iter().enumerate() {
            if a.is_nan() {
                assert!(b.is_nan(), "attribute {index}: {a} → {b}");
            } else {
                assert!(
                    (a - b).abs() <= tolerance * 1.001,
                    "attribute {index}: {a} → {b}"
                );
            }
        }
        let relative = (original.discharge - decoded.discharge).abs() / original.discharge.max(1.0);
        assert!(
            relative < 5e-3,
            "{} → {}",
            original.discharge,
            decoded.discharge
        );
        assert_eq!(original.plate, decoded.plate);
        assert_eq!(original.flags, decoded.flags);
        // Land stays land and sea stays sea.
        assert_eq!(original.elevation >= 0.0, decoded.elevation >= 0.0);
    }

    #[test]
    fn quantisation_keeps_the_attributes_within_their_resolution() {
        let original = cells(500, 0.0);
        let decoded = Planes::new(&original).cell_infos();
        assert_eq!(decoded.len(), original.len());
        for (original, decoded) in original.iter().zip(&decoded) {
            assert_close(original, decoded);
        }
        assert_eq!(BYTES_PER_CELL, 31);
    }

    #[test]
    fn extremes_are_clamped_and_sea_level_is_kept() {
        let mut extreme = cell(1, 0.0);
        extreme.elevation = -0.2;
        extreme.crust_thickness = 1e9;
        extreme.precipitation = -1.0;
        extreme.discharge = f32::INFINITY;
        let decoded = Planes::new(&[extreme]).cell_infos()[0];
        assert!(decoded.elevation < 0.0);
        assert_eq!(decoded.crust_thickness, 10.0 * f32::from(u16::MAX - 1));
        assert_eq!(decoded.precipitation, 0.0);
        assert!(decoded.discharge.is_finite() && decoded.discharge > 1e16);
        let mut zero = extreme;
        zero.elevation = 0.0;
        assert_eq!(Planes::new(&[zero]).cell_infos()[0].elevation, 0.0);
    }

    #[test]
    fn frames_round_trip_as_keyframes_and_deltas() {
        let states: Vec<State> = (0..5)
            .map(|step| State {
                time: f64::from(step),
                planes: Planes::new(&cells(300, step as f32)),
                plates: vec![
                    PlateInfo {
                        rotation: [0.01 * step as f32, 0.0, -0.02],
                        id: step as u32 * 3,
                    };
                    4
                ],
            })
            .collect();
        let mut previous: Option<State> = None;
        for state in &states {
            let frame = encode(state, previous.as_ref());
            assert_eq!(
                kind(&frame).unwrap(),
                if previous.is_some() {
                    Kind::Delta
                } else {
                    Kind::Key
                }
            );
            let decoded = decode(&frame, previous.as_ref()).unwrap();
            assert_eq!(&decoded, state);
            // A delta frame cannot be decoded alone.
            if previous.is_some() {
                assert!(decode(&frame, None).is_err());
            }
            previous = Some(decoded);
        }
        // Another number of cells (a restart) starts with a keyframe.
        let other = State {
            time: 0.0,
            planes: Planes::new(&cells(10, 0.0)),
            plates: Vec::new(),
        };
        let frame = encode(&other, previous.as_ref());
        assert_eq!(kind(&frame).unwrap(), Kind::Key);
        assert!(decode(&frame[..frame.len() - 1], None).is_err());
        assert!(decode(&[], None).is_err());
    }

    #[test]
    fn unchanged_frames_compress_to_almost_nothing() {
        let state = State {
            time: 1.0,
            planes: Planes::new(&cells(10_000, 0.0)),
            plates: vec![
                PlateInfo {
                    rotation: [0.0; 3],
                    id: 1
                };
                12
            ],
        };
        let key = encode(&state, None);
        let delta = encode(&state, Some(&state));
        assert!(delta.len() < 2_000, "{} bytes", delta.len());
        assert!(key.len() > delta.len());
    }

    /// Measures the sizes of real frames (`cargo test --release frame_sizes -- --ignored
    /// --nocapture`, with `FRAME_SIZES_SUBDIVISIONS` and `FRAME_SIZES_STEPS`): raw, keyframe,
    /// delta frames, and the compressed size of each channel's delta, for `results.md`.
    #[test]
    #[ignore]
    fn frame_sizes() {
        use clap::Parser;
        let variable = |name: &str, default: u32| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(default)
        };
        let subdivisions = variable("FRAME_SIZES_SUBDIVISIONS", 5);
        let steps = variable("FRAME_SIZES_STEPS", 100);
        let cli = crate::Cli::parse_from([
            "world-simulator".to_owned(),
            format!("--subdivisions={subdivisions}"),
        ]);
        let mut planet = crate::build_planet(&cli).unwrap();
        let state = |planet: &crate::planet::Planet| State {
            time: planet.tectonics().time(),
            planes: Planes::new(&crate::viewer::cell_infos(planet)),
            plates: crate::viewer::fields::plate_infos(planet),
        };
        let mut previous = state(&planet);
        let cells = previous.planes.cells;
        let raw = cells * BYTES_PER_CELL + previous.plates.len() * PLATE_BYTES;
        let mut keys = vec![encode(&previous, None).len()];
        let mut deltas = Vec::new();
        let mut channels = vec![0usize; CHANNELS.len()];
        let (mut encode_seconds, mut decode_seconds) = (0.0, 0.0);
        for step in 1..=steps {
            planet.step(cli.dt);
            let current = state(&planet);
            keys.push(encode(&current, None).len());
            let start = std::time::Instant::now();
            let delta = encode(&current, Some(&previous));
            encode_seconds += start.elapsed().as_secs_f64();
            let start = std::time::Instant::now();
            let decoded = decode(&delta, Some(&previous)).unwrap();
            let _ = decoded.planes.cell_infos();
            decode_seconds += start.elapsed().as_secs_f64();
            deltas.push(delta.len());
            if step > steps / 2 {
                let mut plane = 0;
                for (channel, code) in CHANNELS.iter().enumerate() {
                    let range = plane * cells..(plane + code.bytes()) * cells;
                    let delta: Vec<u8> = current.planes.bytes[range.clone()]
                        .iter()
                        .zip(&previous.planes.bytes[range])
                        .map(|(a, b)| a.wrapping_sub(*b))
                        .collect();
                    channels[channel] += lz4_flex::compress(&delta).len();
                    plane += code.bytes();
                }
            }
            previous = current;
        }
        let mean = |values: &[usize]| values.iter().sum::<usize>() as f64 / values.len() as f64;
        let late = |values: &[usize]| mean(&values[values.len() / 2..]);
        eprintln!(
            "subdivisions {subdivisions}, {cells} cells, {} plates at the end, {steps} steps",
            previous.plates.len()
        );
        eprintln!(
            "raw (quantised) {raw} B/frame; CellInfo {} B/frame",
            cells * std::mem::size_of::<CellInfo>()
        );
        eprintln!(
            "keyframe: first {} B, mean {:.0} B, second half {:.0} B",
            keys[0],
            mean(&keys),
            late(&keys)
        );
        eprintln!(
            "delta: first {} B, mean {:.0} B, second half {:.0} B",
            deltas[0],
            mean(&deltas),
            late(&deltas)
        );
        eprintln!(
            "delta frame: encode {:.2} ms, decode (with the cells) {:.2} ms",
            encode_seconds * 1e3 / f64::from(steps),
            decode_seconds * 1e3 / f64::from(steps)
        );
        let per_frame = (late(&keys) + (super::super::KEY_INTERVAL as f64 - 1.0) * late(&deltas))
            / super::super::KEY_INTERVAL as f64;
        eprintln!(
            "stored per frame (1 key in {}): {per_frame:.0} B",
            super::super::KEY_INTERVAL
        );
        let counted = (steps - steps / 2) as f64;
        for (channel, bytes) in channels.iter().enumerate() {
            eprintln!(
                "channel {channel:2}: delta {:.0} B/frame",
                *bytes as f64 / counted
            );
        }
    }
}
