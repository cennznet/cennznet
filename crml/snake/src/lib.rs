/* Copyright 2019-2020 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

#![cfg_attr(not(feature = "std"), no_std)]
mod mock;
mod tests;
mod types;

use codec::Encode;

use byteorder::{BigEndian, ByteOrder};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, traits::Randomness, weights::Weight, IterableStorageMap,
	StorageMap,
};
use frame_system::{ensure_signed, WeightInfo};
use sp_core::hash::H256;
use sp_runtime::DispatchResult;
use sp_std::cmp::{max, min};
use sp_std::vec;
pub use types::{Direction, Food, Snake, WindowSize, MAX_WINDOW_SIZE, MIN_WINDOW_SIZE};

// 2. Runtime Configuration Trait
// All of the runtime types and consts go in here. If the module
// is dependent on specific other modules, then their configuration traits
// should be added to the inherited traits list.
pub trait Config: frame_system::Config {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// Weight information for extrinsics in this module.
	type WeightInfo: WeightInfo;
	/// Randomness Source
	type RandomnessSource: Randomness<H256>;
}

decl_error! {
	/// Error for the generic-asset module.
	pub enum Error for Module<T: Config> {
		/// There is no game data for this account
		NoGameExists,
		/// There is already a game running, end this game before starting a new one
		GameAlreadyRunning,
		/// There is no data associated with the snake
		SnakeHasNoBody,
	}
}

// The Module Declaration
// This defines the `Module` struct that is ultimately exported from this pallet.
// It defines the callable functions that this pallet exposes and orchestrates
// actions this pallet takes throughout block execution.
decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		//Called on every block update. Use to move the position of the snake
		fn on_initialize(_block: T::BlockNumber) -> Weight {
			for game in WindowMeta::<T>::iter() {
				let _ = Self::update_snake_position(game.0, game.1);
			}
			0
		}

		/// Start the game of snake and create a new snake
		/// window_height: height of game window
		/// window_width: width of game window
		#[weight = 0]
		pub fn start(
			origin,
			window_width: i8,
			window_height: i8,
		) -> DispatchResult {
			let account = ensure_signed(origin)?;
			ensure!(!<WindowMeta<T>>::contains_key(&account), Error::<T>::GameAlreadyRunning);

			Self::start_game(account, window_width, window_height)
		}

		/// Change the direction of the snake
		/// Check to see that it is not doubling back on itself
		#[weight = 0]
		pub fn change_direction(
			origin,
			new_direction: Direction,
		) -> DispatchResult {
			let account = ensure_signed(origin)?;

			ensure!(<WindowMeta<T>>::contains_key(&account), Error::<T>::NoGameExists);

			Self::update_snake_direction(account, new_direction)
		}

		/// User ends game before the snake eats itself
		#[weight = 0]
		pub fn end_game(
			origin,
		) -> DispatchResult {
			let account = ensure_signed(origin)?;

			ensure!(<WindowMeta<T>>::contains_key(&account), Error::<T>::NoGameExists);

			Self::end_of_game(&account);
			Ok(())
		}
	}
}

// Runtime Storage
// This allows for type-safe usage of the Substrate storage database, so you can
// keep things around between blocks.
decl_storage! {
	trait Store for Module<T: Config> as Snake {
		/// the game data associated with an account id
		pub WindowMeta get(fn window_meta): map hasher(blake2_128_concat) T::AccountId => WindowSize;

		/// the snake data associated with an account id
		pub SnakeMeta get(fn snake_meta): map hasher(blake2_128_concat) T::AccountId => Snake;

		/// the food data associated with an account id
		pub FoodMeta get(fn food_meta): map hasher(blake2_128_concat) T::AccountId => Food;

		/// incrementing value used to handle randomness
		pub Nonce get(fn nonce): u32;

	}
}

// Runtime Events
// Events are a simple means of reporting specific conditions and circumstances
// that have happened that users, Daps and/or chain explorers would find
// interesting and otherwise difficult to detect.
decl_event! {
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
		WindowSize = WindowSize,
		Snake = Snake,
		Food = Food,
		Direction = Direction,
		score = u32,
	{
		/// Game started (account_id, game, snake, food)
		GameStarted(AccountId, WindowSize, Snake, Food),
		/// Game has ended (account_id, game, score)
		GameEnded(AccountId, WindowSize, score),
		/// Snakes Direction has changed (account_id, snake, new_direction)
		DirectionChanged(AccountId, Snake, Direction),
		/// Snakes new direction is the same as the previous direction (account_id, snake, new_direction)
		DirectionSameAsOldDirection(AccountId, Snake, Direction),
		/// The snake can only turn 90 degrees, it can't turn back on itself (account_id, snake, new_direction)
		SnakeCantGoBackwards(AccountId, Snake, Direction),
		/// Snakes Position has updated (account_id, snake, food)
		PositionUpdated(AccountId, Snake, Food),
		/// Snake has eaten food (account_id, snake, food)
		FoodMoved(AccountId, Snake, Food),
	}
}

impl<T: Config> Module<T> {
	/// Starts a new game and creates a default snake in the top left corner
	pub fn start_game(account_id: T::AccountId, window_width: i8, window_height: i8) -> DispatchResult {
		let snake = Snake {
			body: vec![(3, 0), (2, 0), (1, 0), (0, 0)],
			dir: Direction::Right,
			direction_changed: false,
		};

		let game_state = WindowSize {
			window_width: max(MIN_WINDOW_SIZE, min(window_width, MAX_WINDOW_SIZE)),
			window_height: max(MIN_WINDOW_SIZE, min(window_height, MAX_WINDOW_SIZE)),
		};

		let mut food = Food { x: 2, y: 2 };
		food = Self::move_food(account_id.clone(), food, &snake, &game_state);

		<WindowMeta<T>>::insert(&account_id, &game_state);
		<SnakeMeta<T>>::insert(&account_id, &snake);
		<FoodMeta<T>>::insert(&account_id, &food);

		Self::deposit_event(Event::<T>::GameStarted(account_id, game_state, snake, food));
		Ok(())
	}

	/// Check the direction of the snake is not going the opposite way, then update the direction
	pub fn update_snake_direction(account_id: T::AccountId, new_direction: Direction) -> DispatchResult {
		let mut snake: Snake = Self::snake_meta(&account_id);
		let last_direction: Direction = snake.dir.clone();

		if new_direction != last_direction && snake.direction_changed == false {
			snake.dir = match new_direction {
				Direction::Up if last_direction != Direction::Down => Direction::Up,
				Direction::Down if last_direction != Direction::Up => Direction::Down,
				Direction::Left if last_direction != Direction::Right => Direction::Left,
				Direction::Right if last_direction != Direction::Left => Direction::Right,
				_ => last_direction,
			};
			if snake.dir == new_direction {
				snake.direction_changed = true;
				<SnakeMeta<T>>::insert(&account_id, &snake);
				Self::deposit_event(Event::<T>::DirectionChanged(
					account_id.clone(),
					snake.clone(),
					new_direction.clone(),
				));
			} else {
				Self::deposit_event(Event::<T>::SnakeCantGoBackwards(
					account_id.clone(),
					snake.clone(),
					new_direction.clone(),
				));
			}
		} else {
			Self::deposit_event(Event::<T>::DirectionSameAsOldDirection(
				account_id,
				snake,
				new_direction,
			));
		}

		Ok(())
	}

	/// End the game
	pub fn end_of_game(account_id: &T::AccountId) {
		let game_state = Self::window_meta(&account_id);
		let snake_length: u32 = Self::snake_meta(&account_id).body.len() as u32;

		<SnakeMeta<T>>::remove(&account_id);
		<WindowMeta<T>>::remove(&account_id);
		<FoodMeta<T>>::remove(&account_id);

		Self::deposit_event(Event::<T>::GameEnded(account_id.clone(), game_state, snake_length));
	}

	/// Update the position of the snake every block, call this after the direction has been changed,
	/// or if there is no user input, move in the same direction
	pub fn update_snake_position(account_id: T::AccountId, game_state: WindowSize) -> DispatchResult {
		let mut snake: Snake = Self::snake_meta(&account_id);
		let mut food: Food = Self::food_meta(&account_id);
		let mut new_head = snake.body[0].clone();
		snake.direction_changed = false;

		//Move snake body
		match snake.dir {
			Direction::Right => {
				new_head.0 += 1;
				if new_head.0 >= game_state.window_width {
					new_head.0 = 0
				}
			}
			Direction::Left => {
				new_head.0 -= 1;
				if new_head.0 < 0 {
					new_head.0 = game_state.window_width - 1
				}
			}
			Direction::Up => {
				new_head.1 -= 1;
				if new_head.1 < 0 {
					new_head.1 = game_state.window_height - 1
				}
			}
			Direction::Down => {
				new_head.1 += 1;
				if new_head.1 >= game_state.window_height {
					new_head.1 = 0
				}
			}
		}

		// Check if the snake head is on food
		if new_head.0 == food.x && new_head.1 == food.y {
			food = Self::move_food(account_id.clone(), food.clone(), &snake, &game_state);
			Self::deposit_event(Event::<T>::FoodMoved(account_id.clone(), snake.clone(), food.clone()));

			let tail = snake.body[snake.body.len() - 1].clone();
			snake.body.push(tail)
		}

		// check if the snake head is on it's body
		let game_over = snake.body.iter().any(|&x| x.0 == new_head.0 && x.1 == new_head.1);

		snake.body.insert(0, new_head);
		let _ = snake.body.pop();
		if game_over {
			Self::end_of_game(&account_id);
		} else {
			<SnakeMeta<T>>::insert(&account_id, &snake);
			Self::deposit_event(Event::<T>::PositionUpdated(account_id, snake, food));
		}

		Ok(())
	}

	// Find a new location of the food that doesn't collide with the body
	pub fn move_food(account_id: T::AccountId, mut food: Food, snake: &Snake, game: &WindowSize) -> Food {
		let mut rand_x: i8 = 0;
		let mut rand_y: i8 = 0;
		let prev_food_pos = (food.x, food.y);
		let mut blank_spot = false;

		// Check that the food won't spawn on a snake part
		while blank_spot == false {
			rand_x = Self::get_random_int(game.window_width as u64) as i8;
			rand_y = Self::get_random_int(game.window_height as u64) as i8;

			blank_spot = !snake.body.iter().any(|&x| x.0 == rand_x && x.1 == rand_y)
				&& !(rand_x == prev_food_pos.0 && rand_y == prev_food_pos.1);
		}
		food.x = rand_x;
		food.y = rand_y;

		<FoodMeta<T>>::insert(&account_id, &food);
		food
	}

	// Reads the nonce from storage, increments the stored nonce, and returns
	// the encoded nonce to the caller.
	pub fn get_random_int(upper_bound: u64) -> u64 {
		let nonce = Self::nonce().wrapping_add(1);
		<Nonce>::put(nonce);
		let subject = nonce.encode();
		BigEndian::read_u64(T::RandomnessSource::random(&subject).as_bytes()) % upper_bound
	}
}
