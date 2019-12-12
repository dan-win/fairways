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
use std::sync::mpsc;


const LOCAL_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const MAX_SIZE: usize = 262_144;

lazy_static! {
    static ref HEADER_CT_JSON: http::header::HeaderValue = http::header::HeaderValue::from_static("application/json");
    static ref HEADER_CT_URLENCODED: http::header::HeaderValue = http::header::HeaderValue::from_static("x-form/urlencoded");
}

#[derive(Debug, Clone)]
pub struct HttpEventContext {
    inner: MsgContext,
}

impl HttpEventContext {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> MsgContext {
        self.inner
    }    
}
impl FromRequest for HttpEventContext {
    type Config = ();

    type Error = Error;
    type Future = Result<Self, Self::Error>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut dev::Payload) -> Self::Future {
        let route = req.match_info().get("tail").unwrap_or("").to_string();
        let content_type = req.headers().get("content-type").unwrap_or(&HEADER_CT_URLENCODED).to_str().unwrap().to_string();
        let headers: HashMap<String, String> = HashMap::new();

        let inner = MsgContext {
            content_type,
            route,
            headers
        };

        Ok(HttpEventContext{inner})
    }
}




#[derive(Debug)]
pub struct HttpGetPayload {
    inner: Bytes
}

#[derive(Debug)]
pub struct HttpPostPayload {
    inner: Bytes
}


impl HttpPostPayload {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> Bytes {
        self.inner
    }    
}

impl HttpGetPayload {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> Bytes {
        self.inner
    }    
}

impl FromRequest for HttpPostPayload {
    type Config = ();

    type Error = Error;
    type Future =
        Box<dyn Future<Item = HttpPostPayload, Error = Error>>;
        // Either<Box<dyn Future<Item = HttpPostPayload, Error = Error> + 'static>, FutureResult<HttpPostPayload, Error>>;

    #[inline]
    fn from_request(_req: &HttpRequest, payload: &mut dev::Payload) -> Self::Future {

        // payload is a stream of Bytes objects
        Box::new(payload.take()
            // `Future::from_err` acts like `?` in that it coerces the error type from
            // the future into the final error type
            .from_err()
            // `fold` will asynchronously read each chunk of the request body and
            // call supplied closure, then it resolves to result of closure
            .fold(web::BytesMut::with_capacity(MAX_SIZE), move |mut body, chunk| {
                // limit max size of in-memory payload
                if (body.len() + chunk.len()) > MAX_SIZE {
                    Err(error::PayloadError::Overflow)
                } else {
                    body.extend_from_slice(&chunk);
                    Ok(body)
                }
            })
            .map(|body| body.freeze())
            .and_then(|payload|{

                let inner = payload;

                Ok(HttpPostPayload{inner})

            }))
    }
}

impl FromRequest for HttpGetPayload {
    type Config = ();

    type Error = Error;
    type Future = Result<Self, Self::Error>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut dev::Payload) -> Self::Future {
        let inner = web::Bytes::from(req.query_string());

        Ok(HttpGetPayload{inner})
    }
}



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

#[derive(Default)]
pub struct BackboneActor {
    state: ProxyState,
}

impl Actor for BackboneActor {
    type Context = Context<Self>;
}

impl Supervised for BackboneActor {
    fn restarting(&mut self, ctx: &mut Context<Self>) {
        debug!("-restarting");

        if let Ok(_) = self.state.get_conn_str() {
            ctx.run_later(Duration::new(0, 10), move |_act, ctx| {
                ctx.address().do_send(BackboneConnectionReset);
            });    
        }           
    }
}

impl SystemService for BackboneActor {

}

impl Handler<BackboneConnectTo> for BackboneActor {
    type Result = ();
    fn handle(&mut self, msg: BackboneConnectTo, ctx: &mut Context<BackboneActor>) {
        let BackboneConnectTo(conn_str) = msg;
        self.dispatch(ProxyAction::ConnectTo(conn_str), ctx);
    }
}

impl Handler<BackboneConnectionReset> for BackboneActor {
    type Result = ();
    fn handle(&mut self, _: BackboneConnectionReset, ctx: &mut Context<BackboneActor>) {
        self.dispatch(ProxyAction::Reconnect, ctx);
    }
}

impl Handler<SpawnChannel> for BackboneActor {
    type Result = SpawnChannelResponse;
    fn handle(&mut self, _: SpawnChannel, _: &mut Context<BackboneActor>) -> SpawnChannelResponse {
        self.open_channel()
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
                let action = self.try_connect();
                self.dispatch(action, ctx);
            },
            ProxyState::WaitsForRetry(..) => {
                warn!("Running sheduled restart...");
                ctx.run_later(Duration::new(15, 0), move |_act, ctx| {
                    ctx.address().do_send(BackboneConnectionReset);
                });                
                self.issue_system_async(BcConnectionLost);                
            },
            ProxyState::Connected(..) => {
                // issue_async comes from having the `BrokerIssue` trait in scope.
                self.issue_system_async(BcConnectionEstablished);                
            },
            ProxyState::NonRecoverable(..) => {
                // issue_async comes from having the `BrokerIssue` trait in scope.
                self.issue_system_async(BcConnectionLost);                
            },
            ProxyState::Dummy => {},
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
        // match conn {
        //     Ok(ref mut conn) => conn.open_channel(None)?,
        //     _ => Err(SvcError::ConnectionError)
        // }

    }

}


