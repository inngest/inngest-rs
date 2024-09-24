use core::fmt;
use std::collections::HashMap;

use base16;
use sha1::{Digest, Sha1};

enum Opcode {
    StepRun,
    StepSleep,
    StepWaitForEvent,
    StepInvoke,
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Opcode::StepRun => write!(f, "StepRun"),
            Opcode::StepSleep => write!(f, "Sleep"),
            Opcode::StepWaitForEvent => write!(f, "WaitForEvent"),
            Opcode::StepInvoke => write!(f, "InvokeFunction"),
        }
    }
}

pub struct Step {
    state: HashMap<String, String>,
}

impl Step {
    pub fn new(state: &HashMap<String, String>) -> Self {
        Step {
            state: state.clone(),
        }
    }

    // TODO: run

    pub fn sleep(&self) {
        // TODO: unhashed op
        // TODO: hash it
        // TODO: check if state has hashed_id as key already
        //       if not, throw to exit code execution
    }

    // TODO: sleep_until
    // TODO: wait_for_event
    // TODO: invoke
    // TODO: send_event
    // TODO: send_events
}

struct UnhashedOp {
    id: String,
    op: Opcode,
    pos: u32,
    // TODO: need an opts as map
}

impl UnhashedOp {
    fn new(op: &str, id: &str) -> Self {
        let opcode = match op {
            "Step" => Opcode::StepRun,
            "StepRun" => Opcode::StepRun,
            "Sleep" => Opcode::StepSleep,
            "WaitForEvent" => Opcode::StepWaitForEvent,
            "InvokeFunction" => Opcode::StepInvoke,
            _ => Opcode::StepRun,
        };

        UnhashedOp {
            id: id.to_string(),
            op: opcode,
            pos: 0,
        }
    }

    fn hash(&self) -> String {
        let key = if self.pos > 0 {
            format!("{}:{}", self.id, self.pos)
        } else {
            self.id.to_string()
        };

        let mut hasher = Sha1::new();
        hasher.update(key.as_bytes());
        let res = hasher.finalize();

        base16::encode_upper(res.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_hash() {
        let op = UnhashedOp {
            id: "hello".to_string(),
            op: Opcode::StepRun,
            pos: 0,
        };

        assert_eq!(op.hash(), "AAF4C61DDCC5E8A2DABEDE0F3B482CD9AEA9434D");
    }

    #[test]
    fn test_op_hash_with_position() {
        let op = UnhashedOp {
            id: "hello".to_string(),
            op: Opcode::StepRun,
            pos: 1,
        };

        assert_eq!(op.hash(), "20A9BB9477C4AC565CF084D1614C58BBF0A523FF");
    }
}
