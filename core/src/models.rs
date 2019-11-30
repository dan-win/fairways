use std::collections::HashMap;
use std::convert::From;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

use actix_web::web::Bytes;

use std::fmt;

pub type AttrMap = HashMap<String, JsonValue>;



/// Message in internal bus
#[derive(Clone, Debug)]
pub struct Context {
    pub content_type: String,
    pub route: String,
    // pub parameters: HashMap<String, String>,

    pub headers: HashMap<String, String>,
}

pub struct MetaEvent {
    pub ctx: Context,
    // pub payload: models::AttrMap,
    pub payload: Bytes,
}

impl fmt::Debug for MetaEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut vec_of_u8 = vec![];
        vec_of_u8.extend_from_slice(&self.payload);
        let bytes_repr = String::from_utf8(vec_of_u8).unwrap_or("<err>".to_string());
        let dummy = Bytes::from_static(b"-");
        write!(f, "MetaEvent {{ context: {:?}, payload: {} }}", self.ctx, bytes_repr)
    }
}

