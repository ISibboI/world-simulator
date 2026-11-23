//! Where the encoded frames of the history are kept: [`FrameStore`], with a store in memory
//! ([`MemoryStore`]) and one in an anonymous temporary file (`FileStore`, desktop). The web
//! version keeps them in IndexedDB (`crate::web`).

use std::ops::Range;

/// Keeps the encoded frames of one run, appended in order, and reads ranges of them. Reading may
/// complete later (IndexedDB on the web): [`FrameStore::take_read`] returns the result once it
/// is there, and the store wakes the view then.
pub trait FrameStore {
    /// Stores frame `index`; frames come in order 0, 1, 2, … A failure is reported by
    /// [`FrameStore::failed_write`] (maybe only later).
    fn append(&mut self, index: usize, frame: Vec<u8>);

    /// Starts reading the frames of `range` (all stored before). Only one read runs at a
    /// time: the history waits for its result before it starts the next.
    fn read(&mut self, range: Range<usize>);

    /// The frames of the read, in order, once they are there.
    fn take_read(&mut self) -> Option<Result<Vec<Vec<u8>>, String>>;

    /// The first frame whose write failed, if one did.
    fn failed_write(&mut self) -> Option<usize>;

    /// Bytes of encoded frames the store may hold.
    fn capacity(&self) -> u64;

    /// Where the frames are, for the tooltip of the time bar.
    fn describe(&self) -> &'static str;
}

/// Frames in memory: the fallback if no other store is available, and for tests.
pub struct MemoryStore {
    pub(super) frames: Vec<Vec<u8>>,
    capacity: u64,
    result: Option<Result<Vec<Vec<u8>>, String>>,
}

impl MemoryStore {
    pub fn new(capacity: u64) -> Self {
        Self {
            frames: Vec::new(),
            capacity,
            result: None,
        }
    }
}

impl FrameStore for MemoryStore {
    fn append(&mut self, index: usize, frame: Vec<u8>) {
        debug_assert_eq!(index, self.frames.len());
        self.frames.push(frame);
    }

    fn read(&mut self, range: Range<usize>) {
        self.result = Some(
            self.frames
                .get(range)
                .map(<[Vec<u8>]>::to_vec)
                .ok_or_else(|| "frames not stored".to_owned()),
        );
    }

    fn take_read(&mut self) -> Option<Result<Vec<Vec<u8>>, String>> {
        self.result.take()
    }

    fn failed_write(&mut self) -> Option<usize> {
        None
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn describe(&self) -> &'static str {
        "in memory"
    }
}

/// Frames in an anonymous temporary file (desktop): unlinked at once on Unix and deleted when
/// closed on Windows, so it disappears with the run, also after a crash. Only the offsets stay
/// in memory. Reads and writes are small (a frame is a few hundred kB) and run on the view's
/// thread, so they complete at once.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileStore {
    file: std::fs::File,
    /// Start and length of every frame in the file.
    frames: Vec<(u64, usize)>,
    end: u64,
    capacity: u64,
    failed: Option<usize>,
    result: Option<Result<Vec<Vec<u8>>, String>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileStore {
    /// A store in a new temporary file, if one can be created.
    pub fn new(capacity: u64) -> std::io::Result<Self> {
        Ok(Self {
            file: tempfile::tempfile()?,
            frames: Vec::new(),
            end: 0,
            capacity,
            failed: None,
            result: None,
        })
    }

    fn read_range(&mut self, range: Range<usize>) -> std::io::Result<Vec<Vec<u8>>> {
        use std::io::{Read, Seek, SeekFrom};

        let frames = self
            .frames
            .get(range)
            .filter(|frames| !frames.is_empty())
            .ok_or_else(|| std::io::Error::other("frames not stored"))?;
        let start = frames[0].0;
        let (last_start, last_length) = frames[frames.len() - 1];
        let mut bytes = vec![0u8; (last_start - start) as usize + last_length];
        self.file.seek(SeekFrom::Start(start))?;
        self.file.read_exact(&mut bytes)?;
        Ok(frames
            .iter()
            .map(|&(offset, length)| {
                let at = (offset - start) as usize;
                bytes[at..at + length].to_vec()
            })
            .collect())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl FrameStore for FileStore {
    fn append(&mut self, index: usize, frame: Vec<u8>) {
        use std::io::{Seek, SeekFrom, Write};

        debug_assert_eq!(index, self.frames.len());
        let result = self
            .file
            .seek(SeekFrom::Start(self.end))
            .and_then(|_| self.file.write_all(&frame));
        match result {
            Ok(()) => {
                self.frames.push((self.end, frame.len()));
                self.end += frame.len() as u64;
            }
            Err(error) => {
                tracing::error!("Could not store frame {index} of the history: {error}");
                self.failed.get_or_insert(index);
                // The index stays aligned; reading this frame fails.
                self.frames.push((self.end, 0));
            }
        }
    }

    fn read(&mut self, range: Range<usize>) {
        self.result = Some(
            self.read_range(range)
                .map_err(|error| format!("reading the history: {error}")),
        );
    }

    fn take_read(&mut self) -> Option<Result<Vec<Vec<u8>>, String>> {
        self.result.take()
    }

    fn failed_write(&mut self) -> Option<usize> {
        self.failed
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn describe(&self) -> &'static str {
        "in a temporary file"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(store: &mut dyn FrameStore) {
        let frames: Vec<Vec<u8>> = (0..5u8).map(|i| vec![i; usize::from(i) * 3 + 1]).collect();
        for (index, frame) in frames.iter().enumerate() {
            store.append(index, frame.clone());
        }
        store.read(1..4);
        assert_eq!(store.take_read(), Some(Ok(frames[1..4].to_vec())));
        assert_eq!(store.take_read(), None);
        store.read(4..5);
        assert_eq!(store.take_read(), Some(Ok(frames[4..5].to_vec())));
        store.read(3..7);
        assert!(matches!(store.take_read(), Some(Err(_))));
        assert_eq!(store.failed_write(), None);
    }

    #[test]
    fn memory_store() {
        exercise(&mut MemoryStore::new(1 << 20));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_store() {
        exercise(&mut FileStore::new(1 << 20).unwrap());
    }
}
