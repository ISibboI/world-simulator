//! The frame history of a run: every state the view received, compact, so that the time bar
//! ([`super::timeline`]) can show any past one.
//!
//! A frame keeps what the view needs to rebuild everything it shows: the attributes of the
//! cells, quantised ([`codec`], 55 bytes per cell), and the angular velocity of every plate, for
//! the layers derived from plates. The vertices follow from the cells. Frames are stored as the
//! difference to the frame before (mostly zeros), compressed with lz4, with a keyframe every
//! [`KEY_INTERVAL`] frames, so that showing a frame decodes at most that many. A [`FrameStore`]
//! keeps the encoded frames: a temporary file on the desktop, IndexedDB on the web. In memory stay
//! only a few decoded states of one key interval, checkpoints every [`CHECKPOINT`] frames, so
//! that a step back decodes a few frames rather than the chain from the keyframe (#121) (and in
//! the encoder the state of the latest frame, the base of the next difference).
//!
//! While the time bar is dragged faster than frames are read (IndexedDB), every decoded frame is
//! shown, not only the last requested one, so that the view keeps up (#121).
//!
//! The simulation encodes the frames ([`FrameEncoder`], in its thread on the desktop and its
//! worker on the web), and the view only stores them, so that the encoding does not take time
//! from the view (#174).
//!
//! When the store is full (its capacity, or a failed write such as an exhausted browser quota),
//! the history stops recording: the frames it has stay viewable.

mod codec;
pub mod store;

use std::ops::Range;

use super::{CellInfo, PlateInfo};
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

/// Encodes the states of a run into the frames of its history, where the states are made: in
/// the simulation's thread (desktop) or worker (web), so that the view only stores the frames
/// (#174). Each frame is the difference to the one before, every [`KEY_INTERVAL`]th one a
/// keyframe; the view must record every frame it is given, in order.
#[derive(Default)]
pub struct FrameEncoder {
    /// The state of the last encoded frame, the base of the next delta frame.
    latest: Option<State>,
    /// Frames since the last keyframe.
    since_key: usize,
}

impl FrameEncoder {
    /// The frame of a state, or `None` if it is not later than the last encoded one (the
    /// simulation sends a state again when the view asks for other fields).
    pub fn encode(
        &mut self,
        time: f64,
        cells: &[CellInfo],
        plates: &[PlateInfo],
    ) -> Option<Vec<u8>> {
        if self
            .latest
            .as_ref()
            .is_some_and(|latest| time <= latest.time)
        {
            return None;
        }
        let state = State {
            time,
            planes: Planes::new(cells),
            plates: plates.to_vec(),
        };
        let key = self.since_key + 1 >= KEY_INTERVAL;
        let previous = if key { None } else { self.latest.as_ref() };
        let frame = codec::encode(&state, previous);
        self.since_key = if codec::kind(&frame).unwrap_or(Kind::Key) == Kind::Key {
            0
        } else {
            self.since_key + 1
        };
        self.latest = Some(state);
        Some(frame)
    }

    /// Makes the next frame a keyframe: after a frame that may not have reached the view, so
    /// that the frames after it do not depend on it.
    // Only the web worker can fail to post a frame; the desktop's simulation ends instead.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn force_keyframe(&mut self) {
        self.since_key = KEY_INTERVAL;
    }
}

/// The frames of one run.
pub struct History {
    store: Box<dyn FrameStore>,
    frames: Vec<Entry>,
    /// Bytes of the encoded frames, and their size before compression.
    stored: u64,
    raw: u64,
    /// The history no longer records (see the module documentation).
    full: bool,
    /// Decoded states of one key interval, by frame: every [`CHECKPOINT`]th frame from its
    /// keyframe on, and the last decoded frame. A frame decodes from the nearest of them before
    /// it, so that a step back reads and decodes a few frames, not the chain from the keyframe
    /// (#121).
    checkpoints: Vec<(usize, State)>,
    /// The read in progress.
    reading: Option<Read>,
    /// The read in progress is no longer of interest (the view went live).
    stale: bool,
    /// The frame to show next, while a read is in progress.
    wanted: Option<usize>,
    /// The result for the view.
    ready: Option<Result<Frame, String>>,
}

