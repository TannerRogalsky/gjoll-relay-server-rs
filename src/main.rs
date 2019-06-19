extern crate ws;
extern crate serde_json;
extern crate redis;
use redis::Commands;

mod message;
use message::Message;

struct Server {
    out: ws::Sender,
}

impl ws::Handler for Server {
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

    if let Err(error) = ws::listen("127.0.0.1:3012", |out| {
        Server { out }
    }) {
        println!("Failed to create WebSocket due to {:?}", error);
    }
}
