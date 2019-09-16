extern crate futures;

extern crate lapin_futures;

extern crate log;
extern crate env_logger;

extern crate reqwest;

#[macro_use] extern crate serde;
extern crate serde_json;

extern crate argparse;

use argparse::{ArgumentParser, Store};

use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{Client, ConnectionProperties};
use futures::{Future, Stream};
use lapin_futures as lapin;
use log::{info, debug, error};

use std::collections::HashMap;
use std::thread;
use std::sync::mpsc::{channel, Sender};

#[derive(Serialize, Clone, Debug)]
struct Payload {
    pub uri: String,
    pub source: String,
    pub body: String,
    // pub ts: String
}

fn create_consumer(client: &Client, queue_name: String, bc_uri: String, tx: Sender<Payload>) -> impl Future<Item = (), Error = ()> + Send + 'static {
    debug!("will create consumer {}", &queue_name);

    let qn1 = queue_name.clone();
    let qn2 = queue_name.clone();
    let qn3 = queue_name.clone();
    let uri = bc_uri.clone();

    client
        .create_channel()
        .and_then(move |channel| {
            info!("creating queue {}", &queue_name);
            channel
                .queue_declare(
                    &queue_name,
                    QueueDeclareOptions{durable: true, ..Default::default()},
                    FieldTable::default(),
                )
                .map(move |queue| (channel, queue))
        })
        .and_then(move |(channel, queue)| {
            debug!("creating consumer {}", &qn1);
            channel
                .basic_consume(
                    &queue,
                    "",
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .map(move |stream| (channel, stream))
        })
        .and_then(move |(channel, stream)| {
            debug!("got stream for consumer {}", &qn2);
            stream.for_each(move |message| {
                let body = std::str::from_utf8(&message.data).unwrap();
                debug!(
                    "consumer '{}' got '{}'",
                    &qn2,
                    body
                );
                let payload = Payload {
                    uri: uri.clone(),
                    source: qn2.clone(),
                    body: String::from(body)
                };

                match tx.send(payload) {
                    Ok(()) => (),
                    Err(e) => {
                        error!("tx error: {:?}", e)
                    }
                }

                channel.basic_ack(message.delivery_tag, false)
            })
        })
        .map(|_| ())
        .map_err(move |err| eprintln!("got error in consumer '{}': {}", &qn3, err))
}


fn main() -> Result<(), reqwest::Error> {
    env_logger::init();

    let mut amqp_addr = "amqp://user:password@localhost:5672/%2f".to_string();
    let mut backend_addr = "http://localhost:8080".to_string();
    let mut digest_path = "".to_string();

    {  // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("AMQP bridge");
        ap.refer(&mut amqp_addr)
            .add_option(&["-q", "--amqp"], Store,
            "Full url of AMQP broker, e.g.: 'amqp://user:password@localhost:5672/%2f'").required();
        ap.refer(&mut backend_addr)
            .add_option(&["-b", "--backend"], Store,
            "Full url of backend formatted like: 'http://localhost:8080'").required();
        ap.refer(&mut digest_path)
            .add_option(&["-d", "--digest"], Store,
            "Path of digest endpoint which returns hash queue->url, e.g.: '/digest'");
        ap.parse_args_or_exit();
    }

    let digest_uri = format!("{}{}", backend_addr, digest_path);
    let bc_routes: HashMap<String, String> = reqwest::get(&digest_uri).expect("Cannot connect to digest endpoint").json().expect("Cannot decode digest response");

    let bc_client = reqwest::Client::new();

    let (tx, rx) = channel();

    let send_loop = thread::spawn(move || {
        loop {
            let message: Payload = match rx.recv() {
                Ok(m) => m,
                Err(e) => {
                    error!("RX error: {:?}", e);
                    continue
                }
            };
            let target_uri = format!("{}{}", backend_addr, message.uri);
            let resp = bc_client.post(&target_uri)
                .json(&message)
                .send();
            
            match resp {
                Ok(_) =>    debug!("Data sent to backend"),
                Err(e) =>   error!("Backend error: {:?}", e),
            }

        }
    });

    futures::executor::spawn(
        Client::connect(&amqp_addr, ConnectionProperties::default())
            .map_err(|err| eprintln!("An error occured: {}", err))
            .map(|client| {

                for (queue_name, bc_uri) in &bc_routes {
                    let _client = client.clone();
                    let tx1 = tx.clone();
                    let qn = queue_name.clone();
                    let uri = bc_uri.clone();
                    std::thread::spawn(move || {
                        futures::executor::spawn(create_consumer(&_client, qn, uri, tx1))
                            .wait_future()
                            .expect("consumer failure")
                    });
                }
            })
    )
    .wait_future()
    .expect("runtime exited with failure");

    let _ = send_loop.join();

    info!("Exiting...");

    Ok(())
}