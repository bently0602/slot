use slot_ui::{player_column, player_row, players_face, Seat, OUT_H, OUT_W, PLAYER_ROWS};

fn inked(face: &[u8], x: u32, y: u32, w: u32, h: u32) -> usize {
    let mut n = 0;
    for yy in y..y + h {
        for xx in x..x + w {
            if face[((yy * OUT_W + xx) * 4 + 3) as usize] > 0xc0 {
                n += 1;
            }
        }
    }
    n
}

fn seats(players: &[u8]) -> Vec<Seat<'static>> {
    players
        .iter()
        .map(|player| Seat {
            name: "8BitDo Ultimate",
            player: *player,
        })
        .collect()
}

#[test]
fn the_board_fills_the_screen_and_keeps_four_columns_inside_it() {
    let face = players_face(&seats(&[1, 2]));
    assert_eq!((face.w, face.h), (OUT_W, OUT_H));
    let mut right = 0;
    for player in 1..=4 {
        let (x, w) = player_column(player);
        assert!(
            x >= right,
            "player {player}'s column overlaps the one before"
        );
        right = x + w;
    }
    assert!(right <= OUT_W);
    let (y, h) = player_row(PLAYER_ROWS - 1);
    assert!(y + h <= OUT_H);
}

#[test]
fn each_controller_stands_in_its_own_row_under_the_player_it_chose() {
    let face = players_face(&seats(&[3, 1]));
    for (row, player) in [(0, 3), (1, 1)] {
        let (y, h) = player_row(row);
        for column in 1..=4 {
            let (x, w) = player_column(column);
            let ink = inked(&face.rgba, x, y, w, h);
            if column == player {
                assert!(
                    ink > 200,
                    "row {row} has no controller under player {player}"
                );
            } else {
                assert_eq!(ink, 0, "row {row} drew a controller under player {column}");
            }
        }
    }
}

#[test]
fn moving_a_controller_redraws_it_in_the_new_column() {
    assert_ne!(
        players_face(&seats(&[1, 2])).rgba,
        players_face(&seats(&[2, 1])).rgba
    );
}

#[test]
fn a_fifth_controller_does_not_run_off_the_board() {
    let face = players_face(&seats(&[1, 2, 3, 4, 4]));
    assert_eq!((face.w, face.h), (OUT_W, OUT_H));
}

#[test]
#[ignore = "writes a picture to look at: cargo test -p slot-ui --test players -- --ignored"]
fn preview() {
    let face = players_face(&[
        Seat {
            name: "Handheld",
            player: 1,
        },
        Seat {
            name: "8BitDo Ultimate Wireless",
            player: 2,
        },
        Seat {
            name: "8BitDo Ultimate Wireless",
            player: 1,
        },
        Seat {
            name: "Xbox Wireless",
            player: 4,
        },
    ]);
    let mut px = Vec::with_capacity((OUT_W * OUT_H * 3) as usize);
    for p in face.rgba.chunks_exact(4) {
        let a = u32::from(p[3]);
        for (c, under) in [0x2a, 0x2c, 0x34].into_iter().enumerate() {
            px.push(((u32::from(p[c]) * a + under * (255 - a)) / 255) as u8);
        }
    }
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("players.png");
    let file = std::fs::File::create(&path).expect("create");
    let mut enc = png::Encoder::new(file, OUT_W, OUT_H);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(&px))
        .expect("png");
    println!("PREVIEW {}", path.display());
}
