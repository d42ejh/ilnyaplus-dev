use clap::{Parser, Subcommand};
use ilnyaplus_messages::ilnyaplus::ilnyaplus_rpc_service_client::IlnyaplusRpcServiceClient;
use ilnyaplus_messages::{
    Request, StartUploadTaskRequestMessage, UploadRequestMessage, UploadTaskInfoRequestMessage,
};
use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::io;
use tokio::sync::mpsc;
use tokio::task;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

    #[clap(short, long)]
    daemon_address: String,
}

#[derive(Subcommand)]
enum Commands {
    #[clap(arg_required_else_help = true)]
    Upload {
        target_file_path: PathBuf,
    },
    UploadTaskInfo {},
    StartUploadTask {
        task_uuid: String,
    },
}

//https://github.com/clap-rs/clap/blob/master/examples/git-derive.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let mut client = IlnyaplusRpcServiceClient::connect(args.daemon_address).await?;

    match &args.command {
        Commands::Upload { target_file_path } => {
            let target_file_path = std::fs::canonicalize(target_file_path)?; //to absolute path(daemon rejects relative target file path)
            if !target_file_path.is_file() {
                panic!("{:?} does not exist or isn't a file!", target_file_path);
            }
            println!(
                "Send upload request to daemon, target: {:?}",
                target_file_path
            );
            let request = Request::new(UploadRequestMessage {
                path: target_file_path.to_str().unwrap().to_owned(),
            });
            let response = client.upload(request).await?;
            //todo implement response
        }
        Commands::UploadTaskInfo {} => {
            let request = Request::new(UploadTaskInfoRequestMessage {});
            let response = client.upload_task_info(request).await?;
            let task_infos = &response.get_ref().task_infos;

            println!("Daemon has {} upload tasks.", task_infos.len());
            println!();

            let mut count = 1;
            for task_info in task_infos {
                //todo maybe implement Display for task info
                println!("Upload Task #{}", count);
                println!(
                    "ID: {}\nFilePath: {}\nFileSize: {}\nIsEncodeDone: {}\nIsUploadDone: {}\nCHK: {:?}",
                    task_info.uuid,
                    task_info.file_path,
                    task_info.file_size,
                    task_info.is_encode_done,
                    task_info.is_upload_done,
                    task_info.root_i_block_chk
                );
                println!();
                count += 1;
            }
        }
        Commands::StartUploadTask { task_uuid } => {
            let request = Request::new(StartUploadTaskRequestMessage {
                task_uuid: task_uuid.to_owned(),
            });
            let response = client.start_upload_task(request).await?;
            println!("Started upload task, id:{}", task_uuid);
        }
    }

    println!("Bye.");
    Ok(())
}
