//! Behavioral contract for every adapter, including the Swift/Kotlin wire path.
use kog_playback_policy::{
    NavigationEvent as E, OrderTrack, PlaybackDecision as D, PlaybackOrder, RepeatMode as R,
    ShuffleMode as S, bridge::dispatch_json,
};
use serde_json::{Value, json};

fn tracks() -> Vec<OrderTrack> {
    [("A", 1, 2), ("A", 1, 1), ("B", 1, 1), ("B", 2, 1)]
        .into_iter()
        .map(|(album, disc, track)| OrderTrack {
            album: album.into(),
            disc_number: Some(disc),
            track_number: Some(track),
        })
        .collect()
}

#[test]
fn transport_contract() {
    let tracks = tracks();
    let mut p = PlaybackOrder::new(S::Off, R::Off, 17);
    assert_eq!(p.navigate(&tracks, None, E::Next), D::Play(0));
    assert_eq!(p.navigate(&tracks, Some(0), E::Previous), D::Play(0));
    assert_eq!(p.navigate(&tracks, Some(3), E::Next), D::Stop);
    p.set_repeat_mode(R::All);
    assert_eq!(p.navigate(&tracks, Some(3), E::Ended), D::Play(0));
    assert_eq!(p.navigate(&tracks, Some(0), E::Previous), D::Play(3));
    p.set_repeat_mode(R::One);
    p.toggle_queue(&[3]);
    assert_eq!(p.navigate(&tracks, Some(1), E::Ended), D::Play(1));
    assert_eq!(p.navigate(&tracks, Some(1), E::Next), D::Play(3));
    p.set_repeat_mode(R::Album);
    assert_eq!(p.navigate(&tracks, Some(1), E::Ended), D::Play(0));
    p.toggle_stop_after(&[1]);
    assert_eq!(p.navigate(&tracks, Some(1), E::Ended), D::Stop);
    assert_eq!(p.navigate(&tracks, Some(1), E::Next), D::Play(0));
    p.started(Some(1), 0);
    assert!(!p.should_stop_after(1));
}

#[test]
fn failures_cannot_spin_under_any_repeat_or_shuffle_mode() {
    let tracks = tracks();
    for shuffle in [S::Off, S::All, S::Albums] {
        for repeat in [R::Off, R::One, R::Album, R::All] {
            let mut p = PlaybackOrder::new(shuffle, repeat, 19);
            let mut seen = std::collections::HashSet::new();
            let mut result = p.navigate(&tracks, Some(0), E::Ended);
            while let D::Play(index) = result {
                assert!(
                    seen.insert(index),
                    "repeated failed candidate: {shuffle:?}/{repeat:?}"
                );
                assert!(seen.len() <= tracks.len());
                result = p.navigate(&tracks, Some(index), E::Failed);
            }
            assert_eq!(result, D::Stop);
        }
    }
}

#[test]
fn sorted_projection_and_physically_sorted_queue_visit_the_same_tracks() {
    let tracks = tracks();
    let indices = [2, 0, 3, 1];
    let reordered: Vec<_> = indices.iter().map(|i| tracks[*i].clone()).collect();
    let mut projected = PlaybackOrder::new(S::Off, R::Off, 1);
    projected.set_sequence(indices.to_vec(), tracks.len());
    let mut physical = PlaybackOrder::new(S::Off, R::Off, 1);
    let (mut left, mut right) = (None, None);
    for expected in indices {
        assert_eq!(
            projected.navigate(&tracks, left, E::Next),
            D::Play(expected)
        );
        let D::Play(next) = physical.navigate(&reordered, right, E::Next) else {
            panic!()
        };
        assert_eq!(tracks[expected], reordered[next]);
        projected.started(left, expected);
        physical.started(right, next);
        left = Some(expected);
        right = Some(next);
    }
    assert_eq!(projected.navigate(&tracks, left, E::Next), D::Stop);
    assert_eq!(physical.navigate(&reordered, right, E::Next), D::Stop);
}

#[derive(Default)]
struct Wire {
    state: Option<String>,
}
impl Wire {
    fn send(&mut self, command: Value) -> Value {
        let reply: Value = serde_json::from_str(
            &dispatch_json(
                &json!({
                    "state": self.state, "command": command,
                })
                .to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        self.state = Some(reply["state"].as_str().unwrap().into());
        reply
    }
}

#[test]
fn mobile_wire_preserves_duplicate_rows_and_cancels_stale_radio() {
    let mut wire = Wire::default();
    let rows = json!([{"id":"same"},{"id":"other"},{"id":"same"}]);
    wire.send(json!({"op":"sync", "tracks":rows, "current":0}));
    wire.send(json!({"op":"toggle_queue", "indices":[2]}));
    wire.send(json!({"op":"toggle_stop_after", "indices":[0,2]}));
    let moved = wire.send(json!({"op":"sync", "tracks":[{"id":"same"},{"id":"same"}],
        "current":1,"old_to_new":[1,null,0]}));
    assert_eq!(moved["queued"], json!([0]));
    assert_eq!(moved["stop_after"], json!([0, 1]));
    let radio = wire.send(json!({"op":"radio_reset","enabled":true,"current":1}));
    let generation = radio["radio"]["generation"].as_u64().unwrap();
    assert_eq!(radio["shuffle"], "off");
    assert_eq!(radio["repeat"], "off");
    assert_eq!(radio["radio"]["waiting"], false);
    wire.send(json!({"op":"radio_next"}));
    wire.send(json!({"op":"cancel_waiting"}));
    wire.send(json!({"op":"radio_accept","generation":generation,"entries":[{"id":"pick"}],"exhausted":false}));
    assert!(wire.send(json!({"op":"radio_pending"}))["entry"].is_null());
    assert_eq!(wire.send(json!({"op":"radio_next"}))["entry"]["id"], "pick");
    wire.send(json!({"op":"radio_reset","enabled":true,"current":1}));
    assert_eq!(
        wire.send(
            json!({"op":"radio_accept","generation":generation,"entries":[{}],"exhausted":false})
        )["accepted"],
        false
    );
    let repeat = wire.send(json!({"op":"cycle_repeat"}));
    assert_eq!(repeat["repeat"], "one");
    assert_eq!(repeat["radio"]["enabled"], false);
}

#[test]
fn mobile_sorting_and_filtering_use_raw_metadata_and_natural_order() {
    let mut wire = Wire::default();
    let rows = json!([
        {"title":"Song 10","artist":"Alice","duration":10,"star":false},
        {"title":"Song 2","artist":"Bob","duration":2,"star":true},
        {"title":"song 2","artist":"Alice","duration":null,"star":true}
    ]);
    for (column, expected) in [
        ("title", json!([1, 2, 0])),
        ("duration", json!([2, 1, 0])),
        ("star", json!([1, 2, 0])),
    ] {
        assert_eq!(
            wire.send(json!({"op":"sort_rows","rows":rows,"column":column,"descending":false}))["indices"],
            expected
        );
    }
    assert_eq!(
        wire.send(json!({"op":"filter_rows","rows":rows,"query":"ALICE song"}))["indices"],
        json!([0, 2])
    );
}
