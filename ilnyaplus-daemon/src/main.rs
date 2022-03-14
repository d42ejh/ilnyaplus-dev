use cirrus_core::download_manager::DownloadManager;
use cirrus_core::upload_manager::UploadManager;
use cirrus_core::Uuid;
use cocoon_core::DHTManager;
use cocoon_core::DaemonConfig;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{event, span, Level};
use tracing_subscriber;

//use tracing_subscriber::prelude::*;
use ilnyaplus_messages::{
    async_trait,
    ilnyaplus::{
        ilnyaplus_rpc_service_server::{IlnyaplusRpcService, IlnyaplusRpcServiceServer},
        upload_task_info_response_message::UploadTaskInfo,
        UploadTaskInfoRequestMessage, UploadTaskInfoResponseMessage,
    },
    *,
};

//todo log to file https://github.com/tokio-rs/tracing/tree/master/tracing-appender

//#[derive(Default)]
pub struct DaemonRpcService {
    dht_manager: Arc<DHTManager>,
    dl_manager: Arc<tokio::sync::Mutex<DownloadManager>>,
    ul_manager: Arc<tokio::sync::Mutex<UploadManager>>,
}

#[async_trait]
impl IlnyaplusRpcService for DaemonRpcService {
    async fn upload(
        &self,
        request: Request<UploadRequestMessage>,
    ) -> Result<Response<UploadResponseMessage>, Status> {
        event!(
            Level::INFO,
            "Received upload request from {:?}",
            request.remote_addr()
        );

        let request_msg = request.into_inner();
        let target_file_path = PathBuf::from(request_msg.path);

        let reply;
        //use upload manager
        {
            let mut ul_manager = self.ul_manager.lock().await;
            if let Err(e) = ul_manager.upload(&target_file_path).await {
                //error, return error message
                return Err(Status::new(
                    Code::Internal,
                    "Failed to append the upload task.",
                ));
            }
            //success
            reply = UploadResponseMessage {};
        }

        Ok(Response::new(reply))
    }

    async fn download(
        &self,
        request: Request<DownloadRequestMessage>,
    ) -> Result<Response<DownloadResponseMessage>, Status> {
        let reply = DownloadResponseMessage {};
        Ok(Response::new(reply))
    }

    async fn upload_task_info(
        &self,
        request: Request<UploadTaskInfoRequestMessage>,
    ) -> Result<Response<UploadTaskInfoResponseMessage>, Status> {
        //retrive upload task infos from upload manager
        let task_infos;
        {
            let ul_manager = self.ul_manager.lock().await;
            task_infos = ul_manager.task_infos();
        }

        //convert UploadTaskInfo to proto's one
        let infos = task_infos
            .iter()
            .map(|ti| UploadTaskInfo {
                uuid: ti.id.clone(),
                file_path: ti.file_path_string.clone(),
                file_size: ti.file_size,
                is_encode_done: ti.is_encode_done,
                is_upload_done: ti.is_upload_done,
            })
            .collect();

        let reply = UploadTaskInfoResponseMessage { task_infos: infos };
        Ok(Response::new(reply))
    }

    async fn start_upload_task(
        &self,
        request: Request<StartUploadTaskRequestMessage>,
    ) -> Result<Response<StartUploadTaskResponseMessage>, Status> {
        let request_msg = request.into_inner();
        let result = Uuid::from_str(&request_msg.task_uuid);
        if result.is_err() {
            return Err(Status::new(Code::InvalidArgument, "Invalid uuid string."));
        }

        let task_uuid = result.unwrap();
        //todo start task

        let result;
        {
            let ul_manager = self.ul_manager.lock().await;
            result = ul_manager.start_task(&task_uuid).await;
        }

        if result.is_err() {}

        let reply = StartUploadTaskResponseMessage {};
        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    if args.len() != 2 {
        panic!(
            "\n[!] Launched with invalid arguments.\n[!] Usage: {} [config file path]",
            args.nth(0).unwrap()
        );
    }
    let mut config_file_path = PathBuf::new();
    config_file_path.push(args.nth(1).unwrap());
    //config
    let daemon_config = DaemonConfig::new(&config_file_path).expect("Failed to load config"); //use current dir for now, todo accept from argv
                                                                                              /*
                                                                                              let file_appender =
                                                                                                  tracing_appender::rolling::hourly(std::env::current_dir().unwrap(), "cocoon-daemon.log");
                                                                                              let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
                                                                                              tracing_subscriber::fmt().with_writer(non_blocking).init();
                                                                                              */
    //init tracing subscriber
    tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_max_level(Level::DEBUG)
        .init();
    //set up main tracing span
    //todo write logs to file instead of stdout
    //read log file path from config
    let main_span = span!(Level::DEBUG, "daemon main");
    let _ = main_span.enter();

    event!(Level::INFO, "cocoon daemon launched.");

    //print config
    event!(Level::DEBUG, "{:?}", daemon_config);

    //dht manager stuffs
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0); //todo from config
    let dht_manager = DHTManager::new(
        &daemon_config.kv_database_config,
        &daemon_config.sqlite_config,
        &bind_addr,
    )
    .await?;
    let dht_manager = Arc::new(dht_manager);
    let cloned_dht_manager = dht_manager.clone();

    //download manager
    let dl_manager = DownloadManager::new(&daemon_config.working_directory).await;
    let dl_manager = Arc::new(tokio::sync::Mutex::new(dl_manager));

    //upload manager
    let ul_manager = UploadManager::new(&daemon_config.working_directory, &dht_manager).await?;
    let ul_manager = Arc::new(tokio::sync::Mutex::new(ul_manager));

    //todo bootstrap and get own address or maybe use public key as an id

    //everything set! start the dht manager.
    dht_manager.start_receive().await;
    // tokio::join!(handle); //   loop {}
    let rpc_sevice_server = DaemonRpcService {
        dht_manager: cloned_dht_manager,
        dl_manager: dl_manager,
        ul_manager: ul_manager,
    };

    let addr = "[::1]:50051".parse().unwrap(); //todo from config
    event!(Level::DEBUG, "serve service on {}", addr);
    Server::builder()
        .add_service(IlnyaplusRpcServiceServer::new(rpc_sevice_server))
        .serve(addr)
        .await?;
    Ok(())
}
