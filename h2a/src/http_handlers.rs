use actix::prelude::*;

use std::convert::From;
// use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
// use crate::lapin::types::FieldTable;
// use crate::lapin::{Client, ConnectionProperties, Channel};

use amiquip::{
    Channel, ConfirmSmoother, Connection, Exchange, Publish, AmqpProperties, QueueDeclareOptions, Result,
};

use futures::future::{Future, FutureResult, IntoFuture, result, ok as fut_ok, err as fut_err, finished, Either, join_all};

use std::collections::HashMap;

use core::models::{Context as MsgContext, MetaEvent};

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

lazy_static! {
    static ref HEADER_CT_JSON: http::header::HeaderValue = http::header::HeaderValue::from_static("application/json");
    static ref HEADER_CT_URLENCODED: http::header::HeaderValue = http::header::HeaderValue::from_static("x-form/urlencoded");
    static ref MAX_SIZE: usize = 262_144;
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
    // type Result = Result<Self, Error>;

    type Error = Error;
    // type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future: IntoFuture<Item = Self, Error = Self::Error>
    type Future =
        Box<dyn Future<Item = HttpPostPayload, Error = Error>>;
        // Either<Box<dyn Future<Item = HttpPostPayload, Error = Error> + 'static>, FutureResult<HttpPostPayload, Error>>;

    // #[inline]
    // fn from_request(req: &HttpRequest, payload: &mut dev::Payload) -> Self::Future {

    //     let limit = 8000;
    //     Either::A(Box::new(
    //         dev::HttpMessageBody::new(req, payload).limit(limit).from_err(),
    //     ))
    // }

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut dev::Payload) -> Self::Future {

        // payload is a stream of Bytes objects
        Box::new(payload.take()
            // `Future::from_err` acts like `?` in that it coerces the error type from
            // the future into the final error type
            .from_err()
            // `fold` will asynchronously read each chunk of the request body and
            // call supplied closure, then it resolves to result of closure
            .fold(web::BytesMut::with_capacity(262_144), move |mut body, chunk| {
                // limit max size of in-memory payload
                if (body.len() + chunk.len()) > *MAX_SIZE {
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
    // type Result = Result<Self, Error>;

    type Error = Error;
    type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future: IntoFuture<Item = Self, Error = Self::Error>

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut dev::Payload) -> Self::Future {
        let inner = web::Bytes::from(req.query_string());

        Ok(HttpGetPayload{inner})
    }
}


///////////////////

#[inline]
fn handle_parts(payload: Bytes, ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    let msg = AmqpMessage(payload, ctx);
    AmqpServerProxy::from_registry()
    .send(msg)
    .from_err()
    .and_then(|recipient: Option<Recipient<Event>>| {
        // // fut_ok(recipient.unwrap())
        // match recipient {
        //     Some(r) => Either::A(fut_ok(r)),
        //     None => Either::B(fut_err(SvcError::NotFound {}))
        // }
        |res| Ok(HttpResponse::Ok().body(""))
    })
    // fut_ok(HttpResponse::Ok().body(""))
}

pub fn by_get(payload: HttpGetPayload, ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    handle_parts(payload.into_inner(), ctx)
}

pub fn by_post(payload: HttpPostPayload, ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    handle_parts(payload.into_inner(), ctx)
}

/// To-do: implement MessageResponse (Option)
#[derive(Clone, Debug, Message)]
pub struct AmqpMessage(Bytes, MsgContext);
// route, message

impl AmqpMessage {
    pub fn new(payload: Bytes, ctx: MsgContext) -> Self {
        AmqpMessage(payload, ctx)
        // let (payload, ctx) = self;
        // let exchange_name: String = ctx.route.clone();
        // let routing_key = "";
        // let properties = AmqpProperties::default()
        //     .with_content_type(ctx.content_type.clone()); // <- later: add .with_headers!
        // let body = payload;
        // let publish = Publish{
        //     body,
        //     routing_key,
        //     mandatory: false, // <- use later to handle errors on non-existing exchanges!
        //     immediate: false,
        //     properties,
        // };
        // AmqpMessage(exchange_name.clone(), publish) // <- to-do: borrow instead of clone
    }
}

impl<'a> From<AmqpMessage> for Publish<'a> {
    pub fn from(msg: &AmqpMessage) -> Publish<'a> {
        let AmqpMessage(payload, ctx) = *msg;
        let exchange_name: String = ctx.route.clone();
        let routing_key = "";
        let properties = AmqpProperties::default()
            .with_content_type(ctx.content_type.clone()); // <- later: add .with_headers!
        let body = payload;
        Publish{
            body,
            routing_key,
            mandatory: false, // <- use later to handle errors on non-existing exchanges!
            immediate: false,
            properties,
        }
    }
}


pub struct AmqpServerProxy {
    ready:bool,
    restarted: bool,
    pub uri: &'static str,
    connection: Option<Connection>,
    endpoints: HashMap<String, Recipient<AmqpMessage>>
}

impl AmqpServerProxy {
    pub fn new(uri: &'static str) -> Self {
        Self {
            ready: false,
            restarted: false,
            uri,
            connection: None,
            endpoint: HashMap::new()
        }
    }
}


impl Actor for AmqpServerProxy {
    type Context = Context<Self>;
}

impl Handler<AmqpMessage> for AmqpServerProxy {
    type Result = ();

    fn handle(&mut self, msg: AmqpMessage, ctx: &mut Context<AmqpServerProxy>) {
        // let AmqpMessage(route, _) = msg;
        let route = msg.0.clone();
        let endpoint = self.endpoints.get(&route);
        let client = match self.endpoints.get(&route) {
            Some(&client) => client,
            _ => {
                // See also: Connection::insecure_open_stream
                let channel = connection.open_channel(None)?;
                // let exchange = Exchange::direct(channel)
                let client = AmqpClient::new(channel, route.clone()).start().recipient();
                self.endpoints.insert(route.clone(), client);
                client
            }
        };
        client.do_send(msg);
    }
}

impl Supervised for AmqpServerProxy {
    fn restarting(&mut self, ctx: &mut Context<AmqpServerProxy>) {
        self.endpoints.clear();
        self.connection = Connection::insecure_open(self.uri)?
    }
}

impl SystemService for AmqpServerProxy {

}

pub struct AmqpClient {
    // exchange: Exchange
    channel: Channel,
    exchange_name: String
}

impl AmqpClient {
    pub fn new(channel: Channel, exchange_name: String) -> Self {
        Self{channel, exchange_name}
    }
}

impl Actor for AmqpClient {
    type Context = Context<Self>;
}


impl Handler<AmqpMessage> for AmqpClient {
    type Result = ();

    fn handle(&mut self, msg: AmqpMessage, ctx: &mut Context<AmqpClient>) {
        let payload = Payload::from(msg);
        // self.exchange.publish(payload)?;
        self.channel.basic_publish(self.exchange_name, payload)?;
    }
}

impl Supervised for AmqpClient {

}



// #[derive(Debug, Deserialize, Serialize, Clone)]
// struct PubConf {
//     uri: String,
//     exchange: String,
//     routing_key: Option<String>,
// }

// // 1 rxtx per queue


// struct AmqpEndpoint {
//     queue: String,
//     routing_key: Option<String>,
// }


// struct AmqpMessageVector {

// }

// //////////////////


// Reused connections pool. Arbiter + SynConnections. 

// pub struct AmqpClient {
//     conn_uri: String,
//     client: Option<Client>,
//     endpoints: Vec<PubConf>,
//     // Route -> Actor
//     channels: HashMap<String, Channel>
// }

// impl AmqpClient {
//     pub fn new(conn_uri: String, endpoints: Vec<PubConf>) -> Self {
//         Self {
//             conn_uri,
//             client: None,
//             endpoints,
//             channels: HashMap::new(),
//         }
//     }
// }

// impl SystemService for AmqpClient {
//     fn service_started(&mut self, ctx: &mut Context<Self>) {
//         self.restarting(ctx)
//         println!("Service started");
//     }
// }

// impl Supervised for AmqpClient {
//     fn restarting(&mut self, ctx: &mut Self::Context) {
//         let client = Client::connect(&self.conn_uri, ConnectionProperties::default()).wait().expect("connection error");
//         self.client = Some(client);
//         self.channels = self.endpoints.iter().map(|conf: PubConf| {()}).collect::<HashMap<String, Channel>>();
//     }    
// }


// pub struct AmqpPublisher {

//     client: Option<Client>,
// }

// impl AmqpPublisher {
//     pub fn new(conn_uri: String) -> Self {
//         AmqpPublisher{
//             conn_uri,
//             client: None
//         }
//     }    
// }


// impl Actor for AmqpPublisher {

//     type Context = Context<Self>;

//     fn started(&mut self, ctx: &mut Self::Context) {



//         // Increase mailbox capacity because this is a sigle instance!
//         ctx.set_mailbox_capacity(128);

//         // These are broadcast messages to all instances of Router:
//         println!("AmqpPublisher started");

//         // self.subscribe_system_async::<messages::GetRoutesSnapshot>(ctx);

//         // self.subscribe_system_async::<RouteChanged>(ctx);
//         // self.subscribe_system_async::<RouteDropped>(ctx);

//     }

// }

// impl Supervised for AmqpPublisher {
//     fn restarting(&mut self, ctx: &mut Self::Context) {

//     }    
// }

// impl SystemService for AmqpPublisher {
//     fn service_started(&mut self, ctx: &mut Context<Self>) {
//         println!("AmqpPublisher started");
//     }
// }

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