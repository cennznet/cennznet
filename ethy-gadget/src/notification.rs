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

use cennznet_primitives::eth::VersionedEventProof;
use sc_utils::notification::{NotificationSender, NotificationStream, TracingKeyStr};

/// The sending half of the event proof channel(s).
///
/// Used to send notifications about event proofs generated after a majority of validators have witnessed the event
pub type EthyEventProofSender = NotificationSender<VersionedEventProof>;

/// The receiving half of the event proof channel.
///
/// Used to receive notifications about event proofs generated at the end of a ETHY round.
pub type EthyEventProofStream = NotificationStream<VersionedEventProof, EthyEventProofTracingKey>;

/// Provides tracing key for ETHY event proof stream.
#[derive(Clone)]
pub struct EthyEventProofTracingKey;
impl TracingKeyStr for EthyEventProofTracingKey {
	const TRACING_KEY: &'static str = "mpsc_ethy_event_proof_notification_stream";
}
