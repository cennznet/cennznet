// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

use cennznet_primitives::eth::EventProof;
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};

use parking_lot::Mutex;

/// Stream of event proofs returned when subscribing.
type EventProofStream = TracingUnboundedReceiver<EventProof>;

/// Sending endpoint for notifying about event proofs.
type EventProofSender = TracingUnboundedSender<EventProof>;

/// Collection of channel sending endpoints shared with the receiver side so they can register
/// themselves.
type SharedEventProofSenders = Arc<Mutex<Vec<EventProofSender>>>;

/// The sending half of the event proof channel(s).
///
/// Used to send notifications about event proofs generated after a majority of validators have witnessed the event
#[derive(Clone)]
pub struct EthyEventProofSender {
	subscribers: SharedEventProofSenders,
}

impl EthyEventProofSender {
	/// The `subscribers` should be shared with a corresponding `EventProofSender`.
	fn new(subscribers: SharedEventProofSenders) -> Self {
		Self { subscribers }
	}

	/// Send out a notification to all subscribers that a new event proof is available for a
	/// block.
	pub fn notify(&self, event_proof: EventProof) {
		let mut subscribers = self.subscribers.lock();

		// do an initial prune on closed subscriptions
		subscribers.retain(|n| !n.is_closed());

		if !subscribers.is_empty() {
			subscribers.retain(|n| n.unbounded_send(event_proof.clone()).is_ok());
		}
	}
}

/// The receiving half of the event proof channel.
///
/// Used to receive notifications about event proofs generated at the end of a ETHY round.
/// The `EthyEventProofStream` entity stores the `SharedEventProofSenders` so it can be
/// used to add more subscriptions.
#[derive(Clone)]
pub struct EthyEventProofStream {
	subscribers: SharedEventProofSenders,
}

impl EthyEventProofStream {
	/// Creates a new pair of receiver and sender of event proof notifications.
	pub fn channel() -> (EthyEventProofSender, Self) {
		let subscribers = Arc::new(Mutex::new(vec![]));
		let receiver = EthyEventProofStream::new(subscribers.clone());
		let sender = EthyEventProofSender::new(subscribers);
		(sender, receiver)
	}

	/// Create a new receiver of event proof notifications.
	///
	/// The `subscribers` should be shared with a corresponding `EthyEventProofSender`.
	fn new(subscribers: SharedEventProofSenders) -> Self {
		Self { subscribers }
	}

	/// Subscribe to a channel through which event proofs are sent at the end of each ETHY
	/// voting round.
	pub fn subscribe(&self) -> EventProofStream {
		let (sender, receiver) = tracing_unbounded("mpsc_event_proofs_notification_stream");
		self.subscribers.lock().push(sender);
		receiver
	}
}