/// Every this many frames after a keyframe, the decoded state is kept as a checkpoint.
const CHECKPOINT: usize = 5;
/// At most this many bytes of checkpoints: a guard for large meshes (at 6 subdivisions the
/// at most 6 checkpoints take ~14 MB).
const CHECKPOINT_BYTES: usize = 32 << 20;

/// A read of frames from the store.
#[derive(Debug, Clone, Copy)]
struct Read {
    /// The first frame read, the last (the requested one) and the keyframe before it.
    from: usize,
    target: usize,
    key: usize,
    /// The checkpoint the first frame decodes on (`from - 1`), if it is not a keyframe.
    base: Option<usize>,
    started: web_time::Instant,
}

impl History {
    pub fn new(store: Box<dyn FrameStore>) -> Self {
        Self {
            store,
            frames: Vec::new(),
            stored: 0,
            raw: 0,
            full: false,
            checkpoints: Vec::new(),
            reading: None,
            stale: false,
            wanted: None,
            ready: None,
        }
    }

    /// Records a frame of the simulation's [`FrameEncoder`], if it is later than the last
    /// recorded one and the store has room. Returns whether it was recorded.
    pub fn record(&mut self, frame: Vec<u8>) -> bool {
        self.check_writes();
        let (kind, time, raw) = match codec::header(&frame) {
            Ok(header) => header,
            Err(error) => {
                tracing::error!("A frame for the history refused: {error}");
                return false;
            }
        };
        if self.full || self.frames.last().is_some_and(|entry| time <= entry.time) {
            return false;
        }
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
            raw,
            encoded = frame.len(),
            "Frame recorded"
        );
        self.stored += frame.len() as u64;
        self.raw += raw as u64;
        self.store.append(index, frame);
        self.frames.push(Entry {
            time,
            key: kind == Kind::Key,
            lost: false,
        });
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
            self.checkpoints.retain(|(index, _)| *index < failed);
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
        if self.reading.is_some_and(|read| read.target == index) {
            // On its way.
            self.wanted = None;
            self.stale = false;
            return;
        }
        self.wanted = Some(index);
        self.start_read();
    }

    /// Forgets the requests: the view no longer shows past frames. A read in progress
    /// completes, only as a checkpoint.
    pub fn cancel(&mut self) {
        self.wanted = None;
        self.ready = None;
        self.stale = self.reading.is_some();
    }

    /// Whether a requested frame is still being read.
    pub fn is_loading(&self) -> bool {
        (self.reading.is_some() && !self.stale) || self.wanted.is_some()
    }

    /// The decoded frame, once it is there (or why it could not be). While the view scrubs
    /// faster than frames are read, this is also a frame that an older request asked for, so
    /// that the view does not stand still until the scrubbing stops (#121); the requested
    /// frame follows.
    pub fn poll(&mut self) -> Option<Result<Frame, String>> {
        self.check_writes();
        self.finish_read();
        self.ready.take()
    }

    /// The keyframe the frame decodes from.
    fn key_of(&self, index: usize) -> usize {
        (0..=index)
            .rev()
            .find(|&index| self.frames[index].key)
            .unwrap_or(0)
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
        let key = self.key_of(target);
        // The nearest checkpoint at or before the frame, after its keyframe.
        let base = self
            .checkpoints
            .iter()
            .rev()
            .find(|(index, _)| (key..=target).contains(index));
        if let Some((index, state)) = base
            && *index == target
        {
            self.ready = Some(Ok(frame(target, state)));
            return;
        }
        let base = base.map(|(index, _)| *index);
        let from = base.map_or(key, |index| index + 1);
        self.reading = Some(Read {
            from,
            target,
            key,
            base,
            started: web_time::Instant::now(),
        });
        self.store.read(Range {
            start: from,
            end: target + 1,
        });
        self.finish_read();
    }

    fn finish_read(&mut self) {
        let Some(read) = self.reading else {
            return;
        };
        let Some(result) = self.store.take_read() else {
            return;
        };
        let started = web_time::Instant::now();
        self.reading = None;
        let stale = std::mem::take(&mut self.stale);
        let decoded = result.and_then(|frames| self.decode(read, &frames));
        tracing::debug!(
            from = read.from,
            target = read.target,
            newer = self.wanted.is_some(),
            read_ms = (started - read.started).as_secs_f64() * 1e3,
            decode_ms = started.elapsed().as_secs_f64() * 1e3,
            "History frames read and decoded"
        );
        match decoded {
            // Shown even if a newer request waits (see `poll`).
            Ok(state) if !stale => self.ready = Some(Ok(frame(read.target, &state))),
            Ok(_) => {}
            Err(error) => {
                tracing::error!(
                    frame = read.target,
                    "Could not read a frame of the history: {error}"
                );
                // Only the latest request's error is of interest.
                if !stale && self.wanted.is_none() {
                    self.ready = Some(Err(error));
                }
            }
        }
        self.start_read();
    }

    /// Decodes the frames of a read, keeps its checkpoints and returns the state of its last
    /// frame.
    fn decode(&mut self, read: Read, frames: &[Vec<u8>]) -> Result<State, String> {
        if frames.len() != read.target + 1 - read.from {
            return Err("frames missing".to_owned());
        }
        let base = match read.base {
            Some(base) => Some(
                self.checkpoints
                    .iter()
                    .find(|(index, _)| *index == base)
                    .map(|(_, state)| state)
                    .ok_or("the frame before it is gone")?,
            ),
            None => None,
        };
        let mut kept = Vec::new();
        let mut state: Option<State> = None;
        for (index, encoded) in (read.from..).zip(frames) {
            let decoded = codec::decode(encoded, state.as_ref().or(base))?;
            if (index - read.key).is_multiple_of(CHECKPOINT) && index != read.target {
                kept.push((index, decoded.clone()));
            }
            state = Some(decoded);
        }
        let state = state.ok_or("no frames read")?;
        // Only the checkpoints of this key interval stay, and of the last decoded frames only
        // this one.
        let key = read.key;
        let next_key = (key + 1..self.frames.len())
            .find(|&index| self.frames[index].key)
            .unwrap_or(usize::MAX);
        self.checkpoints.retain(|(index, _)| {
            (key..next_key).contains(index) && (index - key).is_multiple_of(CHECKPOINT)
        });
        self.checkpoints.extend(kept);
        self.checkpoints.push((read.target, state.clone()));
        self.checkpoints.sort_by_key(|(index, _)| *index);
        self.checkpoints.dedup_by_key(|(index, _)| *index);
        // The guard for large meshes: the farthest from this frame go first.
        let size = |state: &State| state.planes.size() + state.plates.len() * codec::PLATE_BYTES;
        while self.checkpoints.len() > 1
            && self
                .checkpoints
                .iter()
                .map(|(_, state)| size(state))
                .sum::<usize>()
                > CHECKPOINT_BYTES
        {
            let farthest = (0..self.checkpoints.len())
                .max_by_key(|&at| self.checkpoints[at].0.abs_diff(read.target))
                .expect("checkpoints");
            self.checkpoints.remove(farthest);
        }
        Ok(state)
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
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::store::MemoryStore;
    use super::*;

    fn plate(step: usize) -> PlateInfo {
        PlateInfo {
            rotation: [step as f32, 0.0, 0.0],
            id: step as u32 + 7,
            orientation: [
                0.0,
                0.0,
                (step as f32 * 0.01).sin(),
                (step as f32 * 0.01).cos(),
            ],
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
                lake_depth: 0.0,
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
                drainage: CellInfo::NO_RECEIVER,
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

    /// Encodes the states of `steps` steps of `count` cells and records them.
    fn record(history: &mut History, steps: usize, count: usize, plates: bool) {
        let mut encoder = FrameEncoder::default();
        for step in 0..steps {
            let plates = if plates {
                vec![plate(step)]
            } else {
                Vec::new()
            };
            let frame = encoder
                .encode(step as f64, &cells(count, step), &plates)
                .expect("a later state");
            history.record(frame);
        }
    }

    fn history(steps: usize, capacity: u64) -> History {
        let mut history = History::new(Box::new(MemoryStore::new(capacity)));
        record(&mut history, steps, 100, true);
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
    fn a_forced_keyframe_restarts_the_chain() {
        let mut encoder = FrameEncoder::default();
        let first = encoder.encode(0.0, &cells(100, 0), &[]).unwrap();
        // Lost on the way to the view.
        let _lost = encoder.encode(1.0, &cells(100, 1), &[]).unwrap();
        encoder.force_keyframe();
        let next = encoder.encode(2.0, &cells(100, 2), &[]).unwrap();
        assert_eq!(codec::kind(&next), Ok(Kind::Key));
        assert_eq!(
            codec::kind(&encoder.encode(3.0, &cells(100, 3), &[]).unwrap()),
            Ok(Kind::Delta)
        );
        let mut history = History::new(Box::new(MemoryStore::new(u64::MAX)));
        assert!(history.record(first));
        assert!(history.record(next));
        assert_eq!(
            bytes(&show(&mut history, 1).cells),
            quantised(&cells(100, 2))
        );
    }

    #[test]
    fn repeated_states_are_recorded_once() {
        let mut encoder = FrameEncoder::default();
        let frames: Vec<Vec<u8>> = (0..3)
            .map(|step| encoder.encode(step as f64, &cells(100, step), &[]).unwrap())
            .collect();
        // A state sent again (with other fields) is not encoded again.
        assert!(encoder.encode(2.0, &cells(100, 2), &[]).is_none());
        assert!(encoder.encode(1.0, &cells(100, 1), &[]).is_none());
        let mut history = History::new(Box::new(MemoryStore::new(u64::MAX)));
        for frame in &frames {
            assert!(history.record(frame.clone()));
        }
        // Nor recorded again.
        assert!(!history.record(frames[2].clone()));
        assert!(!history.record(frames[1].clone()));
        assert!(!history.record(vec![0; 3]));
        assert_eq!(history.len(), 3);
        assert_eq!(
            bytes(&show(&mut history, 2).cells),
            quantised(&cells(100, 2))
        );
    }

    #[test]
    fn a_full_history_stops_recording_and_keeps_its_frames() {
        let mut history = history(200, 3_000);
        assert!(history.is_full());
        let recorded = history.len();
        assert!(recorded > 1 && recorded < 200, "{recorded}");
        assert!(history.bytes().0 <= 3_000);
        let mut encoder = FrameEncoder::default();
        let frame = encoder.encode(1000.0, &cells(100, 0), &[]).unwrap();
        assert!(!history.record(frame));
        let last = show(&mut history, recorded - 1);
        assert_eq!(last.index, recorded - 1);
        assert_eq!(history.time(recorded - 1), Some((recorded - 1) as f64));
    }

    /// Reads of a store that complete when the test says so (or at once).
    #[derive(Default)]
    struct Reads {
        instant: bool,
        pending: Option<Range<usize>>,
        done: Option<Result<Vec<Vec<u8>>, String>>,
        ranges: Vec<Range<usize>>,
    }

    struct Slow(MemoryStore, Rc<RefCell<Reads>>);

    impl FrameStore for Slow {
        fn append(&mut self, index: usize, frame: Vec<u8>) {
            self.0.append(index, frame);
        }
        fn read(&mut self, range: Range<usize>) {
            let mut reads = self.1.borrow_mut();
            reads.ranges.push(range.clone());
            // The data as it is now; handed out when the test completes the read.
            self.0.read(range.clone());
            reads.done = self.0.take_read();
            if !reads.instant {
                reads.pending = Some(range);
            }
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

    /// A history of `steps` frames of 50 cells in a [`Slow`] store.
    fn slow(steps: usize, instant: bool) -> (History, Rc<RefCell<Reads>>) {
        let reads = Rc::new(RefCell::new(Reads {
            instant,
            ..Reads::default()
        }));
        let mut history = History::new(Box::new(Slow(MemoryStore::new(u64::MAX), reads.clone())));
        record(&mut history, steps, 50, false);
        (history, reads)
    }

    fn complete(reads: &RefCell<Reads>) {
        assert!(
            reads.borrow_mut().pending.take().is_some(),
            "a read is pending"
        );
    }

    #[test]
    fn frames_of_older_requests_are_shown_while_scrubbing() {
        let (mut history, reads) = slow(10, false);
        history.request(5);
        assert!(history.poll().is_none());
        // Scrubbing on while the read runs: 5 is shown when it arrives (#121), then the last
        // request, not the ones in between.
        history.request(2);
        history.request(7);
        assert!(history.is_loading());
        complete(&reads);
        assert_eq!(history.poll().unwrap().unwrap().index, 5);
        assert!(history.is_loading());
        // 7 continues from 5 with one more read.
        complete(&reads);
        let frame = history.poll().unwrap().unwrap();
        assert_eq!(frame.index, 7);
        assert_eq!(bytes(&frame.cells), quantised(&cells(50, 7)));
        assert_eq!(reads.borrow().ranges, [0..6, 6..8]);
        assert!(!history.is_loading());
        assert!(history.poll().is_none());
    }

    #[test]
    fn a_request_for_the_frame_being_read_waits_for_it() {
        let (mut history, reads) = slow(10, false);
        history.request(5);
        history.request(7);
        history.request(5);
        complete(&reads);
        assert_eq!(history.poll().unwrap().unwrap().index, 5);
        assert!(history.poll().is_none());
        assert!(!history.is_loading());
        assert_eq!(reads.borrow().ranges, vec![0..6]);
    }

    #[test]
    fn a_read_is_not_shown_after_the_view_went_live() {
        let (mut history, reads) = slow(10, false);
        history.request(5);
        history.cancel();
        assert!(!history.is_loading());
        complete(&reads);
        assert!(history.poll().is_none());
        // Back in the past: the read of 5 served as a checkpoint.
        history.request(8);
        complete(&reads);
        assert_eq!(history.poll().unwrap().unwrap().index, 8);
        assert_eq!(reads.borrow().ranges, [0..6, 6..9]);
        // Also when the view leaves live again before the read arrives.
        history.request(3);
        history.cancel();
        history.request(9);
        complete(&reads);
        assert!(history.poll().is_none());
        complete(&reads);
        assert_eq!(history.poll().unwrap().unwrap().index, 9);
        assert!(history.poll().is_none());
    }

    #[test]
    fn stepping_back_reads_a_few_frames() {
        // #121: deltas only go forward from a keyframe; checkpoints keep steps back short.
        let steps = 3 * KEY_INTERVAL;
        let (mut history, reads) = slow(steps, true);
        let check = |history: &mut History, index: usize| {
            let frame = show(history, index);
            assert_eq!(frame.index, index);
            assert_eq!(bytes(&frame.cells), quantised(&cells(50, index)));
            assert_eq!(frame.plates, []);
        };
        check(&mut history, steps - 1);
        reads.borrow_mut().ranges.clear();
        for index in (0..steps - 1).rev() {
            check(&mut history, index);
        }
        let ranges = std::mem::take(&mut reads.borrow_mut().ranges);
        // The first step into an earlier key interval reads it from its keyframe; the others
        // at most to the checkpoint before them.
        for range in &ranges {
            let crosses = range.end % KEY_INTERVAL == 0;
            assert!(
                crosses || range.len() < CHECKPOINT,
                "{range:?} of {ranges:?}"
            );
        }
        let frames: usize = ranges.iter().map(ExactSizeIterator::len).sum();
        assert!(frames < 3 * steps, "{frames} frames read");
        assert!(history.checkpoints.len() <= KEY_INTERVAL.div_ceil(CHECKPOINT) + 1);
        // Forward again: one frame per step.
        for index in 1..steps {
            check(&mut history, index);
        }
        let ranges = std::mem::take(&mut reads.borrow_mut().ranges);
        assert!(ranges.iter().all(|range| range.len() == 1), "{ranges:?}");
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
        record(&mut history, 10, 50, false);
        assert!(history.is_full());
        assert_eq!(history.len(), 7);
        assert_eq!(show(&mut history, 3).index, 3);
        history.request(5);
        assert!(history.poll().unwrap().is_err());
    }
}
