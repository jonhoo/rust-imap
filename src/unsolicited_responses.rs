use enumset::EnumSet;
use std::sync::mpsc;

use super::types::{UnsolicitedResponse, UnsolicitedResponseCategory};

#[derive(Debug, Clone)]
pub struct UnsolicitedResponseSender {
    sender: mpsc::Sender<UnsolicitedResponse>,
    allow: EnumSet<UnsolicitedResponseCategory>,
}

impl UnsolicitedResponseSender {
    pub(crate) fn new(sender: mpsc::Sender<UnsolicitedResponse>) -> UnsolicitedResponseSender {
        UnsolicitedResponseSender {
            sender,
            allow: EnumSet::empty(),
        }
    }

    // Set the new filter mask, and remove unwanted responses from the current queue.
    pub(crate) fn request(
        &mut self,
        receiver: &mpsc::Receiver<UnsolicitedResponse>,
        mask: EnumSet<UnsolicitedResponseCategory>,
    ) {
        self.allow = mask;
        for message in receiver.try_iter() {
            self.send(message);
        }
    }

    pub(crate) fn send(&mut self, message: UnsolicitedResponse) {
        if self.allow.contains(message.category()) {
            self.sender.send(message).unwrap();
        }
    }
}
