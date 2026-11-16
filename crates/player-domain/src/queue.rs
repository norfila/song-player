use crate::{PlayOrder, QueueEntry, RepeatMode, SessionSnapshot, Track};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

const HISTORY_LIMIT: usize = 2_000;

/// A persistable listening session. `entries` preserves the manual base order;
/// `upcoming` is the actual next-play order and never contains the current entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub position: f64,
    #[serde(default)]
    pub repeat: RepeatMode,
    #[serde(default)]
    pub order_mode: PlayOrder,
    /// Sorting to restore when random playback is disabled. Older sessions did
    /// not remember it, so their shuffled queues fall back to manual order.
    #[serde(default)]
    unshuffled_order: PlayOrder,
    #[serde(default)]
    entries: Vec<QueueEntry>,
    #[serde(default)]
    current_entry_id: Option<String>,
    #[serde(default)]
    upcoming: Vec<String>,
    #[serde(default)]
    history: Vec<String>,
    #[serde(default)]
    forward_history: Vec<String>,
    #[serde(default)]
    priority: Vec<String>,
    #[serde(default)]
    shuffle_seed: u64,
}

impl Session {
    pub fn new(name: impl Into<String>) -> Self {
        let uuid = Uuid::new_v4();
        let mut seed_bytes = [0; 8];
        seed_bytes.copy_from_slice(&uuid.as_bytes()[..8]);
        Self {
            id: uuid.to_string(),
            name: name.into(),
            position: 0.0,
            repeat: RepeatMode::Off,
            order_mode: PlayOrder::Manual,
            unshuffled_order: PlayOrder::Manual,
            entries: Vec::new(),
            current_entry_id: None,
            upcoming: Vec::new(),
            history: Vec::new(),
            forward_history: Vec::new(),
            priority: Vec::new(),
            shuffle_seed: u64::from_le_bytes(seed_bytes),
        }
    }

    /// The snapshot includes every queue entry exactly once. Past entries precede
    /// the current entry; the remainder follows the order in which Next plays it.
    pub fn snapshot(&self) -> SessionSnapshot {
        let mut ids = Vec::with_capacity(self.entries.len());
        let future: HashSet<&str> = self
            .upcoming
            .iter()
            .chain(self.forward_history.iter())
            .map(String::as_str)
            .collect();
        // Keep the last occurrence of an entry in the played history. Repeat All
        // may have recorded earlier passes through that same queue identity.
        let mut seen_past = HashSet::new();
        let mut past: Vec<&String> = self
            .history
            .iter()
            .rev()
            .filter(|id| {
                Some(id.as_str()) != self.current_entry_id.as_deref()
                    && !future.contains(id.as_str())
                    && seen_past.insert(id.as_str())
            })
            .collect();
        past.reverse();
        ids.extend(past);
        // Explicitly selecting a later row skips the intervening entries. They
        // still belong to the queue, but must appear before the current row so
        // that the displayed tail remains identical to the upcoming playback.
        ids.extend(
            self.entries
                .iter()
                .filter(|entry| {
                    Some(entry.id.as_str()) != self.current_entry_id.as_deref()
                        && !future.contains(entry.id.as_str())
                })
                .map(|entry| &entry.id),
        );
        ids.extend(self.current_entry_id.iter());
        // Explicit Play Next always takes precedence over history navigation.
        ids.extend(self.priority.iter().filter(|id| self.upcoming.contains(id)));
        ids.extend(self.forward_history.iter().rev());
        ids.extend(self.upcoming.iter());

        let mut seen = HashSet::new();
        let by_id: HashMap<&str, &QueueEntry> = self
            .entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect();
        let entries = ids
            .into_iter()
            .filter(|id| seen.insert(id.as_str()))
            .filter_map(|id| by_id.get(id.as_str()).map(|entry| (*entry).clone()))
            .collect();
        SessionSnapshot {
            id: self.id.clone(),
            name: self.name.clone(),
            entries,
            current_entry_id: self.current_entry_id.clone(),
            position: self.position,
            repeat: self.repeat,
            order_mode: self.order_mode,
        }
    }

    pub fn current_entry(&self) -> Option<&QueueEntry> {
        self.current_entry_id
            .as_deref()
            .and_then(|id| self.entry(id))
    }

    /// Refresh cached metadata without changing queue entry identities/order.
    pub fn refresh_track(&mut self, track: &Track) {
        for entry in &mut self.entries {
            if entry.track.id == track.id {
                entry.track = track.clone();
            }
        }
    }

    pub fn has_upcoming(&self) -> bool {
        !self.upcoming.is_empty() || !self.forward_history.is_empty()
    }

    pub fn append(&mut self, tracks: Vec<Track>) {
        if tracks.is_empty() {
            return;
        }
        let new_entries = tracks.into_iter().map(make_entry).collect::<Vec<_>>();
        self.upcoming
            .extend(new_entries.iter().map(|entry| entry.id.clone()));
        self.entries.extend(new_entries);
        self.reorder_upcoming();
    }

