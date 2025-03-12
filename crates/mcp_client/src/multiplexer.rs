use std::borrow::Cow;
use std::collections::HashMap;
use std::process::Stdio;

use tokio::io::{
    AsyncBufReadExt,
    AsyncWriteExt,
};
use tokio::process::Child;

use crate::error::Error;

/// A layer that handles the connection between one client multiple server via stdio
/// Because the use case for chat is always request response, this layer will work like a "walkie
/// talkie", where when client is speaking, it is not to be spoken to.
#[derive(Default)]
pub struct Multiplexer<'a> {
    // TODO: abstract Child so that it's testable
    servers: HashMap<&'a str, Child>,
}

impl<'a> Multiplexer<'a> {
    // TODO: change this after proper protocol has been defined
    pub fn try_init(processes: Vec<(&'a str, &'a str)>) -> Result<Self, Error> {
        let mut servers = HashMap::default();
        let mut counter = 1;
        for (process_name, command) in processes {
            let child = tokio::process::Command::new(command)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .arg(format!("process_{}", counter))
                .spawn()?;
            servers.insert(process_name, child);
            counter += 1;
        }
        Ok(Self { servers })
    }

    // TODO: change this after proper protocol has been defined
    pub async fn request(&mut self, msg: &str) -> Result<String, Error> {
        let server = self
            .servers
            .get_mut(msg)
            .ok_or(Error::Custom(Cow::Borrowed("Server for tool does not exist")))?;
        let stdin = server
            .stdin
            .as_mut()
            .ok_or(Error::Custom(Cow::Borrowed("Server associated does not have a stdin")))?;
        stdin.write_all(b"some message\n").await?;
        stdin.flush().await?;
        let stdout = server
            .stdout
            .as_mut()
            .ok_or(Error::Custom(Cow::Borrowed("Server associated does not have a stdout")))?;
        let mut buf_reader = tokio::io::BufReader::new(stdout);
        let mut buffer = Vec::<u8>::new();
        match buf_reader.read_until(b'\n', &mut buffer).await {
            Ok(0) => Err(Error::Custom(Cow::Borrowed("Nothing was received from server"))),
            Ok(_) => Ok(String::from_utf8(buffer.clone())?),
            Err(_) => Err(Error::Custom(Cow::Borrowed("Error reading from server"))),
        }
    }
}

#[async_trait::async_trait]
trait ProcessWrapperTrait {
    async fn request(&mut self, msg: &str) -> Result<String, Error>;
}

struct ProcessWrapper(Child);

#[async_trait::async_trait]
impl ProcessWrapperTrait for ProcessWrapper {
    async fn request(&mut self, msg: &str) -> Result<String, Error> {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DUMMY: &str = "/Users/dingfeli/doodle/dummy_bin/target/release/dummy_bin";

    #[tokio::test]
    async fn test_mp() {
        let test_vec = [("0", DUMMY), ("1", DUMMY), ("2", DUMMY), ("3", DUMMY)].to_vec();

        let mut mp = Multiplexer::try_init(test_vec).expect("Multiplexer init failed");
        let res_0 = mp.request("0").await;
        let res_1 = mp.request("1").await;
        let res_2 = mp.request("2").await;
        let res_3 = mp.request("3").await;
        println!("res_0: {:?}", res_0);
        println!("res_1: {:?}", res_1);
        println!("res_2: {:?}", res_2);
        println!("res_3: {:?}", res_3);
    }
}
