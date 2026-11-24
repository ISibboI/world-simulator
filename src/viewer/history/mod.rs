//! The frame history of a run: every state the view received, compact, so that the time bar
//! ([`super::timeline`]) can show any past one.
//!
//! A frame keeps what the view needs to rebuild everything it shows: the attributes of the
//! cells, quantised ([`codec`], 52 bytes per cell), and the angular velocity of every plate, for
//! the layers derived from plates. The vertices follow from the cells. Frames are stored as the
//! difference to the frame before (mostly zeros), compressed with lz4, with a keyframe every
//! [`KEY_INTERVAL`] frames, so that showing a frame decodes at most that many. A [`FrameStore`]
//! keeps the encoded frames: a temporary file on the desktop, IndexedDB on the web; only the
//! state of the latest frame (the base of the next difference) and the last decoded one stay in
//! memory.
//!
//! When the store is full (its capacity, or a failed write such as an exhausted browser quota),
//! the history stops recording: the frames it has stay viewable.

mod codec;
pub mod store;

use std::ops::Range;

use super::{CellInfo, PlateInfo};
pub use codec::BYTES_PER_CELL;
use codec::{Kind, Planes, State};
pub use store::FrameStore;

/// Every this many frames, one is a keyframe.
pub const KEY_INTERVAL: usize = 25;

/// What the history knows about a frame without decoding it.
#[derive(Debug, Clone, Copy)]
struct Entry {
    time: f64,
    key: bool,
    /// Its write failed, or it depends on one that did.
    lost: bool,
}

/// A decoded past frame.
#[derive(Debug, Clone)]
pub struct Frame {
    pub index: usize,
    pub cells: Vec<CellInfo>,
    /// Every plate, by index.
    pub plates: Vec<PlateInfo>,
}

/// The frames of one run.
pub struct History {
    store: Box<dyn FrameStore>,
    frames: Vec<Entry>,
    /// The state of the last recorded frame, the base of the next delta frame.
    latest: Option<State>,
    /// Frames since the last keyframe.
    since_key: usize,
    /// Bytes of the encoded frames, and their size before compression.
    stored: u64,
    raw: u64,
    /// The history no longer records (see the module documentation).
    full: bool,
    /// The last decoded frame, from which the next ones decode with fewer reads.
    decoded: Option<(usize, State)>,
    /// The read in progress: from which frame, to which.
    reading: Option<(usize, usize)>,
    /// The frame to show next, while a read is in progress.
    wanted: Option<usize>,
    /// The result for the view.
    ready: Option<Result<Frame, String>>,
}

impl History {
    pub fn new(store: Box<dyn FrameStore>) -> Self {
        Self {
            store,
            frames: Vec::new(),
            latest: None,
            since_key: 0,
            stored: 0,
            raw: 0,
            full: false,
            decoded: None,
            reading: None,
            wanted: None,
            ready: None,
        }
    }

    /// Records a state, if it is later than the last recorded one (the simulation sends a state
    /// again when the view asks for other fields) and the store has room. Returns whether it
    /// was recorded.
    pub fn record(&mut self, time: f64, cells: &[CellInfo], plates: &[PlateInfo]) -> bool {
        self.check_writes();
        if self.full || self.frames.last().is_some_and(|entry| time <= entry.time) {
            return false;
        }
        let state = State {
            time,
            planes: Planes::new(cells),
            plates: plates.to_vec(),
        };
        let key = self.since_key + 1 >= KEY_INTERVAL;
        let previous = if key { None } else { self.latest.as_ref() };
        let frame = codec::encode(&state, previous);
        let kind = codec::kind(&frame).unwrap_or(Kind::Key);
        if self.stored + frame.len() as u64 > self.store.capacity() {
            tracing::info!(
                frames = self.frames.len(),
                bytes = self.stored,
                "Frame history full; it stops recording"
            );
            self.full = true;
            return false;
        }
        let index = self.frames.len();
        tracing::debug!(
            index,
            key = kind == Kind::Key,
            raw = BYTES_PER_CELL * cells.len() + codec::PLATE_BYTES * plates.len(),
            encoded = frame.len(),
            "Frame recorded"
        );
        self.stored += frame.len() as u64;
        self.raw += (BYTES_PER_CELL * cells.len() + codec::PLATE_BYTES * plates.len()) as u64;
        self.store.append(index, frame);
        self.frames.push(Entry {
            time,
            key: kind == Kind::Key,
            lost: false,
        });
        self.since_key = if kind == Kind::Key {
            0
        } else {
            self.since_key + 1
        };
        self.latest = Some(state);
        true
    }

