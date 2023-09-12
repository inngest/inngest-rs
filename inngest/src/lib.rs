pub mod error;
pub mod event;
pub mod function;
pub mod router;
pub mod sdk;

__private::inventory::collect!(__private::EventMeta);

pub mod __private {
    pub extern crate inventory;

    #[derive(Debug)]
    pub struct EventMeta {
        pub ename: &'static str,
        pub etype: &'static str,
    }
}
