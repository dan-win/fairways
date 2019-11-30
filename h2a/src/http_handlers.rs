use actix::prelude::*;

use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{Client, ConnectionProperties, Channel};

use futures::future::{Future, FutureResult, IntoFuture, result, ok as fut_ok, err as fut_err, finished, Either, join_all};

use std::collections::HashMap;

use core::models::{Context, MetaEvent};

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
    inner: Context,
}

impl HttpEventContext {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> Context {
        self.inner
    }    
}
impl FromRequest for HttpEventContext {
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
        let route = req.path().to_string();
        let content_type = req.headers().get("content-type").unwrap_or(&HEADER_CT_URLENCODED).to_str().unwrap().to_string();
        let headers: HashMap<String, String> = HashMap::new();
        // let payload = web::Bytes::from(req.query_string());
        // let query_string = req.query_string().to_string();

        let inner = Context {
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

        // payload.map_err(Error::from)
        //     .fold(web::BytesMut::new(), move |mut body, chunk| {
        //         body.extend_from_slice(&chunk);
        //         Ok::<_, Error>(body)
        //     })
        //     .and_then(|body| {
        //         format!("Body {:?}!", body);
        //         Ok(HttpResponse::Ok().finish())
        //     })

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
                    // Err(error::ErrorBadRequest("overflow"))
                    Err(error::PayloadError::Overflow)
                } else {
                    body.extend_from_slice(&chunk);
                    Ok(body)
                }
            })
            .map(|body| body.freeze())
            .and_then(|payload|{

                let inner = payload;
                    // payload: web::Bytes::from(body)

                Ok(HttpPostPayload{inner})

            }))
            // Either::A(Box::new(f))
            // Box::new(f)
            // // `Future::and_then` can be used to merge an asynchronous workflow with a
            // // synchronous workflow
            // .and_then(|body| {
            //     // body is loaded, now we can deserialize serde-json

            //     let inner = MetaEvent {
            //         content_type,
            //         route,
            //         headers,
            //         payload: body
            //     };

            //     Ok(HttpPostPayload{inner})
        // Ok(Headers{ inner })
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
/// 
pub fn by_get(evt: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    fut_ok(HttpResponse::Ok().body(""))
}

pub fn by_post(evt: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
    fut_ok(HttpResponse::Ok().body(""))
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
        // let mut evt = evt.into_inner();
        // println!("*******************************> 1");
        // let v8 = q.into_inner().into_bytes();
        // println!("*******************************> 2");
        // evt.payload = Some(Bytes::from(v8));
        // println!("*******************************> 3; {:?}", evt);
        // // format!("{:?}", evt)
        fut_ok(HttpResponse::Ok().body("Ok"))
    }

    fn handle_post(payload: HttpPostPayload, ctx: HttpEventContext) -> impl Future<Item = HttpResponse, Error = Error> {
        println!("*******************************> 0; {:?}", ctx);
        let evt = MetaEvent{
            ctx: ctx.into_inner(),
            payload: payload.into_inner()
        };
        println!("*******************************> 3; {:?}", evt);
        // payload.map_err(Error::from)
        //     .fold(web::BytesMut::new(), move |mut body, chunk| {
        //         body.extend_from_slice(&chunk);
        //         Ok::<_, Error>(body)
        //     })
        //     .and_then(|body| {
        //         // format!("Body {:?}!", evt);
        //         let mut evt = evt.into_inner();
        //         evt.payload = body.freeze();
        //         println!("*******************************> {:?}", evt);
        //         let s = format!("{:?}", evt);
        //         Ok(HttpResponse::Ok().body(s))
        //     })
        fut_ok(HttpResponse::Ok().body("Ok"))
    }

    #[test]
    fn test_from_post() {
        let mut app = test::init_service(
            App::new().route("/", web::post().to_async(handle_post)));
        let req = test::TestRequest::post().uri("/").set_payload(BODY)
            .to_request();

        // let resp = test::block_on(index(req)).unwrap();
        let resp = test::block_on(app.call(req)).unwrap();
        println!("*******************************> {:?}", resp);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[test]
    fn test_from_get() {
        let mut app = test::init_service(
            App::new().route("/", web::get().to_async(handle_get)));
        let req = test::TestRequest::get().uri("/?a=1&b=2")
            .to_request();

        // let resp = test::block_on(index(req)).unwrap();
        let resp = test::block_on(app.call(req)).unwrap();
        println!("*******************************> {:?}", resp);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }
}