    /// Adds a batch in its supplied order after other explicitly queued-next
    /// batches. Re-sorting or shuffling does not remove this user priority.
    pub fn enqueue_next(&mut self, tracks: Vec<Track>) {
        if tracks.is_empty() {
            return;
        }
        let new_entries = tracks.into_iter().map(make_entry).collect::<Vec<_>>();
        let ids = new_entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>();
        self.entries.extend(new_entries);
        self.upcoming.extend(ids.iter().cloned());
        self.priority.extend(ids);
        self.reorder_upcoming();
    }

    pub fn replace(&mut self, tracks: Vec<Track>) {
        self.clear();
        self.append(tracks);
    }

    /// Selects an entry explicitly and starts the remainder at that entry's
    /// displayed position. This makes double-clicking a past row useful as well.
    pub fn play_entry(&mut self, id: &str) -> bool {
        if self.entry(id).is_none() {
            return false;
        }
        if self.current_entry_id.as_deref() == Some(id) {
            self.position = 0.0;
            return true;
        }
        let displayed = self.snapshot().entries;
        if let Some(current) = self.current_entry_id.take() {
            self.push_history(current);
        }
        self.current_entry_id = Some(id.to_owned());
        self.position = 0.0;
        self.forward_history.clear();
        let after = displayed.iter().position(|entry| entry.id == id).unwrap();
        self.upcoming = displayed[after + 1..]
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        self.priority.retain(|id| self.upcoming.contains(id));
        true
    }

    /// Advances playback. Pass `false` for a natural end of file and `true` for
    /// the user's Next command (which bypasses Repeat One).
    pub fn next(&mut self, manual: bool) -> Option<QueueEntry> {
        if !manual && self.repeat == RepeatMode::One && self.current_entry().is_some() {
            self.position = 0.0;
            return self.current_entry().cloned();
        }

        let priority_id = self
            .priority
            .iter()
            .find(|id| self.upcoming.contains(id))
            .cloned();
        let next_id = if let Some(id) = priority_id {
            self.priority.retain(|other| other != &id);
            self.upcoming.retain(|other| other != &id);
            Some(id)
        } else if let Some(id) = self.forward_history.pop() {
            self.upcoming.retain(|other| other != &id);
            Some(id)
        } else if !self.upcoming.is_empty() {
            Some(self.upcoming.remove(0))
        } else if self.repeat == RepeatMode::All && !self.entries.is_empty() {
            self.begin_new_pass();
            (!self.upcoming.is_empty()).then(|| self.upcoming.remove(0))
        } else {
            None
        };

        if let Some(current) = self.current_entry_id.take() {
            self.push_history(current);
        }
        self.current_entry_id = next_id;
        self.position = 0.0;
        self.current_entry().cloned()
    }

    pub fn previous(&mut self) -> Option<QueueEntry> {
        // At the beginning, Previous restarts the current entry without losing it.
        let Some(previous) = self.history.pop() else {
            self.position = 0.0;
            return self.current_entry().cloned();
        };
        if let Some(current) = self.current_entry_id.take() {
            self.forward_history.push(current);
        }
        self.upcoming.retain(|id| id != &previous);
        self.priority.retain(|id| id != &previous);
        self.current_entry_id = Some(previous);
        self.position = 0.0;
        self.current_entry().cloned()
    }

    pub fn remove(&mut self, id: &str) {
        let was_current = self.current_entry_id.as_deref() == Some(id);
        self.entries.retain(|entry| entry.id != id);
        self.upcoming.retain(|entry_id| entry_id != id);
        self.priority.retain(|entry_id| entry_id != id);
        self.history.retain(|entry_id| entry_id != id);
        self.forward_history.retain(|entry_id| entry_id != id);
        if was_current {
            self.current_entry_id = None;
            self.position = 0.0;
            self.next(true);
        }
    }

    /// Reorders the manual base queue and the unplayed remainder. Entries already
    /// heard remain in the playback history. Moving before `None` means append.
    pub fn move_entry(&mut self, id: &str, before: Option<&str>) {
        if before == Some(id) || before.is_some_and(|other| self.entry(other).is_none()) {
            return;
        }
        let Some(from) = self.entries.iter().position(|entry| entry.id == id) else {
            return;
        };
        let entry = self.entries.remove(from);
        let destination = before
            .and_then(|other| self.entries.iter().position(|entry| entry.id == other))
            .unwrap_or(self.entries.len());
        self.entries.insert(destination, entry);
        self.order_mode = PlayOrder::Manual;
        self.unshuffled_order = PlayOrder::Manual;
        // Dragging a prioritized row explicitly changes its position.
        self.priority.retain(|entry_id| entry_id != id);
        self.reorder_upcoming();
    }

