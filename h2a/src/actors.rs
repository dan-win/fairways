use actix::prelude::*;
use actix_broker::{BrokerIssue, BrokerSubscribe};
use std::time::Duration;
use std::convert::From;
use std::env;
// use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
// use crate::lapin::types::FieldTable;
// use crate::lapin::{Client, ConnectionProperties, Channel};

use amiquip::{
    Channel, ConfirmSmoother, Connection, Exchange, Publish, AmqpProperties, QueueDeclareOptions, Result as AmiquipResult,
};

use futures::future::{Future, FutureResult, IntoFuture, result, ok as fut_ok, err as fut_err, finished, Either, join_all};

use std::collections::HashMap;

use actix_web::{
    // get, // <- here is "decorator"
    http, 
    error,
    // middleware, 
    // HttpServer, 
    // App, Error as AWError, HttpMessage, ResponseError,
    HttpRequest, HttpResponse,
    FromRequest,
    // FutureResponse, 
    // web::Data, 
    web,
    web::Path, web::Query, web::Json, web::PayloadConfig, 
    // web::Payload,
};

use actix_web::dev; // <--- for dev::Payload

use actix_web::web::Bytes;

use error::Error;

use crate::messages::{
    AmqpMessage, 
    AmqpMessageResponse, 
    AmqpFailMessage, 

    BackboneConnectTo,
    BackboneConnectionReset,

    BcConnectionEstablished, 
    BcConnectionLost, 
    NeedsConnection,

    SpawnChannel, 
    SpawnChannelResponse
};

use crate::errors::SvcError;

use fairways_core::models::{Context as MsgContext, MetaEvent};
// use std::sync::mpsc;
use uuid;

const LOCAL_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);



enum ProxyAction {
    Dummy,
    ConnectTo(String),
    Reconnect,
    HandleConnection(String, Connection),
    // RetryLater(String),
    EventErrorRecoverable(SvcError),
    EventErrorNonRecoverable(SvcError),
    // ProxyMessage(AmqpMessage),
}

impl std::fmt::Debug for ProxyAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ProxyAction::*;
        let legend = match self {
            Dummy => "Dummy".to_string(),
            ConnectTo(conn_str) => format!("ConnectTo: {}", &conn_str),
            Reconnect => "Reconnect".to_string(),
            // With for immediate connection as soon as possible
            HandleConnection(conn_str, _) => format!("HandleConnection: {}", &conn_str),
            // Deferred attempt due to recent connection failure
            EventErrorRecoverable(e) => format!("Error Recoverable: ({:?})", e),
            EventErrorNonRecoverable(e) => format!("Error Non Recoverable: ({:?})", e),
            _ => unreachable!()
        };
        write!(f, "State: {}", legend)
    }    
}


// See also a parallel idea (!): https://github.com/jgallagher/amiquip/blob/master/src/io_loop/handshake_state.rs
enum ProxyState {
    Dummy,
    Connected(String, Connection),
    // With for immediate connection as soon as possible
    WaitsForConnection(String),
    // Deferred attempt due to recent connection failure
    WaitsForRetry(String, SvcError),
    NonRecoverable(SvcError)
}

impl ProxyState {
    /// Accessors:
    pub fn get_connection(&mut self) -> Result<&mut Connection, SvcError> {
        match self {
            ProxyState::Connected(_, ref mut conn) => Ok(conn),
            _ => Err(SvcError::InvalidOperationForState("Cannot get connection".to_string()))
        }
    }

    pub fn get_conn_str(&self) -> Result<String, SvcError> {
        match self {
            ProxyState::WaitsForConnection(conn_str) => Ok(conn_str.clone()),
            ProxyState::WaitsForRetry(conn_str, _) => Ok(conn_str.clone()),
            ProxyState::Connected(conn_str, _) => Ok(conn_str.clone()),
            _ => Err(SvcError::InvalidOperationForState("Connection string was not assigned".to_string()))
        }
    }

    pub fn get_last_err(&self) -> Option<SvcError> {
        match self {
            ProxyState::WaitsForRetry(_, e) => Some(e.clone()),
            ProxyState::NonRecoverable(e) => Some(e.clone()),
            _ => None
        }
    }

    pub fn is_connected(&self) -> bool {
        if let ProxyState::Connected(_, _) = *self {
            return true
        }
        false
    }

