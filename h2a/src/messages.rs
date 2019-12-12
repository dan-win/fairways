use actix::prelude::*;

use fairways_core::models::{Context as MsgContext, MetaEvent};

use actix_web::web::Bytes;

use amiquip::Channel;

use crate::errors::SvcError;


pub type AmqpMessageResponse = Result<(), SvcError>;
pub type SpawnChannelResponse = Result<Channel, SvcError>;

#[derive(Clone, Debug, Message)]
/// Command to restart AmqpProxy with specified connection string
pub struct BackboneConnectTo(pub String);

#[derive(Clone, Debug, Message)]
/// Command to restart AmqpProxy with specified connection string
pub struct BackboneConnectionReset;

#[derive(Clone, Debug, Message)]
#[rtype(result = "SpawnChannelResponse")]
/// Request to Backbone to create new channel
pub struct SpawnChannel;


#[derive(Clone, Debug, Message)]
#[rtype(result = "AmqpMessageResponse")]
pub struct AmqpMessage(pub Bytes, pub MsgContext);

#[derive(Clone, Debug, Message)]
pub struct AmqpFailMessage(pub Bytes, pub MsgContext, pub SvcError);


// Control messages

#[derive(Clone, Message)]
pub struct BcConnectionEstablished;

#[derive(Clone, Message)]
pub struct BcConnectionLost;

#[derive(Clone, Message)]
pub struct NeedsConnection;
