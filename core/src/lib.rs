#[macro_use] extern crate serde;
#[macro_use] extern crate serde_derive; 
#[macro_use] extern crate serde_json;
extern crate actix_web;
// use std::collections::HashMap;
// use std::convert::From;
// use serde::{Serialize, Deserialize};
// use serde_json::Value as JsonValue;

pub mod models;
// use models;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
