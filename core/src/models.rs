use std::collections::HashMap;
use std::convert::From;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

use actix_web::web::Bytes;

use std::fmt;

pub type AttrMap = HashMap<String, JsonValue>;



/// Message in internal bus
#[derive(Clone)]
pub struct MetaEvent {
    pub content_type: String,
    pub route: String,
    // pub parameters: HashMap<String, String>,

    pub headers: HashMap<String, String>,
    // pub payload: models::AttrMap,
    pub payload: Option<Bytes>,
}

impl fmt::Debug for MetaEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes_repr: String = match self.payload.clone() {
            Some(v) => {
                let mut vec_of_u8 = vec![];
                vec_of_u8.extend_from_slice(&v);
                String::from_utf8(vec_of_u8).unwrap_or("<err>".to_string())
            },
            None => String::from("-")
        };
        let dummy = Bytes::from_static(b"-");
        write!(f, "MetaEvent {{ content_type: {}, route: {}, headerss: {:?}, payload: {} }}", self.content_type, 
        self.route, 
        self.headers, 
        bytes_repr
        )
    }
}