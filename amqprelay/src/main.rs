extern crate futures;

extern crate lapin_futures;

extern crate log;
extern crate env_logger;

extern crate reqwest;

#[macro_use] extern crate serde;
extern crate serde_json;

extern crate argparse;

use argparse::{ArgumentParser, Store, List};
use std::str::FromStr;
use std::io::{stdout, stderr};

use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{Client, ConnectionProperties};
use futures::{Future, Stream};
use lapin_futures as lapin;
use log::{info, debug, error};

use std::collections::HashMap;
use std::thread;
use std::sync::mpsc::{channel, Sender};

use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::error;
use std::fmt;
use std::default::Default;

#[macro_use] extern crate lazy_static;
extern crate regex;

use regex::Regex;


// Change the alias to `Box<error::Error>`.
type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Serialize, Clone, Debug)]
struct Payload {
    pub uri: String,
    pub source: String,
    pub body: String,
    // pub ts: String
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Route {
    queue: String,
    bc_uri: String,
}

#[derive(Serialize, Deserialize, Clone)]
enum Routes {
    #[serde(rename = "map")]
    Inline (Vec<Route>),
    #[serde(rename = "fromRegistry")]
    Uri (String),
    #[serde(rename = "fromFile")]
    File (String),
}

impl fmt::Display for Routes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Routes::Inline (inline) => {
                write!(f, "inline list: {} entries", inline.len())
            },
            Routes::Uri (uri) => {
                write!(f, "load from: {}", uri)
            },
            Routes::File (file) => {
                write!(f, "include from: {}", file)
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Conf {
    amqp_addr: String,
    backend_addr: String,
    routes: Routes,
}

impl Default for Conf {
    fn default() -> Conf {
        Conf {
            amqp_addr: "".to_string(),
            backend_addr: "".to_string(),
            routes: Routes::Uri ("".to_string()),
        }
    }
}

impl fmt::Display for Conf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n*** AMQP host: {}\n*** Backend host: {}\n*** Routes: {}\n", 
            mask_credentials(&self.amqp_addr), 
            mask_credentials(&self.backend_addr), 
            self.routes)
    }
}

impl Conf {
    fn from_file(args: Vec<String>) -> Result<Conf> {
        let mut filepath = "".to_string();

        println!("ARGS: {:?}", args);

        {
            let mut ap = ArgumentParser::new();
            ap.set_description("Loads config from arguments");

            ap.refer(&mut filepath)
                .add_option(&["-f", "--file"], Store,
                r#"Path to config file (absolute or relative)"#);

            if let Err(e) = ap.parse(args, &mut stdout(), &mut stderr()) {
                std::process::exit(e);
            };

        }

        let mut file = File::open(filepath)
            .expect("Could not open conf file");
        // let mut buffered_reader = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("Error reading conf file");
        println!("FILE:\n{}\n", contents);
        let conf: Conf = serde_json::from_str(&contents)?;
        Ok(conf)
    }


    fn from_args(args: Vec<String>) -> Result<Conf> {
        // let mut conf = Conf::default();

        println!("ARGS: {:?}", args);

        let mut amqp_addr = "".to_string();
        let mut backend_addr = "".to_string();
        let mut digest_path = "".to_string();
        {
            let mut ap = ArgumentParser::new();
            ap.set_description("Loads config from arguments");

            ap.refer(&mut amqp_addr)
                .add_option(&["-q", "--amqp"], Store,
                r#"Full url of AMQP broker, e.g.: "amqp://user:password@localhost:5672/%2f""#);
            ap.refer(&mut backend_addr)
                .add_option(&["-b", "--backend"], Store,
                r#"Full url of backend formatted like: "http://localhost:8080""#);
            ap.refer(&mut digest_path)
                .add_option(&["-d", "--digest"], Store,
                r#"Path of digest endpoint which returns hash queue->url, e.g.: "/digest""#);

            if let Err(e) = ap.parse(args, &mut stdout(), &mut stderr()) {
                std::process::exit(e);
            };
        }
        let conf = Conf {
            amqp_addr: amqp_addr.clone(),
            backend_addr: backend_addr.clone(),
            routes: Routes::Uri ( digest_path.clone() ),
            ..Conf::default()
        };
        Ok(conf)
    }


    fn enum_routes(&self) -> Result<HashMap<String, String>> {
        let list: Vec<Route> = match &self.routes {
            Routes::Uri( uri ) => {
                let bc_routes: Vec<Route> = reqwest::get(uri).expect("Cannot connect to digest endpoint").json().expect("Cannot decode digest response");
                bc_routes
            },
            Routes::Inline( inline ) => (*inline).clone(),
            Routes::File( file ) => {
                panic!("Loading routes from file - Not implemented");
            },
        };
        // let root = self.backend_addr;
        let map:HashMap<String, String> = list.into_iter()
            .map(|r|{
                    let bc_uri = to_abs_url(&self.backend_addr, &r.bc_uri);
                    (r.queue.to_string(), bc_uri.to_string())
                })
            .collect();
        Ok(map)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
enum Mode {
    conf,
    cli,
}

impl FromStr for Mode {
    type Err = ();
    fn from_str(src: &str) -> std::result::Result<Mode, ()> {
        return match src {
            "params" => Ok(Mode::cli),
            "conf" => Ok(Mode::conf),
            _ => Err(()),        
        }        
    }
}

fn mask_credentials(uri: &str) -> String {
    // amqp://user:password@localhost:5672/%2f
    lazy_static! {
        static ref RE_CREDENTIALS: Regex = Regex::new(r"//([^:]*):([^@]*)@").unwrap();
    }
    RE_CREDENTIALS.replace_all(uri, "...@").to_string()
}

fn to_abs_url(root: &str, uri: &str) -> String {
    lazy_static! {
        static ref RE2: Regex = Regex::new(r"^/").unwrap();
    }
    if RE2.is_match(uri) {
        format!("{}{}", root, uri).to_string()
    } else {
        uri.to_string()
    }
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

fn main() -> Result<()> {
    env_logger::init();

    let mut mode = Mode::cli;
    let mut args = vec!();

    {  // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("AMQP bridge");
        ap.refer(&mut mode)
            .add_argument("config mode", Store,
            "Select config mode (conf|params)");

        ap.refer(&mut args)
            .add_argument("arguments", List,
                r#"Rest of arguments"#);
        ap.stop_on_first_argument(true);
        ap.parse_args_or_exit();
    }

    args.insert(0, format!("subcommand {:?}", mode));

    let conf = match mode {
        Mode::cli => 
            Conf::from_args(args)?,
        Mode::conf => 
            Conf::from_file(args)?
            
    };

    println!("Staring service with conf:\n{}", conf);

    let bc_routes: HashMap<String, String> = conf.enum_routes()?;

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
            // let target_uri = format!("{}{}", backend_addr, message.uri);
            let resp = bc_client.post(&message.uri)
                .json(&message)
                .send();
            
            match resp {
                Ok(_) =>    debug!("Data sent to backend"),
                Err(e) =>   error!("Backend error: {:?}", e),
            }

        }
    });

    futures::executor::spawn(

        Client::connect(&conf.amqp_addr, ConnectionProperties::default())
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