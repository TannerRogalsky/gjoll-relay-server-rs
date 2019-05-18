extern crate ws;
extern crate serde_json;
extern crate redis;
use redis::{Commands, RedisError};

extern crate serde;
use serde::{Serialize, Deserialize};

mod message;
use message::Message;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Serialize, Deserialize)]
struct RelayServerToken {
    #[serde(rename = "sessionStartTime")]
    session_start_time : i64,
}

struct Session {
    id: message::RelayKey,
//    client: Option<ws::Sender>,
//    server: ws::Sender,
//    established: bool,
//    verified: bool,
//    session_start_time: i64,
}

type Sessions = Rc<RefCell<HashMap<message::RelayKey, Session>>>;

struct Server<'a> {
    redis: &'a redis::Connection,
    sessions: Sessions,
    out: ws::Sender,
}

//impl<'a> Server<'a> {
//    fn get_or_create_session(&mut self, session_key: &message::RelayKey) -> &mut Session {
////        match self.sessions.get(session_key) {
////            Some(session) => Ok(session),
////            None => {
////                let redis_result : Result<String, RedisError> = redis::pipe().atomic()
////                    .cmd("GET").arg("bvg-streaming-relay-".to_owned() + session_key)
////                    .cmd("DEL").arg("bvg-streaming-relay-".to_owned() + session_key)
////                    .query(self.redis);
////                match redis_result {
////                    Err(err) => Err(err.to_string()),
////                    Ok(redis_key) => {
////                        println!("{:?}", redis_key);
////                        let new_session = Session {
////                            id: session_key.to_owned(),
////                        };
////                        self.sessions.insert(session_key.to_owned(), new_session);
////                        Ok(&new_session)
////                    },
////                }
////            },
////        }
//
////        match self.sessions.borrow_mut().entry(session_key.to_owned()) {
////            Entry::Occupied(entry) => Ok(entry.get()),
////            Entry::Vacant(_) => Err("oh no".to_owned()),
////        }
//
//        self.sessions.borrow_mut().entry(session_key.to_owned()).or_insert_with(|| Session { id: session_key.to_owned() })
//    }
//}

impl<'a> ws::Handler for Server<'a> {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        if let Some(ip_addr) = shake.remote_addr()? {
            println!("Connection opened from {}.", ip_addr)
        } else {
            println!("Unable to obtain client's IP address.")
        }
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        println!("Server got message '{}' from {:?}. ", msg, self.out.token());

        let result : Result<Message, serde_json::Error> = serde_json::from_str(msg.as_text().unwrap());
        match result {
            Ok(value) => {
                match value {
                    Message::Ping {} => self.out.send(serde_json::to_string(&Message::Pong {}).unwrap()),
                    Message::ClientRegister {data} => {
                        println!("{:?}", data.key);
//                        let _session = self.get_or_create_session(&data.key);
                        let _session = self.sessions.borrow_mut().entry(data.key.to_owned()).or_insert_with(|| {
                            Session {
                                id: data.key,
                            }
                        });
                        Ok(())
                    },
                    _ => self.out.close(ws::CloseCode::Unsupported),
                }
            },
            Err(_error) => self.out.close(ws::CloseCode::Invalid),
        }
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        println!("WebSocket closing for ({:?}) {}", code, reason);
    }
}

fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let con = client.get_connection().unwrap();
    let _ : () = con.set("my_key", 42).unwrap();
    let r : i32 = con.get("my_key").unwrap();
    println!("{}", r);

    let sessions = Rc::new(RefCell::new(HashMap::new()));

    if let Err(error) = ws::listen("127.0.0.1:3012", |out| {
        Server {
            redis: &con,
            sessions: sessions.clone(),
            out,
        }
    }) {
        println!("Failed to create WebSocket due to {:?}", error);
    }
}
