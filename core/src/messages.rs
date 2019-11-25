/// Backbone messages
use actix::prelude::*;

use crate::models;
use crate::errors;

use collections::HashMap;

// /// Main representation of the input request
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct Event {
//     // Keep status for processing if necessary:
//     pub ctx: models::AttrMap,
//     // Put path, header, cookies, query here? 
//     pub payload: models::AttrMap,
// }

