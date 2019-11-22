use actix::prelude::*;

use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{Client, ConnectionProperties, Channel};

use collections::HashMap;


#[derive(Debug, Deserialize, Serialize, Clone)]
struct PubConf {
    uri: String,
    exchange: String,
    routing_key: Option<String>,
}

// 1 rxtx per queue


struct AmqpEndpoint {
    queue: String,
    routing_key: Option<String>,
}


struct AmqpMessageVector {

}

//////////////////




pub struct AmqpClient {
    conn_uri: String,
    client: Option<Client>,
    endpoints: Vec<PubConf>,
    // Route -> Actor
    channels: HashMap<String, Channel>
}

impl AmqpClient {
    pub fn new(conn_uri: String, endpoints: Vec<PubConf>) -> Self {
        Self {
            conn_uri,
            client: None,
            endpoints,
            channels: HashMap::new(),
        }
    }
}

impl SystemService for AmqpClient {
    fn service_started(&mut self, ctx: &mut Context<Self>) {
        self.restarting(ctx)
        println!("Service started");
    }
}

impl Supervised for AmqpClient {
    fn restarting(&mut self, ctx: &mut Self::Context) {
        let client = Client::connect(&self.conn_uri, ConnectionProperties::default()).wait().expect("connection error");
        self.client = Some(client);
        self.channels = self.endpoints.iter().map(|conf: PubConf| {()}).collect::<HashMap<String, Channel>>();
    }    
}


pub struct AmqpPublisher {

    client: Option<Client>,
}

impl AmqpPublisher {
    pub fn new(conn_uri: String) -> Self {
        AmqpPublisher{
            conn_uri,
            client: None
        }
    }    
}


impl Actor for AmqpPublisher {

    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {



        // Increase mailbox capacity because this is a sigle instance!
        ctx.set_mailbox_capacity(128);

        // These are broadcast messages to all instances of Router:
        println!("AmqpPublisher started");

        // self.subscribe_system_async::<messages::GetRoutesSnapshot>(ctx);

        // self.subscribe_system_async::<RouteChanged>(ctx);
        // self.subscribe_system_async::<RouteDropped>(ctx);

    }

}

impl Supervised for AmqpPublisher {
    fn restarting(&mut self, ctx: &mut Self::Context) {

    }    
}

impl SystemService for AmqpPublisher {
    fn service_started(&mut self, ctx: &mut Context<Self>) {
        println!("AmqpPublisher started");
    }
}