    pub fn set_order(&mut self, order: PlayOrder) {
        if !is_shuffle(order) {
            self.unshuffled_order = order;
        } else if !is_shuffle(self.order_mode) {
            self.unshuffled_order = self.order_mode;
        }
        self.order_mode = order;
        self.reorder_upcoming();
    }

    /// Random playback only changes the upcoming entries. Disabling it restores
    /// the queue's preceding sort without changing position or playback history.
    pub fn set_shuffle(&mut self, enabled: bool) {
        if enabled {
            if self.order_mode != PlayOrder::ShuffleTracks {
                self.set_order(PlayOrder::ShuffleTracks);
            }
        } else if is_shuffle(self.order_mode) {
            let order = if is_shuffle(self.unshuffled_order) {
                PlayOrder::Manual
            } else {
                self.unshuffled_order
            };
            self.set_order(order);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_entry_id = None;
        self.upcoming.clear();
        self.history.clear();
        self.forward_history.clear();
        self.priority.clear();
        self.position = 0.0;
    }

    /// Cleans references on restore, including partially written older sessions.
    /// This never creates an automatic playing state; the application controls it.
    pub fn sanitize(&mut self) {
        if is_shuffle(self.unshuffled_order) {
            self.unshuffled_order = PlayOrder::Manual;
        }
        let mut seen_entries = HashSet::new();
        self.entries
            .retain(|entry| seen_entries.insert(entry.id.clone()));
        let valid: HashSet<&str> = self.entries.iter().map(|entry| entry.id.as_str()).collect();
        if !self
            .current_entry_id
            .as_deref()
            .is_some_and(|id| valid.contains(id))
        {
            self.current_entry_id = None;
            self.position = 0.0;
        }
        if !self.position.is_finite() || self.position < 0.0 {
            self.position = 0.0;
        }
        let mut seen_upcoming = HashSet::new();
        self.upcoming.retain(|id| {
            valid.contains(id.as_str())
                && Some(id.as_str()) != self.current_entry_id.as_deref()
                && seen_upcoming.insert(id.clone())
        });
        let mut seen_priority = HashSet::new();
        self.priority
            .retain(|id| self.upcoming.contains(id) && seen_priority.insert(id.clone()));
        self.history.retain(|id| valid.contains(id.as_str()));
        self.forward_history
            .retain(|id| valid.contains(id.as_str()));
        if self.history.len() > HISTORY_LIMIT {
            self.history.drain(..self.history.len() - HISTORY_LIMIT);
        }
    }

    fn entry(&self, id: &str) -> Option<&QueueEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn push_history(&mut self, id: String) {
        self.history.push(id);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
    }

    fn begin_new_pass(&mut self) {
        let previous = self.current_entry_id.clone();
        self.upcoming = self.entries.iter().map(|entry| entry.id.clone()).collect();
        self.priority.clear();
        self.forward_history.clear();
        self.reorder_upcoming();
        if matches!(
            self.order_mode,
            PlayOrder::ShuffleTracks | PlayOrder::ShuffleAlbums | PlayOrder::ShuffleFolders
        ) && self.upcoming.len() > 1
            && self.upcoming.first() == previous.as_ref()
        {
            // Rotate whole groups for grouped shuffle, preserving their order.
            let first_group = self
                .upcoming
                .first()
                .and_then(|id| self.entry(id))
                .map(|entry| self.group_key(&entry.track));
            let pivot = match self.order_mode {
                PlayOrder::ShuffleTracks => Some(1),
                _ => self.upcoming.iter().position(|id| {
                    self.entry(id).map(|entry| self.group_key(&entry.track)) != first_group
                }),
            };
            // A single album/folder cannot be rotated without breaking its track
            // order; it already follows the specified ordered-group behavior.
            if let Some(pivot) = pivot {
                self.upcoming.rotate_left(pivot);
            }
        }
    }

    fn reorder_upcoming(&mut self) {
        let pending: HashSet<&str> = self.upcoming.iter().map(String::as_str).collect();
        let prioritized: HashSet<&str> = self.priority.iter().map(String::as_str).collect();
        let mut remainder: Vec<&QueueEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                pending.contains(entry.id.as_str()) && !prioritized.contains(entry.id.as_str())
            })
            .collect();
        let mut random = Random(self.shuffle_seed);
        self.shuffle_seed = random.next();
        match self.order_mode {
            PlayOrder::Manual => {}
            PlayOrder::ShuffleTracks => random.shuffle(&mut remainder),
            PlayOrder::ShuffleAlbums | PlayOrder::ShuffleFolders => {
                let mut groups: BTreeMap<String, Vec<&QueueEntry>> = BTreeMap::new();
                for entry in remainder {
                    groups
                        .entry(self.group_key(&entry.track))
                        .or_default()
                        .push(entry);
                }
                let mut groups: Vec<Vec<&QueueEntry>> = groups.into_values().collect();
                for group in &mut groups {
                    group.sort_by(|a, b| compare_album_tracks(&a.track, &b.track));
                }
                random.shuffle(&mut groups);
                remainder = groups.into_iter().flatten().collect();
            }
            order => remainder.sort_by(|a, b| compare_tracks(&a.track, &b.track, order)),
        }
        let mut reordered = self
            .priority
            .iter()
            .filter(|id| pending.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        reordered.extend(remainder.into_iter().map(|entry| entry.id.clone()));
        self.upcoming = reordered;
    }

