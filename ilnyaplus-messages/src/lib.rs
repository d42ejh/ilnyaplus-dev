pub use ilnyaplus::ilnyaplus_rpc_service_server::IlnyaplusRpcService;
pub use ilnyaplus::{
    upload_task_info_response_message::UploadTaskInfo, DownloadRequestMessage,
    DownloadResponseMessage, StartUploadTaskRequestMessage, StartUploadTaskResponseMessage,
    UploadRequestMessage, UploadResponseMessage, UploadTaskInfoRequestMessage,
    UploadTaskInfoResponseMessage,
};
pub use tonic::async_trait;
pub use tonic::{transport::Server, Code, Request, Response, Status};
pub mod ilnyaplus {
    tonic::include_proto!("ilnyaplus");
}
