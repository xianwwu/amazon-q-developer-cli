use std::io::Write as _;
use std::{
    env,
    io,
};
use mcp_client::transport::base_protocol::{JsonRpcMessage, JsonRpcResponse};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Process needs a name");
        std::process::exit(1);
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let out_going_msg = {
        let resp = JsonRpcResponse::default();
        JsonRpcMessage::Response(resp)
    };
    let mut out_going_msg = serde_json::to_vec(&out_going_msg).expect("Failed to convert outgoing msg to vec");
    out_going_msg.push(b'\n');

    let mut buffer = String::new();
    loop {
        match stdin.read_line(&mut buffer) {
            Ok(0) => continue,
            Ok(_) => {
                stdout.write_all(out_going_msg.as_slice()).expect("write failed");
                stdout.flush().expect("flush failed");
                buffer.clear();
            },
            Err(e) => {
                eprintln!("Ecountered error {:?}", e);
                break;
            },
        }
    }
}
