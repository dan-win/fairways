use actix::prelude::*;

use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{Client, ConnectionProperties, Channel};

use collections::HashMap;

use core::models::{MetaEvent};

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
    web::Path, web::Query, web::Json, 
    // web::Payload,
};

use error::Error;

pub struct HttpGetEvent {
    inner: MetaEvent
}

pub struct HttpGetEvent {
    inner: MetaEvent
}


impl HttpPostEvent {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> MetaEvent {
        self.inner
    }    
}

impl HttpGetEvent {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> MetaEvent {
        self.inner
    }    
}

impl FromRequest for HttpPostEvent {
    type Config = ();
    // type Result = Result<Self, Error>;

    type Error = Error;
    type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future: IntoFuture<Item = Self, Error = Self::Error>

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let route = req.path();
        let content_type = req.headers().get("content-type").unwrap_or("application/json").to_string();
        let headers: HashMap<String, String> = HashMap::new();

        // payload is a stream of Bytes objects
        payload
            // `Future::from_err` acts like `?` in that it coerces the error type from
            // the future into the final error type
            .from_err()
            // `fold` will asynchronously read each chunk of the request body and
            // call supplied closure, then it resolves to result of closure
            .fold(BytesMut::new(), move |mut body, chunk| {
                // limit max size of in-memory payload
                if (body.len() + chunk.len()) > MAX_SIZE {
                    Err(error::ErrorBadRequest("overflow"))
                } else {
                    body.extend_from_slice(&chunk);
                    Ok(body)
                }
            })
            // `Future::and_then` can be used to merge an asynchronous workflow with a
            // synchronous workflow
            .and_then(|body| {
                // body is loaded, now we can deserialize serde-json

                let inner = MetaEvent {
                    content_type,
                    route,
                    headers,
                    payload: Bytes::from(&body)
                };

                Ok(HttpPostEvent{inner})
            })
        // Ok(Headers{ inner })
    }
}



impl FromRequest for HttpGetEvent {
    type Config = ();
    // type Result = Result<Self, Error>;

    type Error = Error;
    type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future = Result<Self, Self::Error>;
    // type Config = QueryConfig;
    // type Future: IntoFuture<Item = Self, Error = Self::Error>

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let route = req.path();
        let content_type = req.headers().get("content-type").unwrap_or("application/json").to_string();
        let headers: HashMap<String, String> = HashMap::new();
        let payload = Bytes::from(req.query_string());

        let inner = MetaEvent {
            content_type,
            route,
            headers,
            payload
        };

        Ok(HttpGetEvent{inner})
    }
}


///////////////////


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
    use actix_web::test;

    static body: &str = b"123";
    static query: &str = "";

    fn handle_get(evt: HttpGetEvent) -> String {
        format!("{:?}", evt)
    }

    fn handle_post(evt: HttpPostEvent) -> String {
        format!("{:?}", evt)
    }

    #[test]
    fn test_from_post() {
        let mut app = test::init_service(App::new().route("/", web::post().to(handle_post)));
        let req = test::TestRequest::post().uri("/").set_payload(body)
            .to_request();

        // let resp = test::block_on(index(req)).unwrap();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[test]
    fn test_from_get() {
        let mut app = test::init_service(App::new().route("/", web::post().to(handle_get)));
        let req = test::TestRequest::post().uri("/").param("a", "1").param("b", "2")
            .to_request();

        // let resp = test::block_on(index(req)).unwrap();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
    }
}