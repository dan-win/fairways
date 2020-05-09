use actix::prelude::*;
use actix_broker::{BrokerIssue, BrokerSubscribe};
use std::convert::From;
use std::env;
use std::time::Duration;
// use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
// use crate::lapin::types::FieldTable;
// use crate::lapin::{Client, ConnectionProperties, Channel};

use amiquip::{
    AmqpProperties, Channel, ConfirmSmoother, Connection, Exchange, Publish, QueueDeclareOptions,
    Result as AmiquipResult,
};

use futures::future::{
    err as fut_err, finished, join_all, ok as fut_ok, result, Either, Future, FutureResult,
    IntoFuture,
};

use std::collections::HashMap;

use actix_web::{
    error,
    // get, // <- here is "decorator"
    http,
    // FutureResponse,
    // web::Data,
    web,
    web::Json,
    web::Path,
    web::PayloadConfig,
    // web::Payload,
    web::Query,
    FromRequest,
    // middleware,
    // HttpServer,
    // App, Error as AWError, HttpMessage, ResponseError,
    HttpRequest,
    HttpResponse,
};

use actix_web::dev; // <--- for dev::Payload

use actix_web::web::Bytes;

use error::Error;

use crate::messages::{
    AmqpFailMessage, AmqpMessage, AmqpMessageResponse, BackboneConnectTo, BackboneConnectionReset,
    BcConnectionEstablished, BcConnectionLost, NeedsConnection, SpawnChannel, SpawnChannelResponse,
};

use crate::actors;
use crate::errors::SvcError;

use fairways_core::models::{Context as MsgContext, MetaEvent};
// use std::sync::mpsc;
use uuid;

const LOCAL_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const MAX_SIZE: usize = 262_144;

lazy_static! {
    static ref HEADER_CT_JSON: http::header::HeaderValue =
        http::header::HeaderValue::from_static("application/json");
    static ref HEADER_CT_URLENCODED: http::header::HeaderValue =
        http::header::HeaderValue::from_static("x-form/urlencoded");
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
        let content_type = req
            .headers()
            .get("content-type")
            .unwrap_or(&HEADER_CT_URLENCODED)
            .to_str()
            .unwrap()
            .to_string();
        let headers: HashMap<String, String> = HashMap::new();

        let inner = MsgContext {
            content_type,
            route,
            headers,
        };

        Ok(HttpEventContext { inner })
    }
}

#[derive(Debug)]
pub struct HttpGetPayload {
    inner: Bytes,
}

#[derive(Debug)]
pub struct HttpPostPayload {
    inner: Bytes,
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
    type Future = Box<dyn Future<Item = HttpPostPayload, Error = Error>>;
    // Either<Box<dyn Future<Item = HttpPostPayload, Error = Error> + 'static>, FutureResult<HttpPostPayload, Error>>;

    #[inline]
    fn from_request(_req: &HttpRequest, payload: &mut dev::Payload) -> Self::Future {
        // payload is a stream of Bytes objects
        Box::new(
            payload
                .take()
                // `Future::from_err` acts like `?` in that it coerces the error type from
                // the future into the final error type
                .from_err()
                // `fold` will asynchronously read each chunk of the request body and
                // call supplied closure, then it resolves to result of closure
                .fold(
                    web::BytesMut::with_capacity(MAX_SIZE),
                    move |mut body, chunk| {
                        // limit max size of in-memory payload
                        if (body.len() + chunk.len()) > MAX_SIZE {
                            Err(error::PayloadError::Overflow)
                        } else {
                            body.extend_from_slice(&chunk);
                            Ok(body)
                        }
                    },
                )
                .map(|body| body.freeze())
                .and_then(|payload| {
                    let inner = payload;

                    Ok(HttpPostPayload { inner })
                }),
        )
    }
}

impl FromRequest for HttpGetPayload {
    type Config = ();

    type Error = Error;
    type Future = Result<Self, Self::Error>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut dev::Payload) -> Self::Future {
        let inner = web::Bytes::from(req.query_string());

        Ok(HttpGetPayload { inner })
    }
}

#[inline]
fn handle_parts(
    client: Recipient<AmqpMessage>,
    payload: Bytes,
    http_ctx: HttpEventContext,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let msg = AmqpMessage(payload, http_ctx.into_inner());
    client.send(msg)
        // .from_err::<actix::MailboxError>()
        .map_err(SvcError::from)
        .and_then(|res| {
            if let Err(e) = res {
                debug!("-- Some error occurs: {}, processing... ", e);
                match e {
                    SvcError::TargetNotFound(_) => {},
                    SvcError::ConnectionError(e)  => {
                        debug!("-- ConnectionError during http request hndling {}: restarting [Err], mailbox error ", e);
                        actors::BackboneActor::from_registry().do_send(BackboneConnectionReset);
                    },
                    SvcError::ChannelError(_) => {},
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
                    SvcError::NonRecoverableError(_) => {

                    },
                    _ => {}
                };
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

pub fn by_get(
    client: web::Data<actors::ClientPool>,
    payload: HttpGetPayload,
    http_ctx: HttpEventContext,
) -> impl Future<Item = HttpResponse, Error = Error> {
    // let proxy = state.lock().unwrap();
    handle_parts(client.addr.clone(), payload.into_inner(), http_ctx)
}

pub fn by_post(
    client: web::Data<actors::ClientPool>,
    payload: HttpPostPayload,
    http_ctx: HttpEventContext,
) -> impl Future<Item = HttpResponse, Error = Error> {
    handle_parts(client.addr.clone(), payload.into_inner(), http_ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    // use actix_web::test;

    use actix_web::dev::Service;
    use actix_web::{test, web, web::Bytes, web::Payload, web::Query, App};

    static BODY: &[u8] = b"a_post=1&b_bost=2";
    static QUERY: &str = "";

    fn handle_get(
        payload: HttpGetPayload,
        ctx: HttpEventContext,
    ) -> impl Future<Item = HttpResponse, Error = Error> {
        println!("*******************************> 0; {:?}", ctx);
        let evt = MetaEvent {
            ctx: ctx.into_inner(),
            payload: payload.into_inner(),
        };
        println!("*******************************> 3; {:?}", evt);
        fut_ok(HttpResponse::Ok().body("Ok"))
    }

    fn handle_post(
        payload: HttpPostPayload,
        ctx: HttpEventContext,
    ) -> impl Future<Item = HttpResponse, Error = Error> {
        println!("*******************************> 0; {:?}", ctx);
        let evt = MetaEvent {
            ctx: ctx.into_inner(),
            payload: payload.into_inner(),
        };
        println!("*******************************> 3; {:?}", evt);
        fut_ok(HttpResponse::Ok().body("Ok"))
    }

    #[test]
    fn test_from_post() {
        let mut app =
            test::init_service(App::new().route("/{tail:.*}", web::post().to_async(handle_post)));
        let req = test::TestRequest::post()
            .uri("/myroute")
            .set_payload(BODY)
            .to_request();

        let resp = test::block_on(app.call(req)).unwrap();
        println!("*******************************> {:?}", resp);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[test]
    fn test_from_get() {
        let mut app =
            test::init_service(App::new().route("/{tail:.*}", web::get().to_async(handle_get)));
        let req = test::TestRequest::get()
            .uri("/myroute?a=1&b=2")
            .to_request();

        let resp = test::block_on(app.call(req)).unwrap();
        println!("*******************************> {:?}", resp);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }
}
