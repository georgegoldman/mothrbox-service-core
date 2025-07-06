use std::process::Command;


pub struct WalrusCore;

impl WalrusCore {
    fn command(&self) -> Command {
        let mut cmd = Command::new("walrus");
        cmd
    }

    pub fn read(&self, blob_id: &str) -> std::result::Result<Vec<u8>, std::io::Error> {
        let mut cmd = self.command();
        cmd.arg("read")
        .arg(blob_id);

        let output = cmd.output().expect("the program crashed");

        if output.status.success() {
            Ok(output.stdout)
        }else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Read command failed:\nStatus: {}\nStderr: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )
            ))
        }
    }

    pub fn store(&self, file: &str/*  config: &str, use_key_store: &str*/) -> Result<String, std::io::Error> {
        let mut cmd = self.command();
        cmd.arg("store")
        .arg(file)
        .args(["--epochs", "max"])
        // .args(["--config", config])
        // .args(["--wallet", use_key_store])
        .arg("--json");
        
        let output = cmd.output().expect("the program crashed");

        // println!("status: {}", output.status);
        // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        // println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            // You can return stderr or construct a custom error
            Err(std::io::Error::new(
                std::io::ErrorKind::Other, 
            format!(
                "Command failed:\nStatus: {}\nStderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

}