    pub fn dispatch(&mut self, action: ProxyAction) -> bool {
        if let ProxyAction::Dummy = action {
            return false
        }

        match *self {

            ProxyState::NonRecoverable(_) => {
                return false;
            },

            ProxyState::Dummy => {
                match action {
                    ProxyAction::ConnectTo(conn_str) => {
                        *self = ProxyState::WaitsForConnection(conn_str);
                    },
                    _ => unreachable!()
                };
            },

            ProxyState::WaitsForConnection(ref conn_str0) => {
                match action {
                    ProxyAction::HandleConnection(conn_str, conn_handle) => {
                        *self = ProxyState::Connected(conn_str, conn_handle);
                    },
                    ProxyAction::EventErrorRecoverable(e) => {
                        *self = ProxyState::WaitsForRetry(conn_str0.clone(), e);
                    },
                    ProxyAction::EventErrorNonRecoverable(e) => {
                        *self = ProxyState::NonRecoverable(e);
                    },
                    ProxyAction::ConnectTo(conn_str) => {
                        if *conn_str0 == conn_str {return false};
                        *self = ProxyState::WaitsForConnection(conn_str);
                    },
                    // connected -> non connected; all channels obsolete!
                    _ => unreachable!()
                };
            },

            ProxyState::WaitsForRetry(ref conn_str0, _) => {
                debug!("State: {:?} | action: {:?}", self, action);
                match action {
                    ProxyAction::HandleConnection(conn_str, conn_handle) => {
                        *self = ProxyState::Connected(conn_str, conn_handle);
                    },
                    ProxyAction::EventErrorNonRecoverable(e) => {
                        *self = ProxyState::NonRecoverable(e);
                    },
                    ProxyAction::ConnectTo(conn_str) => {
                        if *conn_str0 == conn_str {return false};
                        *self = ProxyState::WaitsForConnection(conn_str);
                    },
                    ProxyAction::Reconnect => {
                        *self = ProxyState::WaitsForConnection(conn_str0.clone());
                    },
                    // connected -> non connected; all channels obsolete!
                    _ => unreachable!()
                };
            },

            ProxyState::Connected(ref conn_str0, _) => {
                match action {
                    ProxyAction::EventErrorRecoverable(e) => {
                        *self = ProxyState::WaitsForRetry(conn_str0.clone(), e);
                    },
                    ProxyAction::EventErrorNonRecoverable(e) => {
                        *self = ProxyState::NonRecoverable(e);
                    },
                    ProxyAction::ConnectTo(conn_str) => {
                        if *conn_str0 == conn_str {return false};
                        *self = ProxyState::WaitsForConnection(conn_str);
                    },
                    ProxyAction::Reconnect => {
                        *self = ProxyState::WaitsForConnection(conn_str0.clone());
                    },
                    // connected -> non connected; all channels obsolete!
                    _ => unreachable!()
                };
            },

            _ => unreachable!()
        };
        return true;
    }    
}

impl std::default::Default for ProxyState {
    fn default() -> Self {
        ProxyState::Dummy
    }
}

impl std::fmt::Debug for ProxyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ProxyState::*;
        let legend = match self {
            Dummy => "Dummy".to_string(),
            Connected(conn_str, _) => format!("Connected: {}", &conn_str),
            // With for immediate connection as soon as possible
            WaitsForConnection(conn_str) => format!("WaitsForConnection: {}", &conn_str),
            // Deferred attempt due to recent connection failure
            WaitsForRetry(conn_str, e) => format!("WaitsForRety: {} ({:?})", &conn_str, e),
            NonRecoverable(e) => format!("Non Recoverable: ({:?})", e),
            _ => unreachable!()
        };
        write!(f, "State: {}", legend)
    }    
}

pub struct BackboneActor {
    state: ProxyState,
    id: uuid::Uuid,
}

impl std::default::Default for BackboneActor {
    fn default() -> BackboneActor {
        BackboneActor {
            state: ProxyState::default(),
            id: uuid::Uuid::new_v4()
        }
    }
}

impl std::fmt::Debug for BackboneActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BackboneActor {{ id: {}, state: {:?} }}", self.id, self.state)
    }    
}


impl Actor for BackboneActor {
    type Context = Context<Self>;
}

impl Supervised for BackboneActor {
    fn restarting(&mut self, ctx: &mut Context<Self>) {
        debug!("-- BackboneActor {}: restarting called ", self.id);
        self.dispatch(ProxyAction::Reconnect, ctx);
    }
}

impl SystemService for BackboneActor {}

impl Handler<BackboneConnectTo> for BackboneActor {
    type Result = ();
    fn handle(&mut self, msg: BackboneConnectTo, ctx: &mut Self::Context) {
        let BackboneConnectTo(conn_str) = msg;
        self.dispatch(ProxyAction::ConnectTo(conn_str), ctx);
    }
}

impl Handler<SpawnChannel> for BackboneActor {
    type Result = SpawnChannelResponse;
    fn handle(&mut self, _: SpawnChannel, _: &mut Self::Context) -> SpawnChannelResponse {
        self.open_channel()
    }
}

impl Handler<BackboneConnectionReset> for BackboneActor {
    type Result = ();
    fn handle(&mut self, _: BackboneConnectionReset, ctx: &mut Self::Context) {
        debug!("-- BackboneActor {}: restarting called via message handler", self.id);
        self.dispatch(ProxyAction::Reconnect, ctx);
    }
}

impl BackboneActor {

    pub fn load_from_registry() -> Addr<Self> {
        Self::from_registry()
    }

    fn dispatch(&mut self, action: ProxyAction, ctx: &mut Context<Self>) {
        let state_changed:bool = self.state.dispatch(action);
        if state_changed {
            self.handle_state_changes(ctx)
        }
    }

