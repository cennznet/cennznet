// Copyright (C) 2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::sync::Arc;

use sp_runtime::traits::{Block, NumberFor};
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};

use parking_lot::Mutex;

/// Stream of signed witness' returned when subscribing.
pub type SignedWitness<Block> = ethy_primitives::SignedWitness<NumberFor<Block>, ethy_primitives::MmrRootHash>;

/// Stream of signed witness' returned when subscribing.
type SignedWitnessStream<Block> = TracingUnboundedReceiver<SignedWitness<Block>>;

/// Sending endpoint for notifying about signed witness'.
type SignedWitnessSender<Block> = TracingUnboundedSender<SignedWitness<Block>>;

/// Collection of channel sending endpoints shared with the receiver side so they can register
/// themselves.
type SharedSignedWitnessSenders<Block> = Arc<Mutex<Vec<SignedWitnessSender<Block>>>>;

/// The sending half of the signed witness channel(s).
///
/// Used to send notifications about signed witness' generated at the end of an ETHY round.
#[derive(Clone)]
pub struct EthySignedWitnessSender<B>
where
	B: Block,
{
	subscribers: SharedSignedWitnessSenders<B>,
}

impl<B> EthySignedWitnessSender<B>
where
	B: Block,
{
	/// The `subscribers` should be shared with a corresponding `SignedWitnessSender`.
	fn new(subscribers: SharedSignedWitnessSenders<B>) -> Self {
		Self { subscribers }
	}

	/// Send out a notification to all subscribers that a new signed witness is available for a
	/// block.
	pub fn notify(&self, signed_witness: SignedWitness<B>) {
		let mut subscribers = self.subscribers.lock();

		// do an initial prune on closed subscriptions
		subscribers.retain(|n| !n.is_closed());

		if !subscribers.is_empty() {
			subscribers.retain(|n| n.unbounded_send(signed_witness.clone()).is_ok());
		}
	}
}

/// The receiving half of the signed witnesss channel.
///
/// Used to receive notifications about signed witnesss generated at the end of a ETHY round.
/// The `EthySignedWitnessStream` entity stores the `SharedSignedWitnessSenders` so it can be
/// used to add more subscriptions.
#[derive(Clone)]
pub struct EthySignedWitnessStream<B>
where
	B: Block,
{
	subscribers: SharedSignedWitnessSenders<B>,
}

impl<B> EthySignedWitnessStream<B>
where
	B: Block,
{
	/// Creates a new pair of receiver and sender of signed witness notifications.
	pub fn channel() -> (EthySignedWitnessSender<B>, Self) {
		let subscribers = Arc::new(Mutex::new(vec![]));
		let receiver = EthySignedWitnessStream::new(subscribers.clone());
		let sender = EthySignedWitnessSender::new(subscribers);
		(sender, receiver)
	}

	/// Create a new receiver of signed witness notifications.
	///
	/// The `subscribers` should be shared with a corresponding `EthySignedWitnessSender`.
	fn new(subscribers: SharedSignedWitnessSenders<B>) -> Self {
		Self { subscribers }
	}

	/// Subscribe to a channel through which signed witnesss are sent at the end of each ETHY
	/// voting round.
	pub fn subscribe(&self) -> SignedWitnessStream<B> {
		let (sender, receiver) = tracing_unbounded("mpsc_signed_witnesss_notification_stream");
		self.subscribers.lock().push(sender);
		receiver
	}
}
