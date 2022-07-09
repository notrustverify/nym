// Copyright 2021 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use super::action_controller::{Action, ActionSender};
use futures::StreamExt;
use gateway_client::AcknowledgementReceiver;
use log::*;
use nymsphinx::{
    acknowledgements::{identifier::recover_identifier, AckKey},
    chunking::fragment::{FragmentIdentifier, COVER_FRAG_ID},
};
use std::{sync::Arc, time::Duration};
use task::ShutdownListener;

/// Module responsible for listening for any data resembling acknowledgements from the network
/// and firing actions to remove them from the 'Pending' state.
pub(super) struct AcknowledgementListener {
    ack_key: Arc<AckKey>,
    ack_receiver: AcknowledgementReceiver,
    action_sender: ActionSender,
    shutdown: ShutdownListener,
}

impl AcknowledgementListener {
    pub(super) fn new(
        ack_key: Arc<AckKey>,
        ack_receiver: AcknowledgementReceiver,
        action_sender: ActionSender,
        shutdown: ShutdownListener,
    ) -> Self {
        AcknowledgementListener {
            ack_key,
            ack_receiver,
            action_sender,
            shutdown,
        }
    }

    async fn on_ack(&mut self, ack_content: Vec<u8>) {
        debug!("Received an ack");
        let frag_id = match recover_identifier(&self.ack_key, &ack_content)
            .map(FragmentIdentifier::try_from_bytes)
        {
            Some(Ok(frag_id)) => frag_id,
            _ => {
                warn!("Received invalid ACK!"); // should we do anything else about that?
                return;
            }
        };

        // if we received an ack for cover message or a reply there will be nothing to remove,
        // because nothing was inserted in the first place
        if frag_id == COVER_FRAG_ID {
            trace!("Received an ack for a cover message - no need to do anything");
            return;
        } else if frag_id.is_reply() {
            info!("Received an ack for a reply message - no need to do anything! (don't know what to do!)");
            // TODO: probably there will need to be some extra procedure here, something to notify
            // user that his reply reached the recipient (since we got an ack)
            return;
        }

        trace!("Received {} from the mix network", frag_id);

        self.action_sender
            .unbounded_send(Action::new_remove(frag_id))
            .unwrap();
    }

    pub(super) async fn run(&mut self) {
        debug!("Started AcknowledgementListener");
        while !self.shutdown.is_shutdown() {
            tokio::select! {
                Some(acks) = self.ack_receiver.next() => {
                    // realistically we would only be getting one ack at the time
                    for ack in acks {
                        self.on_ack(ack).await;
                    }
                },
                _ = self.shutdown.recv() => {
                    log::trace!("AcknowledgementListener: Received shutdown");
                }
            }
        }

        log::info!("AcknowledgementListener: Entering listen state");

        loop {
            tokio::select! {
                Some(acks) = self.ack_receiver.next() => {
                    log::trace!("Ignoring acks received");
                },
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    log::info!("MixTrafficController: Finished waiting");
                    break;
                }
            }
        }
        log::info!("AcknowledgementListener: Exiting");
    }
}
