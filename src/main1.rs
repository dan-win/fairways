extern crate futures;

extern crate lapin_futures;

// extern crate fastlog;
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate websocket;

extern crate reqwest;

use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use websocket::client::ClientBuilder;
use websocket::{Message, OwnedMessage};

use futures::{Future, Stream};
use lapin_futures as lapin;
use crate::lapin::{Client, ConnectionProperties};
use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;

use log::{debug, info};

const CONNECTION: &'static str = "ws://127.0.0.1:8765/ws";

fn main() {
    env_logger::init();
    ::std::env::set_var("RUST_LOG", "debug");

    // Get announce:
    let bc_routes: HashMap<String, String> = reqwest::blocking::get("http://127.0.0.1:8765/announce")?
        .json()?;
    println!("{:#?}", resp);

    // let mut pools: Vec<

    // ----------------------------
	println!("Connecting to {}", CONNECTION);

	let client = ClientBuilder::new(CONNECTION)
		.unwrap()
		.add_protocol("rust-websocket")
		.connect_insecure()
		.unwrap();

	println!("Successfully connected");

	let (mut receiver, mut sender) = client.split().unwrap();

	let (tx, rx) = channel();

    let tx_1 = tx.clone();

	let send_loop = thread::spawn(move || {
		loop {
			// Send loop
			let message = match rx.recv() {
				Ok(m) => m,
				Err(e) => {
					println!("Send Loop: {:?}", e);
					return;
				}
			};
			match message {
				OwnedMessage::Close(_) => {
					let _ = sender.send_message(&message);
					// If it's a close message, just send it and then return.
					return;
				}
				_ => (),
			}
			// Send the message
			match sender.send_message(&message) {
				Ok(()) => (),
				Err(e) => {
					println!("Send Loop: {:?}", e);
					let _ = sender.send_message(&Message::close());
					return;
				}
			}
		}
	});

	let receive_loop = thread::spawn(move || {
		// Receive loop
		for message in receiver.incoming_messages() {
			let message = match message {
				Ok(m) => m,
				Err(e) => {
					println!("Receive Loop: {:?}", e);
					let _ = tx_1.send(OwnedMessage::Close(None));
					return;
				}
			};
			match message {
				OwnedMessage::Close(_) => {
					// Got a close message, so send a close message and return
					let _ = tx_1.send(OwnedMessage::Close(None));
					return;
				}
				OwnedMessage::Ping(data) => {
					match tx_1.send(OwnedMessage::Pong(data)) {
						// Send a pong in response
						Ok(()) => (),
						Err(e) => {
							println!("Receive Loop: {:?}", e);
							return;
						}
					}
				}
				// Say what we received
				_ => println!("Receive Loop: {:?}", message),
			}
		}
	});

  // ----------------------------

  let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://houston:houston@127.0.0.1:5672/%2f".into());

    futures::executor::spawn(
        Client::connect(&addr, ConnectionProperties::default()).and_then(|client| {
            // create_channel returns a future that is resolved
            // once the channel is successfully created
            client.create_channel()
        }).and_then(|channel| {
            let id = channel.id();
            info!("created channel with id: {}", id);

            let ch = channel.clone();
            channel.queue_declare(
                "hello", 
                QueueDeclareOptions{durable: true, ..Default::default()}, 
                FieldTable::default())
            .and_then(move |queue| {
                info!("channel {} declared queue {}", id, "hello");

                // basic_consume returns a future of a message
                // stream. Any time a message arrives for this consumer,
                // the for_each method would be called
                channel.basic_consume(&queue, "my_consumer", BasicConsumeOptions::default(), FieldTable::default())
            }).and_then(|stream| {
                info!("got consumer stream");

                stream.for_each(move |message| {
                    debug!("got message: {:?}", message);
                    info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
                    
                    let body = std::str::from_utf8(&message.data).unwrap();
                    let msg = OwnedMessage::Text(body.to_string());
                    match tx.send(msg) {
                        Ok(()) => (),
                        Err(e) => {
                            info!("Error sending WS message: {:?}", e);
                        }
                    }
                    ch.basic_ack(message.delivery_tag, false)
                })
            })
        })
    ).wait_future().expect("runtime failure");

    // We're exiting

    println!("Waiting for child threads to exit");
    let _ = send_loop.join();
    let _ = receive_loop.join();

    println!("Exited");
}

