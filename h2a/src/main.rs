#[macro_use]
extern crate actix;
extern crate actix_broker;
// extern crate actix_lua;
extern crate actix_web;
extern crate futures;
// extern crate future_union;
// extern crate num_cpus;
use error::Error;

#[macro_use]
extern crate lazy_static;

// extern crate fastlog;
#[macro_use]
extern crate log;
extern crate env_logger;
use log::Level;

use futures::future::{
    err as fut_err, finished, join_all, ok as fut_ok, result, Either, Future, IntoFuture,
};
// use future_union::future_union;

#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_derive;
// #[macro_use] extern crate serde_json;
// #[macro_use] extern crate actix_derive;

#[macro_use]
extern crate failure;

// extern crate itertools;

// extern crate lapin_futures;
// use lapin_futures as lapin;

extern crate amiquip;

extern crate dotenv;
use dotenv::dotenv;
use std::env;

// use actix_broker::{BrokerSubscribe, BrokerIssue};
// use actix_redis::{Command, RedisActor, Error as ARError};
use actix_web::{
    error,
    get, // <- here is "decorator"
    http,
    middleware,
    // FutureResponse,
    // web::Data,
    web,
    web::Json,
    // web::Payload,
    web::Path,
    web::Query,
    App,
    Error as AWError,
    FromRequest,
    HttpMessage,
    HttpRequest,
    HttpResponse,
    HttpServer,
    ResponseError,
};

extern crate uuid;
// ---
// struct handlers {};

// impl handlers {

// }
// ---

extern crate argparse;

use argparse::{ArgumentParser, StoreOption};

mod messages;
use messages::BackboneConnectTo;
mod errors;

mod appconfig;
use appconfig::{config_app, cors_middleware};

// mod http_handlers;
mod actors;
mod extractors;
// use http_handlers;
extern crate fairways_core;

use derive_more::Display; // naming it clearly for illustration purposes

use std::sync::mpsc;
use std::thread;

lazy_static! {
    static ref DEFAULT_HTTP_HOST: String = "0.0.0.0".to_string();
    static ref DEFAULT_HTTP_PORT: u32 = 12000;
    static ref DEFAULT_AMQP_CONN_STR: String = "amqp://guest:guest@127.0.0.1:5672".to_string();
}

fn main() -> std::io::Result<()> {
    dotenv().ok();

    let implement_cors = false;

    let mut port: Option<u32> = None; // 12000;
    let mut host: Option<String> = None; // "0.0.0.0".to_string();

    // ::std::env::set_var("RUST_LOG", "debug,actix_web=error,main=debug,http_handlers=debug");
    env_logger::init();

    {
        // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("HTTP->AMQP bridge");
        ap.refer(&mut port)
            .add_option(&["-p", "--port"], StoreOption, "Port to listen");

        ap.refer(&mut host)
            .add_option(&["-h", "--host"], StoreOption, "Host to listen");

        // ap.stop_on_first_argument(true);
        ap.parse_args_or_exit();
    }

    let srv_addr = format!(
        "{}:{}",
        host.unwrap_or(DEFAULT_HTTP_HOST.clone()),
        port.unwrap_or(*DEFAULT_HTTP_PORT)
    );
    let conn_str = env::var("FWS_AMQP_CONN_STR").unwrap_or(DEFAULT_AMQP_CONN_STR.clone());

    let (tx, rx) = mpsc::channel();

    let server_thread = thread::spawn(move || {
        let sys = actix::System::new("h2a");
        println!("Starting http server: {}", &srv_addr);

        let proxy_addr = actors::BackboneActor::load_from_registry();

        // let state = actix::SyncArbiter::start(10, || {
        //     actors::ChannelActor::default().start()
        // }).recipient::<messages::AmqpMessage>();

        HttpServer::new(move || {
            let app = App::new()
                .data(actors::ClientPool {
                    addr: actix::SyncArbiter::start(4, || actors::ChannelActor::default())
                        .recipient::<messages::AmqpMessage>(),
                })
                // addr: actors::ChannelActor::start_instance().recipient::<messages::AmqpMessage>()
                .configure(config_app)
                // enable logger
                .wrap(middleware::Logger::default());

            // if implement_cors {
            //     app = app.wrap(cors_middleware());
            // }

            app
        })
        .bind(srv_addr)
        .unwrap()
        .keep_alive(5)
        // .workers(4)
        .shutdown_timeout(1)
        .start();

        let _ = tx
            .send(proxy_addr)
            .expect("Cannot send registry address to channel");
        let _ = sys.run();
    });

    let proxy_addr = rx
        .recv()
        .expect("Cannot obtain proxy addr - whether it running now?");
    let _ = proxy_addr
        .send(BackboneConnectTo(conn_str))
        .wait()
        .expect("Proxy: cannot establish connection to AMQP server");

    server_thread.join().expect("Error in the main thread");

    Ok(())
}
