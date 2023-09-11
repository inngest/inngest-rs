pub mod error;
pub mod event;
pub mod function;
pub mod router;
pub mod sdk;

__private::inventory::collect!(__private::EventMeta);

pub mod __private {
    pub extern crate inventory;

    pub struct EventMeta {
        pub ename: String,
        pub etype: String,
    }
}