    fn group_key(&self, track: &Track) -> String {
        match self.order_mode {
            PlayOrder::ShuffleAlbums if !track.album.trim().is_empty() => format!(
                "{}\0{}",
                normalized(if track.album_artist.is_empty() {
                    &track.artist
                } else {
                    &track.album_artist
                }),
                normalized(&track.album)
            ),
            _ => folder(&track.path),
        }
    }
}

fn is_shuffle(order: PlayOrder) -> bool {
    matches!(
        order,
        PlayOrder::ShuffleTracks | PlayOrder::ShuffleAlbums | PlayOrder::ShuffleFolders
    )
}

fn make_entry(track: Track) -> QueueEntry {
    QueueEntry {
        id: Uuid::new_v4().to_string(),
        track,
    }
}

fn normalized(value: &str) -> String {
    value.trim().to_lowercase()
}

fn filename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn folder(path: &str) -> String {
    path.rsplit_once(['/', '\\'])
        .map(|(parent, _)| parent.replace('\\', "/"))
        .unwrap_or_default()
}

fn compare_album_tracks(a: &Track, b: &Track) -> Ordering {
    a.disc_number
        .unwrap_or(1)
        .cmp(&b.disc_number.unwrap_or(1))
        .then_with(|| {
            a.track_number
                .unwrap_or(u32::MAX)
                .cmp(&b.track_number.unwrap_or(u32::MAX))
        })
        .then_with(|| natural_compare(filename(&a.path), filename(&b.path)))
}

fn compare_tracks(a: &Track, b: &Track, order: PlayOrder) -> Ordering {
    let primary = match order {
        PlayOrder::Title => natural_compare(&a.title, &b.title),
        PlayOrder::Artist => natural_compare(&a.artist, &b.artist)
            .then_with(|| natural_compare(&a.album, &b.album))
            .then_with(|| compare_album_tracks(a, b)),
        PlayOrder::Album => natural_compare(&a.album, &b.album)
            .then_with(|| natural_compare(&a.album_artist, &b.album_artist))
            .then_with(|| compare_album_tracks(a, b)),
        PlayOrder::Filename => natural_compare(filename(&a.path), filename(&b.path)),
        PlayOrder::Year => a.year.unwrap_or(u32::MAX).cmp(&b.year.unwrap_or(u32::MAX)),
        PlayOrder::Duration => a.duration.total_cmp(&b.duration),
        PlayOrder::Added => a.added_at.cmp(&b.added_at),
        _ => Ordering::Equal,
    };
    primary.then_with(|| natural_compare(filename(&a.path), filename(&b.path)))
}

/// Numeric filename parts compare by value, so Track 2 precedes Track 10.
fn natural_compare(a: &str, b: &str) -> Ordering {
    let a = normalized(a);
    let b = normalized(b);
    let mut left = a.chars().peekable();
    let mut right = b.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (Some(l), Some(r)) if l.is_ascii_digit() && r.is_ascii_digit() => {
                let mut left_number = String::new();
                let mut right_number = String::new();
                while left.peek().is_some_and(char::is_ascii_digit) {
                    left_number.push(left.next().unwrap());
                }
                while right.peek().is_some_and(char::is_ascii_digit) {
                    right_number.push(right.next().unwrap());
                }
                let left_digits = left_number.trim_start_matches('0');
                let right_digits = right_number.trim_start_matches('0');
                let order = left_digits
                    .len()
                    .cmp(&right_digits.len())
                    .then_with(|| left_digits.cmp(right_digits));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(l), Some(r)) => {
                let order = l.cmp(&r);
                if order != Ordering::Equal {
                    return order;
                }
                left.next();
                right.next();
            }
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
        }
    }
}

/// SplitMix64 with Fisher–Yates: reproducible queue tests without platform RNG
/// dependencies. UUID-derived session seeds provide different listening orders.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            // Rejection sampling avoids modulo bias.
            let bound = i as u64 + 1;
            let threshold = bound.wrapping_neg() % bound;
            let mut value = self.next();
            while value < threshold {
                value = self.next();
            }
            items.swap(i, (value % bound) as usize);
        }
    }
}

#[cfg(test)]
mod tests;
