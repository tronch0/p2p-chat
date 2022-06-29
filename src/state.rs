
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::history::History;

#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    pub history: History<Msg>,
    pub usernames: HashMap<String,String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageType {
    Message,
    State,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Msg {
    pub message_type: MessageType,
    pub data: Vec<u8>,
    pub addressee: Option<String>,
    pub source: String,
}

impl State{

    pub fn get_username(&self, usr: &String) -> String {
        self.usernames.get(usr).unwrap_or(&String::from("annonmous")).to_string()
    }


    pub fn merge(&mut self, mut other: State) {
        for (peer_id, user_name) in other.usernames.drain()  {
            println!("{} has joined the chat!", &user_name);
            self.usernames.insert(peer_id, user_name);
        }

        if self.history.get_count() < 1 && other.history.get_count() > 0 {
            for msg in other.history.get_all() {
                println!("{}: {}", self.get_username(&msg.source), String::from_utf8_lossy(&msg.data));
                self.history.insert(msg.to_owned());
            }
        }
    }

}