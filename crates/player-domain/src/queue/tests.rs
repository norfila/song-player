use super::*;

fn track(number: u32) -> Track {
    Track {
        id: format!("track-{number}"),
        path: format!("C:\\Music\\Album\\{number}.flac"),
        title: format!("Track {number}"),
        artist: "Artist".into(),
        album: "Album".into(),
        track_number: Some(number),
        duration: 180.0 + number as f64,
        ..Track::default()
    }
}

fn populated(count: u32) -> Session {
    let mut session = Session::new("Test");
    session.shuffle_seed = 42;
    session.append((1..=count).map(track).collect());
    session
}

fn drain(session: &mut Session) -> Vec<QueueEntry> {
    let mut result = Vec::new();
    while let Some(entry) = session.next(true) {
        result.push(entry);
    }
    result
}

#[test]
fn manual_queue_finishes_and_previous_reopens_actual_history() {
    let mut session = populated(3);
    assert_eq!(session.next(true).unwrap().track.id, "track-1");
    assert_eq!(session.next(true).unwrap().track.id, "track-2");
    assert_eq!(session.previous().unwrap().track.id, "track-1");
    assert_eq!(session.next(true).unwrap().track.id, "track-2");
    assert_eq!(session.next(true).unwrap().track.id, "track-3");
    assert!(session.next(false).is_none());
    assert!(session.current_entry().is_none());
    assert_eq!(session.previous().unwrap().track.id, "track-3");
}

#[test]
fn repeat_one_observes_end_of_file_but_not_manual_next() {
    let mut session = populated(2);
    session.repeat = RepeatMode::One;
    let first = session.next(true).unwrap();
    session.position = 50.0;
    assert_eq!(session.next(false).unwrap().id, first.id);
    assert_eq!(session.position, 0.0);
    assert_ne!(session.next(true).unwrap().id, first.id);
    assert!(session.next(true).is_none());
}

#[test]
fn duplicate_files_keep_distinct_queue_identities() {
    let mut session = Session::new("Duplicates");
    session.append(vec![track(1), track(1), track(1)]);
    session.set_order(PlayOrder::ShuffleTracks);
    let entries = drain(&mut session);
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries
            .iter()
            .map(|entry| &entry.id)
            .collect::<HashSet<_>>()
            .len(),
        3
    );
    assert!(entries.iter().all(|entry| entry.track.id == "track-1"));
}

#[test]
fn shuffle_has_no_missing_or_repeated_entries_and_preserves_current() {
    for seed in 0..50 {
        let mut session = populated(20);
        session.shuffle_seed = seed;
        let first = session.next(true).unwrap();
        session.position = 17.5;
        session.set_order(PlayOrder::ShuffleTracks);
        assert_eq!(session.current_entry().unwrap().id, first.id);
        assert_eq!(session.position, 17.5);
        let remainder = drain(&mut session);
        assert_eq!(remainder.len(), 19);
        assert_eq!(
            remainder
                .iter()
                .map(|entry| &entry.id)
                .collect::<HashSet<_>>()
                .len(),
            19
        );
        assert!(remainder.iter().all(|entry| entry.id != first.id));
    }
}

#[test]
fn repeat_all_reshuffles_without_immediate_same_entry() {
    for seed in 0..80 {
        let mut session = populated(4);
        session.shuffle_seed = seed;
        session.set_order(PlayOrder::ShuffleTracks);
        session.repeat = RepeatMode::All;
        let pass = (0..4)
            .map(|_| session.next(true).unwrap().id)
            .collect::<Vec<_>>();
        assert_eq!(pass.iter().collect::<HashSet<_>>().len(), 4);
        let first_next_pass = session.next(false).unwrap();
        assert_ne!(first_next_pass.id, pass[3]);
    }
}

#[test]
fn repeat_all_handles_empty_and_single_entry_queues() {
    let mut session = populated(0);
    session.repeat = RepeatMode::All;
    assert!(session.next(false).is_none());
    session.append(vec![track(1)]);
    let first = session.next(true).unwrap();
    assert_eq!(session.next(false).unwrap().id, first.id);
}

