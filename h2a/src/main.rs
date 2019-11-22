#[macro_use]
extern crate actix;
// extern crate actix_broker;
// extern crate actix_lua;
extern crate actix_web;
extern crate futures;
// extern crate future_union;
// extern crate num_cpus;
use error::Error;

// extern crate fastlog;
#[macro_use]
extern crate log;
extern crate env_logger;
use log::Level;

use futures::future::{Future, IntoFuture, result, ok as fut_ok, err as fut_err, finished, Either, join_all};
// use future_union::future_union;

#[macro_use] extern crate serde;
#[macro_use] extern crate serde_derive; 
// #[macro_use] extern crate serde_json;
// #[macro_use] extern crate actix_derive;

// extern crate itertools;

use actix_web::dev::Payload; // <--- for dev::Payload
// use actix_broker::{BrokerSubscribe, BrokerIssue};
// use actix_redis::{Command, RedisActor, Error as ARError}; 
use actix_web::{
    get, // <- here is "decorator"
    http, 
    error,
    middleware, 
    HttpServer, 
    App, Error as AWError, HttpMessage, ResponseError,
    HttpRequest, HttpResponse,
    FromRequest,
    // FutureResponse, 
    // web::Data, 
    web,
    web::Path, web::Query, web::Json, 
    // web::Payload,
};

use actix_cors::Cors;

// ---

fn main() -> std::io::Result<()> {
    dotenv().ok();

    
    ::std::env::set_var("RUST_LOG", "error,actix_web=error,main=error");
    env_logger::init();

    // let port = env::args().nth(1).ok_or("Port parameter is invalid or missed!".to_owned()).and_then(|arg| arg.parse::<u32>().map_err(|err| err.to_string())).unwrap();
    let port = match env::args().nth(1) {
        Some(arg) => arg.parse::<u32>().map_err(|err| err.to_string()).unwrap(),
        _ => 8010,
    };

    let srv_addr = format!("0.0.0.0:{}", port);

    let X_COOKIE_HEADER: &'static str = "x-cookie";

    let sys = actix::System::new("http2buff");
    println!("Starting http server: {}", &srv_addr);
    
    let registry_addr = RoutesRegistry::from_registry();
    
    // TO_DO: SyncContext: Context.set_mailbox_capacity() up from 16!
    HttpServer::new(move || {
        
        let shared_data = web::Data::new(AppState{
            // router: Arbiter::start(move|_| Router::new())
            router: Router::new().start()
            // buffers
        });
    
        fn add_cors() {
            // Cors::default()
            Cors::new()
                // .allowed_origin("*") // <- Avoid credentials in client!!! See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin
                .allowed_methods(vec![
                    "HEAD",
                    "OPTIONS",
                    "GET", 
                    "POST",
                    "DELETE"
                    ])
                .allowed_headers(vec![
                    http::header::ACCEPT,
                    http::header::CONTENT_TYPE,
                    http::header::CONTENT_LENGTH,
                    // http::header::USER_AGENT,
                    http::header::ORIGIN, 
                    // http::header::AUTHORIZATION, 
                    http::header::COOKIE, 
                    http::header::HeaderName::from_static(X_COOKIE_HEADER),
                    ])
                .supports_credentials()
                .max_age(3600)        
        }
        
        App::new().register_data(shared_data.clone())
            .wrap(
                add_cors()
            )
            .configure(config_app)
            // enable logger
            .wrap(middleware::Logger::default())
    
    }).bind(srv_addr)
        .unwrap()
        .keep_alive(5)
        // .workers(4)
        .shutdown_timeout(1)
        .start();

    Ok(())
}