#[derive(Default)]
pub struct ChannelActor {
    channel: Option<Channel>
}

impl Actor for ChannelActor {
    type Context = SyncContext<Self>;
}

impl Handler<BcConnectionEstablished> for ChannelActor {
    type Result = ();
    
    fn handle(&mut self, _: BcConnectionEstablished, _ctx: &mut SyncContext<ChannelActor>) {
        let backbone = BackboneActor::from_registry();
        backbone.send(SpawnChannel)
            .and_then(|resp: SpawnChannelResponse| {
                match resp {
                    Ok(channel) => {
                        self.channel = Some(channel)
                    },
                    Err(e) => {
                        error!("Error creating channel: {:?}", e);
                        self.channel = None
                    }
                };
                Ok(())
            });
    }

}

impl Handler<BcConnectionLost> for ChannelActor {
    type Result = ();
    
    fn handle(&mut self, _: BcConnectionLost, _ctx: &mut SyncContext<ChannelActor>) {
        self.channel = None
    }

}

impl Handler<AmqpMessage> for ChannelActor {
    type Result = AmqpMessageResponse;
    
    fn handle(&mut self, msg: AmqpMessage, _ctx: &mut SyncContext<ChannelActor>) -> AmqpMessageResponse {
        match self.channel {
            Some(ref channel) => {
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
            },
            None => {
                return Err(SvcError::MessageError("Proxy not connected".to_string()))
            }
        }
    }

}

// impl ChannelActor {
//     pub fn start_instance() -> Addr<ChannelActor> {
//         ChannelActor::default().start()
//     }
// }


// ==============



///////////////////
/// 
/// 

pub struct ClientPool {
    pub addr: Recipient<AmqpMessage>
}


#[inline]
fn handle_parts(client: Recipient<AmqpMessage>, payload: Bytes, http_ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    let msg = AmqpMessage(payload, http_ctx.into_inner());
    client.send(msg)
        // .from_err::<actix::MailboxError>()
        .map_err(SvcError::from)
        .and_then(|res| {
            if let Err(e) = res {
                match e {
                    // SvcError::AmqpSendError(e) => {
                    //     let text = format!("Send error: {:?}", e);
                    //     let payload = Bytes::from(text);
                    //     let context = MsgContext{
                    //         content_type: "text/plain".to_string(), 
                    //         headers: HashMap::new(), 
                    //         route: "fairways.failure".to_string()
                    //     };
                    //     let msg = AmqpMessage(payload, context);
                    //     AmqpServerProxy::from_registry().do_send(msg)
                    // },
                    _ => {}
                }
                
            }; 
            Ok(())           
        })
        // .map_err(|e|{
        //     match e {

        //     }
        //     e
        // })
        // .from_err()
        .then(|_| {
            // match res {
            //     Ok(_) => {}.
            //     Err(e) => {

            //     }
            // };
            // // fut_ok(recipient.unwrap())
            // match recipient {
            //     Some(r) => Either::A(fut_ok(r)),
            //     None => Either::B(fut_err(SvcError::NotFound {}))
            // }
            Ok(HttpResponse::Ok().body(""))
        })
        // fut_ok(HttpResponse::Ok().body(""))
}

pub fn by_get(client: web::Data<ClientPool>, payload: HttpGetPayload, http_ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    // let proxy = state.lock().unwrap();
    handle_parts(client.addr.clone(), payload.into_inner(), http_ctx)
}

pub fn by_post(client: web::Data<ClientPool>, payload: HttpPostPayload, http_ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    handle_parts(client.addr.clone(), payload.into_inner(), http_ctx)
}



#[cfg(test)]
mod tests {
    use super::*;
    // use actix_web::test;

    use actix_web::dev::Service;
    use actix_web::{test, web, App, web::Query, web::Payload, web::Bytes};


    static BODY: &[u8] = b"a_post=1&b_bost=2";
    static QUERY: &str = "";

    fn handle_get(payload: HttpGetPayload, ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
        println!("*******************************> 0; {:?}", ctx);
        let evt = MetaEvent{
            ctx: ctx.into_inner(),
            payload: payload.into_inner()
        };
        println!("*******************************> 3; {:?}", evt);
        fut_ok(HttpResponse::Ok().body("Ok"))
    }

    fn handle_post(payload: HttpPostPayload, ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
        println!("*******************************> 0; {:?}", ctx);
        let evt = MetaEvent{
            ctx: ctx.into_inner(),
            payload: payload.into_inner()
        };
        println!("*******************************> 3; {:?}", evt);
        fut_ok(HttpResponse::Ok().body("Ok"))
    }

    #[test]
    fn test_from_post() {
        let mut app = test::init_service(
            App::new().route("/{tail:.*}", web::post().to_async(handle_post)));
        let req = test::TestRequest::post().uri("/myroute").set_payload(BODY)
            .to_request();

        let resp = test::block_on(app.call(req)).unwrap();
        println!("*******************************> {:?}", resp);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[test]
    fn test_from_get() {
        let mut app = test::init_service(
            App::new().route("/{tail:.*}", web::get().to_async(handle_get)));
        let req = test::TestRequest::get().uri("/myroute?a=1&b=2")
            .to_request();

        let resp = test::block_on(app.call(req)).unwrap();
        println!("*******************************> {:?}", resp);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }
}