// Copyright 2019-2021
//     by  Centrality Investments Ltd.
//     and Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Tests for the module.

#![cfg(test)]

use super::*;

use crate::mock::{ALICE, BOB};
use crate::{
	mock::{Event, ExtBuilder, Origin, Snake, System},
	types::{Direction, Food, Snake as snake_obj, WindowSize, MAX_WINDOW_SIZE, MIN_WINDOW_SIZE},
};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};

fn has_event(event: RawEvent<u64, WindowSize, snake_obj, Food, Direction, u32>) -> bool {
	System::events()
		.iter()
		.find(|e| e.event == Event::crml_snake(event.clone()))
		.is_some()
}

#[test]
fn start_game_with_default_window_dimensions() {
	ExtBuilder::default().start_game(true).build();
}

//Check that the window size defaults to MAX_WINDOW_SIZE if it is started with a larger value
#[test]
fn start_game_with_too_large_window_should_default() {
	ExtBuilder::default()
		.start_game(true)
		.window_size(MAX_WINDOW_SIZE + 5)
		.build()
		.execute_with(|| {
			assert_eq!(
				Snake::window_meta(ALICE),
				WindowSize {
					window_width: MAX_WINDOW_SIZE,
					window_height: MAX_WINDOW_SIZE,
				}
			);
		});
}

//Check that the window size defaults to MIN_WINDOW_SIZE if it is started with a smaller value
#[test]
fn start_game_with_too_small_window_should_default() {
	ExtBuilder::default()
		.start_game(true)
		.window_size(0)
		.build()
		.execute_with(|| {
			assert_eq!(
				Snake::window_meta(ALICE),
				WindowSize {
					window_width: MIN_WINDOW_SIZE,
					window_height: MIN_WINDOW_SIZE,
				}
			);
		});
}

#[test]
fn end_game() {
	ExtBuilder::default().start_game(true).build().execute_with(|| {
		assert_ok!(Snake::end_game(Origin::signed(ALICE)));
		assert_eq!(
			has_event(RawEvent::GameEnded(
				ALICE,
				WindowSize {
					window_width: 20,
					window_height: 20,
				},
				4
			)),
			true
		);
	});
}

#[test]
fn snake_should_change_direction() {
	ExtBuilder::default().start_game(true).build().execute_with(|| {
		assert_ok!(Snake::change_direction(Origin::signed(ALICE), Direction::Down));
		assert!(has_event(RawEvent::DirectionChanged(
			ALICE,
			Snake::snake_meta(ALICE),
			Direction::Down,
		)));
	});
}

#[test]
fn snake_changing_direction_to_same_direction_should_throw_event() {
	ExtBuilder::default()
		.start_game(true)
		.window_size(20)
		.build()
		.execute_with(|| {
			assert_ok!(Snake::change_direction(Origin::signed(ALICE), Direction::Right));

			assert!(has_event(RawEvent::DirectionSameAsOldDirection(
				ALICE,
				Snake::snake_meta(ALICE),
				Direction::Right
			)));
		});
}

#[test]
fn two_accounts_should_have_separate_game_instances() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Snake::start(Origin::signed(ALICE), 20, 20));
		assert_ok!(Snake::start(Origin::signed(BOB), 10, 10));

		assert_ok!(Snake::change_direction(Origin::signed(ALICE), Direction::Down));

		assert_eq!(Snake::snake_meta(ALICE).dir, Direction::Down);
		assert_eq!(
			Snake::window_meta(ALICE),
			WindowSize {
				window_width: 20,
				window_height: 20,
			}
		);

		assert_eq!(Snake::snake_meta(BOB).dir, Direction::Right);
		assert_eq!(
			Snake::window_meta(BOB),
			WindowSize {
				window_width: 10,
				window_height: 10,
			}
		);
	});
}
