
mod state;
mod history;

use libp2p::{
    noise::{Keypair,X25519, NoiseConfig}, 
    tcp::TcpConfig, Transport,
    floodsub::{Floodsub, FloodsubEvent, Topic},
    core::upgrade,
    mplex,
    mdns::{Mdns, MdnsConfig, MdnsEvent}, 
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour, Swarm,
};
use log::{error, info};
use std::{collections::HashMap, process};
use state::{Msg, MessageType, State};

use tokio::{io::AsyncBufReadExt, sync::mpsc, signal::ctrl_c};
use futures::StreamExt;


#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct Session {
    dns: Mdns,
    msngr: Floodsub,
    #[behaviour(ignore)]
    state: State,
    #[behaviour(ignore)]
    peer_id: String,
    #[behaviour(ignore)]
    responder: mpsc::UnboundedSender<state::Msg>,
}

impl NetworkBehaviourEventProcess<MdnsEvent> for Session {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(nodes) => {
                for (peer, addr) in nodes {
                    info!("Peer {} found at {}", peer, addr);
                    self.msngr.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(nodes) => {
                for (peer, addr) in nodes {
                    if !self.dns.has_node(&peer) {
                        info!("Peer {} disconnected at {}", peer, addr);
                        self.msngr.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for Session {
    fn inject_event(&mut self, event: FloodsubEvent) {
        match event {
            FloodsubEvent::Message(raw_data) => {
                //Parse the message as bytes
                let deser = bincode::deserialize::<Msg>(&raw_data.data);
                if let Ok(message) = deser {
                    if let Some(user) = &message.addressee {
                        if *user != self.peer_id.to_string() {
                            return; //Don't process messages not intended for us.
                        }
                    }

                    match message.message_type {
                        MessageType::Message => {
                            let username: String =
                                self.state.get_username(&raw_data.source.to_string());
                            println!("{}: {}", username, String::from_utf8_lossy(&message.data));

                            //Store message in history
                            self.state.history.insert(message);
                        }
                        MessageType::State => {
                            info!("History recieved!");
                            let data: State = bincode::deserialize(&message.data).unwrap();
                            self.state.merge(data);
                        }
                    }
                } else {
                    error!("Unable to decode message! Due to {:?}", deser.unwrap_err());
                }
            }
            FloodsubEvent::Subscribed { peer_id, topic: _ } => {
                //Send our state to new user
                info!("Sending stage to {}", peer_id);
                let message: Msg = Msg {
                    message_type: MessageType::State,
                    data: bincode::serialize(&self.state).unwrap(),
                    addressee: Some(peer_id.to_string()),
                    source: self.peer_id.to_string(),
                };
                send_response(message, self.responder.clone());
            }
            FloodsubEvent::Unsubscribed { peer_id, topic: _ } => {
                let name = self
                    .state
                    .usernames
                    .remove(&peer_id.to_string())
                    .unwrap_or(String::from("Anon"));
                println!("{} has left the chat.", name);
            }
        }
    }
}



fn send_response(message: Msg, sender: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        if let Err(e) = sender.send(message) {
            error!("error sending response via channel {}", e);
        }
    });
}

fn send_message(message: &Msg, swarm: &mut Swarm<Session>, topic: &Topic) {
    let bytes = bincode::serialize(message).unwrap();
    swarm
        .behaviour_mut()
        .msngr
        .publish(topic.clone(), bytes);
}

 
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting chat app...");


    let peer_keys = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = libp2p::PeerId::from_public_key(&peer_keys.public());
     
    println!("peer-id: {}", peer_id);
    
    let auth_keys = Keypair::<X25519>::new().into_authentic(&peer_keys).unwrap();

    let transport = TcpConfig::new().upgrade(upgrade::Version::V1).authenticate(NoiseConfig::xx(auth_keys).into_authenticated()).multiplex(mplex::MplexConfig::new()).boxed();
    
    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    print!("Please enter a username: \n");
    let username = stdin.next_line().await.expect("a valid username").unwrap_or(String::from("unknown")).trim().to_owned();

    let mut behaviour = Session{dns: Mdns::new(MdnsConfig::default()).await.expect("unable to create mdns"),
        msngr: Floodsub::new(peer_id),
        state: State {
            history: history::History::new(),
            usernames: HashMap::from([(peer_id.to_string(), username)]),
        },
        peer_id: peer_id.to_string(),
        responder: response_sender,
    };
    let topic = Topic::new("sylo");
    behaviour.msngr.subscribe(topic.clone());

    let mut swarm = Swarm::new(transport, behaviour, peer_id);
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Some(input_line) = line.expect("a valid line") {
                    let message: Msg = Msg {
                        message_type: MessageType::Message,
                        data: input_line.as_bytes().to_vec(),
                        addressee: None,
                        source: peer_id.to_string(),
                    };

                    send_message(&message, &mut swarm, &topic);

                    swarm
                        .behaviour_mut()
                        .state
                        .history
                        .insert(message);
                }
            },
            event = swarm.select_next_some() => {
                info!("Swarm event: {:?}", event);
            },
            response = response_rcv.recv() => {
                if let Some(message) = response {
                    send_message(&message, &mut swarm, &topic);
                }
            },
            event = ctrl_c() => {
                if let Err(e) = event {
                    println!("Failed to register interrupt handler {}", e);
                }
                break;
            }
        }
    }

    swarm.behaviour_mut().msngr.unsubscribe(topic);

    swarm.select_next_some().await;

    process::exit(0);
    
}