    /// Marks the frames from a failed write on as lost and stops recording.
    fn check_writes(&mut self) {
        if let Some(failed) = self.store.failed_write()
            && !self.frames.get(failed).is_none_or(|entry| entry.lost)
        {
            tracing::warn!(
                frame = failed,
                "A frame of the history could not be stored; it stops recording"
            );
            // Every later frame is a delta on it or was written after it (and fails as well).
            for entry in &mut self.frames[failed..] {
                entry.lost = true;
            }
            self.full = true;
        }
    }

    /// The number of recorded frames.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// The simulated time of a frame in Myr.
    pub fn time(&self, index: usize) -> Option<f64> {
        self.frames.get(index).map(|entry| entry.time)
    }

    /// Whether the history stopped recording.
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Bytes of the stored frames, and their size before compression.
    pub fn bytes(&self) -> (u64, u64) {
        (self.stored, self.raw)
    }

    /// Where the frames are kept.
    pub fn describe(&self) -> &'static str {
        self.store.describe()
    }

    /// Asks for a frame; [`History::poll`] returns it once it is decoded. A newer request
    /// replaces one that has not started.
    pub fn request(&mut self, index: usize) {
        self.wanted = Some(index);
        self.start_read();
    }

    /// Whether a requested frame is still being read.
    pub fn is_loading(&self) -> bool {
        self.reading.is_some() || self.wanted.is_some()
    }

    /// The requested frame, once it is decoded (or why it could not be).
    pub fn poll(&mut self) -> Option<Result<Frame, String>> {
        self.check_writes();
        self.finish_read();
        self.ready.take()
    }

    fn start_read(&mut self) {
        if self.reading.is_some() {
            return;
        }
        let Some(target) = self.wanted.take() else {
            return;
        };
        let Some(entry) = self.frames.get(target) else {
            self.ready = Some(Err(format!("frame {target} was not recorded")));
            return;
        };
        if entry.lost {
            self.ready = Some(Err("this frame could not be stored".to_owned()));
            return;
        }
        if let Some((index, state)) = &self.decoded
            && *index == target
        {
            self.ready = Some(Ok(frame(target, state)));
            return;
        }
        let key = (0..=target)
            .rev()
            .find(|&index| self.frames[index].key)
            .unwrap_or(0);
        let from = match &self.decoded {
            Some((index, _)) if (key..target).contains(index) => index + 1,
            _ => key,
        };
        self.reading = Some((from, target));
        self.store.read(Range {
            start: from,
            end: target + 1,
        });
        self.finish_read();
    }

    fn finish_read(&mut self) {
        let Some((from, target)) = self.reading else {
            return;
        };
        let Some(result) = self.store.take_read() else {
            return;
        };
        self.reading = None;
        let decoded = result.and_then(|frames| {
            let mut state = match self.decoded.take() {
                Some((index, state)) if index + 1 == from => Some(state),
                _ => None,
            };
            for encoded in &frames {
                state = Some(codec::decode(encoded, state.as_ref())?);
            }
            state.ok_or_else(|| "no frames read".to_owned())
        });
        // Only the latest request is shown; an older one only serves as the base of the next.
        let shown = self.wanted.is_none();
        match decoded {
            Ok(state) => {
                if shown {
                    self.ready = Some(Ok(frame(target, &state)));
                }
                self.decoded = Some((target, state));
            }
            Err(error) => {
                tracing::error!(
                    frame = target,
                    "Could not read a frame of the history: {error}"
                );
                if shown {
                    self.ready = Some(Err(error));
                }
            }
        }
        self.start_read();
    }
}