#[test]
fn album_shuffle_keeps_disc_and_track_order_and_album_boundaries() {
    let mut session = Session::new("Albums");
    let mut tracks = Vec::new();
    for album in ["Alpha", "Beta", "Gamma"] {
        for number in [4, 2, 3, 1] {
            let mut item = track(number);
            item.album = album.into();
            item.disc_number = Some(if number > 2 { 2 } else { 1 });
            tracks.push(item);
        }
    }
    session.append(tracks);
    session.set_order(PlayOrder::ShuffleAlbums);
    let entries = drain(&mut session);
    for group in entries.chunks(4) {
        assert!(group
            .iter()
            .all(|entry| entry.track.album == group[0].track.album));
        assert_eq!(
            group
                .iter()
                .map(|entry| entry.track.track_number.unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
    }
}

#[test]
fn folder_shuffle_keeps_folders_together_on_windows_and_unix_paths() {
    let mut session = Session::new("Folders");
    let paths = [
        "C:\\A\\10.flac",
        "/music/B/2.flac",
        "C:\\A\\2.flac",
        "/music/B/1.flac",
    ];
    session.append(
        paths
            .into_iter()
            .map(|path| Track {
                path: path.into(),
                ..Track::default()
            })
            .collect(),
    );
    session.set_order(PlayOrder::ShuffleFolders);
    let entries = drain(&mut session);
    for group in entries.chunks(2) {
        assert_eq!(folder(&group[0].track.path), folder(&group[1].track.path));
        assert_eq!(
            natural_compare(
                filename(&group[0].track.path),
                filename(&group[1].track.path)
            ),
            Ordering::Less
        );
    }
}

#[test]
fn next_priority_survives_sorting_and_history_navigation() {
    let mut session = populated(4);
    session.next(true);
    session.next(true);
    session.previous();
    session.enqueue_next(vec![track(9), track(8)]);
    session.set_order(PlayOrder::ShuffleTracks);
    assert_eq!(session.next(true).unwrap().track.id, "track-9");
    assert_eq!(session.next(true).unwrap().track.id, "track-8");
    assert_eq!(session.next(true).unwrap().track.id, "track-2");
}

#[test]
fn changing_order_does_not_change_actual_previous_history() {
    let mut session = populated(6);
    let first = session.next(true).unwrap();
    let second = session.next(true).unwrap();
    session.set_order(PlayOrder::ShuffleTracks);
    let third = session.next(true).unwrap();
    assert_eq!(session.previous().unwrap().id, second.id);
    assert_eq!(session.previous().unwrap().id, first.id);
    assert_eq!(session.next(true).unwrap().id, second.id);
    assert_eq!(session.next(true).unwrap().id, third.id);
}

#[test]
fn removing_current_advances_and_removing_historical_entries_cleans_history() {
    let mut session = populated(3);
    let first = session.next(true).unwrap();
    let second = session.next(true).unwrap();
    session.remove(&first.id);
    session.remove(&second.id);
    assert_eq!(session.current_entry().unwrap().track.id, "track-3");
    assert_eq!(session.previous().unwrap().track.id, "track-3");
    session.remove(&session.current_entry().unwrap().id.clone());
    assert!(session.current_entry().is_none());
    assert!(session.snapshot().entries.is_empty());
}

#[test]
fn switching_back_to_manual_restores_remaining_base_order() {
    let mut session = populated(8);
    session.next(true);
    session.set_order(PlayOrder::ShuffleTracks);
    session.set_order(PlayOrder::Manual);
    assert_eq!(
        drain(&mut session)
            .iter()
            .map(|entry| entry.track.track_number.unwrap())
            .collect::<Vec<_>>(),
        (2..=8).collect::<Vec<_>>()
    );
}

#[test]
fn disabling_shuffle_restores_selected_sort_without_changing_current_or_history() {
    let mut session = Session::new("Sorted");
    session.append((1..=6).rev().map(track).collect());
    session.set_order(PlayOrder::Title);
    let first = session.next(true).unwrap();
    let current = session.next(true).unwrap();
    session.position = 27.5;
    session.set_shuffle(true);
    let shuffled = session.snapshot();
    session.set_shuffle(true);
    assert_eq!(
        session.snapshot(),
        shuffled,
        "enabling again must not reshuffle"
    );
    session.set_shuffle(false);
    assert_eq!(session.order_mode, PlayOrder::Title);
    assert_eq!(session.current_entry().unwrap().id, current.id);
    assert_eq!(session.position, 27.5);
    assert_eq!(session.previous().unwrap().id, first.id);
    assert_eq!(session.next(true).unwrap().id, current.id);
    assert_eq!(
        drain(&mut session)
            .iter()
            .map(|entry| entry.track.track_number.unwrap())
            .collect::<Vec<_>>(),
        vec![3, 4, 5, 6]
    );
}

#[test]
fn shuffle_toggle_preserves_play_next_priority_and_forward_history() {
    let mut session = populated(6);
    session.next(true);
    let second = session.next(true).unwrap();
    session.previous();
    session.position = 12.;
    session.enqueue_next(vec![track(9), track(8)]);
    session.set_order(PlayOrder::Duration);
    session.set_shuffle(true);
    session.set_shuffle(false);
    assert_eq!(session.position, 12.);
    assert_eq!(session.next(true).unwrap().track.id, "track-9");
    assert_eq!(session.next(true).unwrap().track.id, "track-8");
    assert_eq!(session.next(true).unwrap().id, second.id);
    assert_eq!(session.next(true).unwrap().track.id, "track-3");
}

#[test]
fn each_queue_remembers_its_own_unshuffled_order() {
    let mut first = populated(4);
    let mut second = populated(5);
    first.set_order(PlayOrder::Title);
    second.set_order(PlayOrder::Duration);
    first.set_shuffle(true);
    second.set_shuffle(true);
    let second_before = second.snapshot();
    first.set_shuffle(false);
    assert_eq!(second.snapshot(), second_before);
    second.set_shuffle(false);
    assert_eq!(first.order_mode, PlayOrder::Title);
    assert_eq!(second.order_mode, PlayOrder::Duration);
}

#[test]
fn unshuffled_sort_survives_persistence_and_legacy_shuffle_commands() {
    let mut session = populated(5);
    session.set_order(PlayOrder::Filename);
    session.set_order(PlayOrder::ShuffleAlbums);
    session.set_order(PlayOrder::ShuffleFolders);
    session.set_shuffle(true);
    let mut restored: Session =
        serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
    restored.sanitize();
    restored.set_shuffle(false);
    assert_eq!(restored.order_mode, PlayOrder::Filename);
    assert_eq!(
        drain(&mut restored)
            .iter()
            .map(|entry| entry.track.track_number.unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn older_shuffled_sessions_disable_to_manual_and_sorted_sessions_keep_their_sort() {
    for order in [
        PlayOrder::ShuffleTracks,
        PlayOrder::ShuffleAlbums,
        PlayOrder::Title,
    ] {
        let mut session = populated(4);
        session.set_order(order);
        let mut json = serde_json::to_value(&session).unwrap();
        json.as_object_mut().unwrap().remove("unshuffledOrder");
        let mut restored: Session = serde_json::from_value(json).unwrap();
        restored.sanitize();
        restored.set_shuffle(true);
        restored.set_shuffle(false);
        assert_eq!(
            restored.order_mode,
            if is_shuffle(order) {
                PlayOrder::Manual
            } else {
                order
            }
        );
    }
}

#[test]
fn manually_moving_an_entry_resets_the_shuffle_restore_target() {
    let mut session = populated(4);
    let snapshot = session.snapshot();
    session.set_order(PlayOrder::Title);
    session.set_shuffle(true);
    session.move_entry(&snapshot.entries[3].id, Some(&snapshot.entries[1].id));
    session.set_shuffle(true);
    session.set_shuffle(false);
    assert_eq!(session.order_mode, PlayOrder::Manual);
    assert_eq!(
        drain(&mut session)
            .iter()
            .map(|entry| entry.track.track_number.unwrap())
            .collect::<Vec<_>>(),
        vec![1, 4, 2, 3]
    );
}

#[test]
fn move_updates_remaining_queue_without_restarting_current() {
    let mut session = populated(4);
    let snapshot = session.snapshot();
    session.next(true);
    session.position = 10.0;
    session.move_entry(&snapshot.entries[3].id, Some(&snapshot.entries[1].id));
    assert_eq!(session.position, 10.0);
    assert_eq!(
        drain(&mut session)
            .iter()
            .map(|entry| entry.track.track_number.unwrap())
            .collect::<Vec<_>>(),
        vec![4, 2, 3]
    );
}

#[test]
fn snapshots_show_the_order_that_next_actually_uses() {
    let mut session = populated(5);
    session.next(true);
    session.next(true);
    session.set_order(PlayOrder::ShuffleTracks);
    session.enqueue_next(vec![track(9)]);
    let snapshot = session.snapshot();
    let current_index = snapshot
        .entries
        .iter()
        .position(|entry| Some(&entry.id) == snapshot.current_entry_id.as_ref())
        .unwrap();
    let shown_next = snapshot.entries[current_index + 1..]
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let played_next = drain(&mut session)
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(shown_next, played_next);
    assert_eq!(snapshot.entries.len(), 6);
}

#[test]
fn selecting_a_later_entry_shows_skipped_rows_before_current() {
    let mut session = populated(5);
    let selected = session.snapshot().entries[3].id.clone();
    assert!(session.play_entry(&selected));
    let snapshot = session.snapshot();
    let current_index = snapshot
        .entries
        .iter()
        .position(|entry| entry.id == selected)
        .unwrap();
    assert_eq!(current_index, 3);
    assert_eq!(
        snapshot.entries[current_index + 1].track.track_number,
        Some(5)
    );
    assert_eq!(drain(&mut session).len(), 1);
    assert!(!session.play_entry("missing"));
}

#[test]
fn sessions_round_trip_with_current_position_priority_and_history() {
    let mut session = populated(5);
    session.next(true);
    session.next(true);
    session.position = 34.25;
    session.enqueue_next(vec![track(9)]);
    session.repeat = RepeatMode::All;
    session.set_order(PlayOrder::ShuffleAlbums);
    let serialized = serde_json::to_string(&session).unwrap();
    let mut restored: Session = serde_json::from_str(&serialized).unwrap();
    restored.sanitize();
    assert_eq!(session.snapshot(), restored.snapshot());
    assert_eq!(session.next(true), restored.next(true));
    assert_eq!(session.previous(), restored.previous());
}

#[test]
fn independent_sessions_do_not_share_queue_or_position() {
    let mut first = populated(2);
    let mut second = populated(3);
    first.next(true);
    first.position = 90.0;
    second.next(true);
    second.next(true);
    assert_ne!(first.id, second.id);
    assert_eq!(first.current_entry().unwrap().track.track_number, Some(1));
    assert_eq!(second.current_entry().unwrap().track.track_number, Some(2));
    assert_eq!(first.position, 90.0);
    assert_eq!(second.position, 0.0);
}

#[test]
fn natural_sort_handles_numeric_names_and_missing_track_tags() {
    let mut session = Session::new("Names");
    session.append(
        ["10", "2", "1"]
            .into_iter()
            .map(|name| Track {
                path: format!("/music/{name}.flac"),
                title: format!("Title {name}"),
                ..Track::default()
            })
            .collect(),
    );
    session.set_order(PlayOrder::Filename);
    assert_eq!(
        drain(&mut session)
            .iter()
            .map(|entry| filename(&entry.track.path))
            .collect::<Vec<_>>(),
        vec!["1.flac", "2.flac", "10.flac"]
    );
}

#[test]
fn sanitize_removes_dangling_references_and_invalid_positions() {
    let mut session = populated(2);
    session.next(true);
    session.position = f64::NAN;
    session.upcoming.push("missing".into());
    session.upcoming.push(session.upcoming[0].clone());
    session.history.push("missing".into());
    session.sanitize();
    assert_eq!(session.position, 0.0);
    assert_eq!(session.upcoming.len(), 1);
    assert!(session.history.is_empty());
}