    fn handle_state_changes(&mut self, ctx: &mut Context<Self>) {
        match self.state {
            ProxyState::WaitsForConnection(..) => {
                debug!("-- BackboneActor {}: handle_state_change: WaitsForConnection", self.id);
                let action = self.try_connect();
                self.dispatch(action, ctx);
            },
            ProxyState::WaitsForRetry(..) => {
                debug!("-- BackboneActor {}: handle_state_change: WaitsForRetry", self.id);
                let id = self.id.clone();
                ctx.run_later(Duration::new(5, 0), move |_act, ctx| {
                    warn!("Running sheduled restart for BackboneActor {} ...", id);
                    ctx.stop(); // Force Superviser to restart actor
                });
            },
            ProxyState::Connected(..) => {
                debug!("-- BackboneActor {}: handle_state_change: Connected", self.id);
            },
            ProxyState::NonRecoverable(..) => {
                warn!("-- BackboneActor {}: handle_state_change: NonRecoverable", self.id);
            },
            ProxyState::Dummy => {
                debug!("-- BackboneActor {}: handle_state_change: Dummy", self.id);
            },
            _ => unreachable!(),
        };
    }

    #[inline]
    fn try_connect(&self) -> ProxyAction {
        if let Ok(conn_str) = self.state.get_conn_str() {

            return match Connection::insecure_open(&conn_str) {
                
                Ok(conn) => {
                    debug!("New Connection established");
                    ProxyAction::HandleConnection(conn_str, conn)
                },
                
                Err(e) => {
                    error!("Error on connection: {:?}", e);
                    let svc_error = SvcError::from( e);
                    match svc_error {
                        // Non-recoverable errors:
                        SvcError::NonRecoverableError(_) => ProxyAction::EventErrorNonRecoverable(svc_error),
                        _ => ProxyAction::EventErrorRecoverable(svc_error),
                    }
                }
            }
        };
        ProxyAction::Dummy
    }

    #[inline]
    pub fn open_channel(&mut self) -> Result<Channel, SvcError> {
        let conn = self.state.get_connection()?;
        let channel = conn.open_channel(None)?;
        Ok(channel)
    }

}


pub struct ChannelActor {
    channel: Option<Channel>,
    id: uuid::Uuid
}

impl std::default::Default for ChannelActor {
    fn default() -> ChannelActor {
        ChannelActor {
            channel: None,
            id: uuid::Uuid::new_v4()
        }
    }
}

impl Actor for ChannelActor {
    type Context = SyncContext<Self>;
}

impl ChannelActor {
    fn reset(&mut self) -> bool {
        debug!("-- ChannelActor {}: restarting called ", self.id);

        let backbone = BackboneActor::from_registry();
        let res = backbone.send(SpawnChannel).wait();
        println!("======================>");
        match res {
            Ok(inner) => {
                match inner {
                    Ok(channel) => {
                        self.channel = Some(channel);
                        debug!("-- ChannelActor {}: restarting [Ok], got channel ", self.id);
                        return true;
                    },
                    Err(e) => {
                        debug!("-- ChannelActor {}: restarting [Err], ", self.id);
                        return false;
                    }
                };
            },
            // MailboxError
            Err(e) => {
                self.channel = None;
                debug!("-- ChannelActor {}: restarting [Err], mailbox error ", self.id);
                return false;
            }
        };
    }

    fn send_msg(channel: &Channel, msg: AmqpMessage) -> AmqpMessageResponse {
        let AmqpMessage(bytes_payload, msg_ctx) = msg;
        let exchange_name: String = msg_ctx.route.clone();
        let routing_key = String::from("");
        let properties = AmqpProperties::default()
            .with_content_type(msg_ctx.content_type.clone()); // <- later: add .with_headers!
        // let body = &payload;
        let payload = Publish{
            body: &bytes_payload,
            routing_key,
            mandatory: false, // <- use later to handle errors on non-existing exchanges!
            immediate: false,
            properties,
        };

        channel.basic_publish(exchange_name, payload)?;
        Ok(())
    }

}


impl Handler<AmqpMessage> for ChannelActor {
    type Result = AmqpMessageResponse;
    
    fn handle(&mut self, msg: AmqpMessage, ctx: &mut Self::Context) -> AmqpMessageResponse {
        let res = match self.channel {
            Some(ref channel) => {
                Self::send_msg(channel, msg)
            },
            None => {
                if self.reset() {
                    match self.channel {
                        Some(ref channel) => {
                            Self::send_msg(channel, msg)
                        },
                        _ => unreachable!()
                    }
                } else {
                    Err(SvcError::MessageError("Channel not connected".to_string()))
                }
            }
        };
        if let Err(e) = res {
            let svc_err = SvcError::from(e);
            error!("Error sending message: {}", svc_err);
            match svc_err {
                SvcError::ChannelError(_) | SvcError::ConnectionError(_) | SvcError::TargetNotFound(_) => {
                    // Force to reset:
                    self.channel = None;
                },
                // Ignore other errors, do not reset channel on them
                _ => {}
            };
            return Err(svc_err);
        };
        res        
    }

}


pub struct ClientPool {
    pub addr: Recipient<AmqpMessage>
}
