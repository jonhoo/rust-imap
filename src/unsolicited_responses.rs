use enumset::EnumSet;
use std::sync::mpsc;

use super::types::{UnsolicitedResponse, UnsolicitedResponseCategory};

#[derive(Debug, Clone)]
pub struct UnsolicitedResponseSender {
    sender: mpsc::Sender<UnsolicitedResponse>,
    allow: EnumSet<UnsolicitedResponseCategory>,
}


impl UnsolicitedResponseSender {
    pub fn new(sender: mpsc::Sender<UnsolicitedResponse>) -> UnsolicitedResponseSender {
        UnsolicitedResponseSender {
            sender,
            allow: EnumSet::empty(),
        }
    }

    // Check if the user wants the specified unsolicited response.
    fn filter(&self, r: &UnsolicitedResponse) -> bool {
       match r {
            UnsolicitedResponse::Status { .. } => self.allow.contains(UnsolicitedResponseCategory::Status),
            UnsolicitedResponse::Recent(_) => self.allow.contains(UnsolicitedResponseCategory::Recent),
            UnsolicitedResponse::Exists(_) => self.allow.contains(UnsolicitedResponseCategory::Exists),
            UnsolicitedResponse::Expunge(_) => self.allow.contains(UnsolicitedResponseCategory::Expunge),
            UnsolicitedResponse::Ok { .. } => self.allow.contains(UnsolicitedResponseCategory::Ok),
            UnsolicitedResponse::No { .. } => self.allow.contains(UnsolicitedResponseCategory::No),
            UnsolicitedResponse::Bad { .. } => self.allow.contains(UnsolicitedResponseCategory::Bad),
            UnsolicitedResponse::Bye { .. } => self.allow.contains(UnsolicitedResponseCategory::Bye),
            UnsolicitedResponse::Fetch { .. } => self.allow.contains(UnsolicitedResponseCategory::Fetch),
        }
    }

    // Set the new filter mask, and remove unwanted responses from the current queue.
    pub fn request(&mut self, rcv: &mpsc::Receiver<UnsolicitedResponse>, mask: EnumSet<UnsolicitedResponseCategory>) {
        self.allow = mask;
        let mut keep: Vec<_> = rcv.try_iter().filter(|r| self.filter(r)).collect();
        keep.drain(..).for_each(|r| self.sender.send(r).unwrap());
    }

    pub fn send(&mut self, r: UnsolicitedResponse) {
        if self.filter(&r) {
            self.sender.send(r).unwrap();
        }
    }
}
