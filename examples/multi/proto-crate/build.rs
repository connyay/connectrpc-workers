fn main() {
    connectrpc_build::Config::new()
        .files(&[
            "proto/multi/echo/v1/echo.proto",
            "proto/multi/gateway/v1/gateway.proto",
        ])
        .includes(&["proto"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("failed to compile protos");
}
