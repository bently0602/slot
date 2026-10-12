use slot::input::controller_hub::ControllerHub;
use slot_input::{
    Btn, ControllerConfig, ControllerInfo, ControllerMode, ControllerProfile, RawEvent,
};

fn pad(id: &str) -> ControllerInfo {
    ControllerInfo {
        id: id.into(),
        name: id.into(),
    }
}

fn two_pads() -> ControllerHub {
    let mut hub = ControllerHub::default();
    hub.connect(1, pad("ultimate"));
    hub.connect(2, pad("ultimate"));
    hub
}

fn in_game() -> ControllerHub {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Game);
    hub
}

#[test]
fn docked_psx_triggers_stay_game_buttons_and_disconnect_releases_them() {
    use slot_input::{Action, Gestures};
    let mut hub = in_game();
    let mut gestures = Gestures::new();
    let actions: Vec<_> = hub
        .set(1, vec![Btn::L2, Btn::R2])
        .into_iter()
        .flat_map(|event| gestures.feed_psx(event, 0))
        .collect();
    assert!(actions.contains(&Action::GbaDown(Btn::L2)));
    assert!(actions.contains(&Action::GbaDown(Btn::R2)));
    assert!(!actions.contains(&Action::RewindStart));
    assert!(!actions.contains(&Action::FfStart));
    let actions: Vec<_> = hub
        .disconnect(1)
        .into_iter()
        .flat_map(|event| gestures.feed_psx(event, 2001))
        .collect();
    assert!(actions.contains(&Action::GbaUp(Btn::L2)));
    assert!(actions.contains(&Action::GbaUp(Btn::R2)));
    assert!(!actions.contains(&Action::QuickMenu));
}

#[test]
fn pads_are_seated_as_players_in_the_order_they_connect() {
    let mut hub = two_pads();
    assert_eq!(hub.status().players, [1, 2]);
    hub.connect(3, pad("other"));
    hub.connect(4, pad("other"));
    hub.connect(5, pad("other"));
    assert_eq!(hub.status().players, [1, 2, 3, 4, 4]);
}