fn frame(index: usize, state: &State) -> Frame {
    Frame {
        index,
        cells: state.planes.cell_infos(),
        plates: state.plates.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::store::MemoryStore;
    use super::*;

    fn plate(step: usize) -> PlateInfo {
        PlateInfo {
            rotation: [step as f32, 0.0, 0.0],
            id: step as u32 + 7,
        }
    }

    fn cells(count: usize, step: usize) -> Vec<CellInfo> {
        (0..count)
            .map(|index| CellInfo {
                elevation: ((index * 7 + step * 13) % 9000) as f32 - 4000.0,
                crust_thickness: 30_000.0,
                age: step as f32,
                sediment: 0.0,
                orogenic: 0.0,
                since_orogeny: f32::NAN,
                strength_thermal: 1.0,
                strength_thinning: 1.0,
                temperature: 10.0 + step as f32 * 0.1,
                temperature_january: 0.0,
                temperature_july: 20.0,
                precipitation: 1.0,
                precipitation_january: 0.5,
                precipitation_july: 1.5,
                potential_evaporation: 0.8,
                evaporation: 0.7,
                evaporation_january: 0.4,
                evaporation_july: 1.0,
                ice: 0.0,
                discharge: 0.0,
                current_east: 1.0,
                current_north: -2.0,
                streamfunction: 3.0,
                gyre_heating: 4.0,
                wind_east: -5.0,
                wind_north: 0.5,
                wind_january_east: -6.0,
                wind_january_north: 1.0,
                wind_july_east: -4.0,
                wind_july_north: -0.5,
                climate_class: (index % 31) as u32,
                plate: (index % 3) as u32,
                flags: 0,
            })
            .collect()
    }

    /// The cells as the history gives them back, as bytes (NaN is not equal to itself).
    fn quantised(cells: &[CellInfo]) -> Vec<u8> {
        bytes(&Planes::new(cells).cell_infos())
    }

    fn bytes(cells: &[CellInfo]) -> Vec<u8> {
        bytemuck::cast_slice(cells).to_vec()
    }

    fn history(steps: usize, capacity: u64) -> History {
        let mut history = History::new(Box::new(MemoryStore::new(capacity)));
        for step in 0..steps {
            history.record(step as f64, &cells(100, step), &[plate(step)]);
        }
        history
    }

    fn show(history: &mut History, index: usize) -> Frame {
        history.request(index);
        history
            .poll()
            .expect("memory reads complete at once")
            .unwrap()
    }

    #[test]
    fn every_frame_can_be_shown_from_any_other() {
        let steps = 2 * KEY_INTERVAL + 7;
        let mut history = history(steps, u64::MAX);
        assert_eq!(history.len(), steps);
        let order = [
            0,
            steps - 1,
            3,
            4,
            2,
            KEY_INTERVAL,
            KEY_INTERVAL - 1,
            60,
            61,
            120,
            5,
        ];
        for index in order.into_iter().filter(|&index| index < steps) {
            let frame = show(&mut history, index);
            assert_eq!(frame.index, index);
            assert_eq!(bytes(&frame.cells), quantised(&cells(100, index)));
            assert_eq!(frame.plates, [plate(index)]);
        }
        let keys = history.frames.iter().filter(|entry| entry.key).count();
        assert_eq!(keys, steps.div_ceil(KEY_INTERVAL));
        let (stored, raw) = history.bytes();
        assert!(stored < raw, "{stored} of {raw}");
    }

    #[test]
    fn repeated_states_are_recorded_once() {
        let mut history = history(3, u64::MAX);
        assert!(!history.record(2.0, &cells(100, 2), &[]));
        assert!(!history.record(1.0, &cells(100, 1), &[]));
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn a_full_history_stops_recording_and_keeps_its_frames() {
        let mut history = history(200, 3_000);
        assert!(history.is_full());
        let recorded = history.len();
        assert!(recorded > 1 && recorded < 200, "{recorded}");
        assert!(history.bytes().0 <= 3_000);
        assert!(!history.record(1000.0, &cells(100, 0), &[]));
        let last = show(&mut history, recorded - 1);
        assert_eq!(last.index, recorded - 1);
        assert_eq!(history.time(recorded - 1), Some((recorded - 1) as f64));
    }

    #[test]
    fn only_the_latest_request_is_shown() {
        use std::cell::RefCell;
        use std::rc::Rc;

        /// Reads of a store that complete when the test says so.
        #[derive(Default)]
        struct Reads {
            pending: Option<Range<usize>>,
            done: Option<Result<Vec<Vec<u8>>, String>>,
            count: usize,
        }
        struct Slow(MemoryStore, Rc<RefCell<Reads>>);
        impl FrameStore for Slow {
            fn append(&mut self, index: usize, frame: Vec<u8>) {
                self.0.append(index, frame);
            }
            fn read(&mut self, range: Range<usize>) {
                let mut reads = self.1.borrow_mut();
                reads.count += 1;
                // The data as it is now; handed out when the test completes the read.
                self.0.read(range.clone());
                reads.done = self.0.take_read();
                reads.pending = Some(range);
            }
            fn take_read(&mut self) -> Option<Result<Vec<Vec<u8>>, String>> {
                let mut reads = self.1.borrow_mut();
                if reads.pending.is_some() {
                    return None;
                }
                reads.done.take()
            }
            fn failed_write(&mut self) -> Option<usize> {
                None
            }
            fn capacity(&self) -> u64 {
                u64::MAX
            }
            fn describe(&self) -> &'static str {
                "slow"
            }
        }
        let reads = Rc::new(RefCell::new(Reads::default()));
        let mut history = History::new(Box::new(Slow(MemoryStore::new(u64::MAX), reads.clone())));
        for step in 0..10 {
            history.record(step as f64, &cells(50, step), &[]);
        }
        let complete = || {
            assert!(
                reads.borrow_mut().pending.take().is_some(),
                "a read is pending"
            );
        };
        history.request(5);
        assert!(history.poll().is_none());
        // Scrubbing on while the read runs: only the last of these is shown.
        history.request(2);
        history.request(7);
        assert!(history.is_loading());
        complete();
        // Frame 5 arrived; 7 continues from it with one more read.
        assert!(history.poll().is_none());
        complete();
        let frame = history.poll().unwrap().unwrap();
        assert_eq!(frame.index, 7);
        assert_eq!(bytes(&frame.cells), quantised(&cells(50, 7)));
        assert_eq!(reads.borrow().count, 2);
        assert!(!history.is_loading());
    }

    #[test]
    fn a_failed_write_loses_the_frames_from_it_on() {
        struct Failing(MemoryStore);
        impl FrameStore for Failing {
            fn append(&mut self, index: usize, frame: Vec<u8>) {
                self.0.append(index, frame);
            }
            fn read(&mut self, range: Range<usize>) {
                self.0.read(range);
            }
            fn take_read(&mut self) -> Option<Result<Vec<Vec<u8>>, String>> {
                self.0.take_read()
            }
            fn failed_write(&mut self) -> Option<usize> {
                // The write of frame 4 failed, reported after more were appended.
                (self.0.frames.len() > 6).then_some(4)
            }
            fn capacity(&self) -> u64 {
                u64::MAX
            }
            fn describe(&self) -> &'static str {
                "failing"
            }
        }
        let mut history = History::new(Box::new(Failing(MemoryStore::new(u64::MAX))));
        for step in 0..10 {
            history.record(step as f64, &cells(50, step), &[]);
        }
        assert!(history.is_full());
        assert_eq!(history.len(), 7);
        assert_eq!(show(&mut history, 3).index, 3);
        history.request(5);
        assert!(history.poll().unwrap().is_err());
    }
}
