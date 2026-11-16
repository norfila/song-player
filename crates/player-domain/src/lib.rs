//! Playback rules independent of the desktop shell, storage, and audio device.
//!
//! Queue entry identities deliberately differ from track identities: the same
//! file can be queued several times. The active entry remains stable while the
//! remaining queue is sorted or shuffled. Natural end-of-file and explicit Next
//! are distinct operations because only the former observes Repeat One.

mod listening;
mod model;
mod queue;

pub use listening::ListeningEvent;
pub use model::{PlayOrder, QueueEntry, RepeatMode, SessionSnapshot, Track};
pub use queue::Session;