#[test]
fn every_pad_drives_the_menus() {
    let mut hub = two_pads();
    assert_eq!(hub.set(2, vec![Btn::Down]), vec![RawEvent::Down(Btn::Down)]);
    assert_eq!(hub.set(1, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
    assert_eq!(hub.set(2, vec![]), vec![RawEvent::Up(Btn::Down)]);
    assert!(
        hub.players().iter().all(Vec::is_empty),
        "a menu press reached the game"
    );
}

#[test]
fn in_a_game_each_pad_plays_its_own_player() {
    let mut hub = in_game();
    assert_eq!(hub.set(1, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
    assert!(
        hub.set(2, vec![Btn::B, Btn::Left]).is_empty(),
        "player two pressed player one's buttons"
    );
    assert_eq!(hub.players()[1], vec![Btn::B, Btn::Left]);
    assert!(hub.players()[0].is_empty() && hub.players()[2].is_empty());
}

#[test]
fn any_players_menu_button_opens_the_menu_and_is_not_sent_to_the_game() {
    let mut hub = in_game();
    assert_eq!(hub.set(2, vec![Btn::Menu]), vec![RawEvent::Down(Btn::Menu)]);
    assert!(hub.players()[1].is_empty());
}

#[test]
fn a_button_held_when_a_menu_opens_waits_for_a_fresh_press() {
    let mut hub = in_game();
    hub.set(2, vec![Btn::A]);
    assert!(
        hub.set_mode(ControllerMode::Menu).is_empty(),
        "a button held in the game acted on the menu"
    );
    hub.set(2, vec![]);
    assert_eq!(hub.set(2, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);

    assert_eq!(
        hub.set_mode(ControllerMode::Game),
        vec![RawEvent::Up(Btn::A)],
        "the menu press was left held when the game took the pad back"
    );
    assert!(
        hub.players()[1].is_empty(),
        "a button held in the menu was pressed in the game"
    );
    hub.set(2, vec![]);
    hub.set(2, vec![Btn::A]);
    assert_eq!(hub.players()[1], vec![Btn::A]);
}

#[test]
fn player_one_keeps_a_held_button_across_a_menu() {
    let mut hub = in_game();
    hub.set(1, vec![Btn::Right]);
    assert!(hub.set_mode(ControllerMode::Menu).is_empty());
    assert!(hub.set_mode(ControllerMode::Game).is_empty());
    assert_eq!(hub.set(1, vec![]), vec![RawEvent::Up(Btn::Right)]);
}

#[test]
fn each_pad_moves_itself_between_players_and_one_and_two_can_swap() {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Assign);
    assert!(
        hub.set(1, vec![Btn::Right]).is_empty(),
        "choosing a player moved the menu"
    );
    assert_eq!(hub.status().players, [2, 2]);
    hub.set(1, vec![]);
    hub.set(2, vec![Btn::Left]);
    assert_eq!(hub.status().players, [2, 1]);
    hub.set(2, vec![]);

    hub.set_mode(ControllerMode::Game);
    assert_eq!(hub.set(2, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
    hub.set(1, vec![Btn::B]);
    assert_eq!(hub.players()[1], vec![Btn::B]);
}

#[test]
fn a_pad_stops_at_the_first_and_last_player() {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Assign);
    for _ in 0..6 {
        hub.set(2, vec![Btn::Right]);
        hub.set(2, vec![]);
    }
    assert_eq!(hub.status().players, [1, 4]);
    for _ in 0..6 {
        hub.set(2, vec![Btn::Left]);
        hub.set(2, vec![]);
    }
    assert_eq!(hub.status().players, [1, 1]);
}

#[test]
fn holding_a_direction_moves_one_player_and_other_buttons_still_reach_the_screen() {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Assign);
    hub.set(2, vec![Btn::Right]);
    hub.set(2, vec![Btn::Right, Btn::Up]);
    assert_eq!(hub.status().players, [1, 3]);
    assert_eq!(
        hub.set(2, vec![Btn::Right, Btn::Up, Btn::B]),
        vec![RawEvent::Down(Btn::B)]
    );
}

#[test]
fn two_pads_on_one_player_share_it() {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Assign);
    hub.set(1, vec![Btn::Right]);
    hub.set(1, vec![]);
    hub.set_mode(ControllerMode::Game);
    hub.set(1, vec![Btn::A]);
    hub.set(2, vec![Btn::B]);
    assert_eq!(hub.players()[1], vec![Btn::A, Btn::B]);
}

#[test]
fn a_chosen_seat_survives_another_pad_connecting() {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Assign);
    hub.set(1, vec![Btn::Right]);
    hub.set(1, vec![]);
    hub.set(1, vec![Btn::Right]);
    hub.set(1, vec![]);
    assert_eq!(hub.status().players, [3, 2]);
    hub.connect(3, pad("other"));
    assert_eq!(hub.status().players, [3, 2, 1]);
}

#[test]
fn player_two_keeps_their_seat_while_player_one_is_unplugged() {
    let mut hub = in_game();
    hub.set(2, vec![Btn::X]);
    hub.disconnect(1);
    assert_eq!(hub.status().players, [2]);
    assert_eq!(hub.players()[1], vec![Btn::X]);
}

#[test]
fn unplugging_a_pad_releases_its_buttons() {
    let mut hub = two_pads();
    hub.set(2, vec![Btn::A]);
    assert_eq!(hub.disconnect(2), vec![RawEvent::Up(Btn::A)]);

    let mut hub = in_game();
    hub.set(2, vec![Btn::X]);
    hub.disconnect(2);
    assert!(hub.players()[1].is_empty());
}

#[test]
fn every_player_uses_the_profile_saved_for_their_pad() {
    let mut hub = ControllerHub::default();
    hub.connect(1, pad("ultimate"));
    hub.connect(2, pad("other"));
    hub.configure(&ControllerConfig {
        profiles: vec![ControllerProfile {
            id: "other".into(),
            swap_ab: true,
            ..ControllerProfile::default()
        }],
        ..ControllerConfig::default()
    });
    hub.set_mode(ControllerMode::Game);
    hub.set(2, vec![Btn::A]);
    assert_eq!(hub.players()[1], vec![Btn::B]);
}

#[test]
fn the_handheld_starts_as_player_one_and_moves_with_its_own_left_and_right() {
    let mut hub = two_pads();
    assert_eq!(hub.status().handheld_player(), 1);
    assert!(
        !hub.builtin(RawEvent::Down(Btn::Right)),
        "the handheld changed player outside the player screen"
    );
    hub.set_mode(ControllerMode::Assign);
    assert!(hub.builtin(RawEvent::Down(Btn::Right)));
    assert!(!hub.builtin(RawEvent::Up(Btn::Right)));
    assert_eq!(hub.status().handheld_player(), 2);
    assert_eq!(hub.status().players, [1, 2], "the handheld moved a pad");
    for _ in 0..6 {
        hub.builtin(RawEvent::Down(Btn::Right));
    }
    assert_eq!(hub.status().handheld_player(), 4);
    for _ in 0..6 {
        hub.builtin(RawEvent::Down(Btn::Left));
    }
    assert_eq!(hub.status().handheld_player(), 1);
    assert!(
        !hub.builtin(RawEvent::Down(Btn::B)),
        "the handheld could not leave the player screen"
    );
}

#[test]
fn a_pad_seated_with_the_handheld_shares_its_stream_and_the_rest_play_apart() {
    let mut hub = two_pads();
    hub.set_mode(ControllerMode::Assign);
    hub.builtin(RawEvent::Down(Btn::Right));
    hub.set_mode(ControllerMode::Game);

    assert_eq!(hub.set(2, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
    assert!(hub.players()[1].is_empty());
    assert!(
        hub.set(1, vec![Btn::B]).is_empty(),
        "player one pressed the handheld's buttons"
    );
    assert_eq!(hub.players()[0], vec![Btn::B]);
}
