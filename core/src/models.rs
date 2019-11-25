use std::collections::HashMap;
use std::convert::From;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

use actix_web::web::Bytes;

pub type AttrMap = HashMap<String, JsonValue>;



/// Message in internal bus
#[derive(Clone, Debug)]
pub struct MetaEvent {
    pub content_type: String,
    pub route: String,
    // pub parameters: HashMap<String, String>,

    pub headers: HashMap<String, String>,
    // pub payload: models::AttrMap,
    pub payload: Bytes,
}

