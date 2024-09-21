use std::collections::HashMap;

pub struct Step {
    state: HashMap<String, String>,
}

impl Step {
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
