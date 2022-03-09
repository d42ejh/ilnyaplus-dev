# daemon
cargo run ../daemon_config.toml


# client

## upload file
cargo run --  -d http://[::1]:50051 upload Cargo.toml <-(target file)

## get upload task info from daemon
cargo run --  -d http://[::1]:50051 upload-task-info
